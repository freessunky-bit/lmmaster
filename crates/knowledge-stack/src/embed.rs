//! Embedder trait + MockEmbedder.
//!
//! 정책 (ADR-0024 §3, phase-4p5-rag-decision.md §1.3):
//! - `Embedder` async trait — 실 모델은 v1.1 cascade (bge-m3 → KURE-v1 → EXAONE-Embed).
//! - `MockEmbedder` (sha256 deterministic) — unit/integration test가 안정적으로 통과해야 함.
//! - default dim = 384 (multilingual-e5-small / bge-small-multilingual 호환).
//!
//! deterministic 알고리즘:
//!   1. sha256(text) → 32 byte digest.
//!   2. digest를 dim개 f32로 펼치되, byte를 [-1, 1] 범위로 매핑.
//!   3. 결과 벡터를 L2 normalize — cosine similarity 안정.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use crate::embed_download::{is_downloaded, OnnxModelKind};
use crate::error::KnowledgeError;

/// Embedder trait. caller가 워크스페이스마다 inject.
#[async_trait]
pub trait Embedder: Send + Sync {
    fn dim(&self) -> usize;
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, KnowledgeError>;
}

/// sha256 기반 deterministic 임베더 — 실 모델이 연결되기 전 default.
pub struct MockEmbedder {
    dim: usize,
}

impl MockEmbedder {
    /// dim default = 384 (multilingual-e5-small / bge-small-multilingual 호환).
    pub fn new(dim: usize) -> Self {
        Self { dim: dim.max(1) }
    }
}

impl Default for MockEmbedder {
    fn default() -> Self {
        Self::new(384)
    }
}

#[async_trait]
impl Embedder for MockEmbedder {
    fn dim(&self) -> usize {
        self.dim
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, KnowledgeError> {
        let mut out = Vec::with_capacity(texts.len());
        for text in texts {
            let v = mock_vector(text, self.dim);
            out.push(v);
        }
        Ok(out)
    }
}

/// sha256(text) → dim 길이 f32 벡터 (L2 normalize). pure function.
fn mock_vector(text: &str, dim: usize) -> Vec<f32> {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let digest = hasher.finalize();
    // 32 byte를 dim개로 확장 — 각 인덱스마다 hash(seed=index, prev=digest[i % 32])로 추가 byte 생성.
    let mut bytes: Vec<u8> = Vec::with_capacity(dim);
    let mut current = digest.to_vec();
    while bytes.len() < dim {
        for &b in &current {
            bytes.push(b);
            if bytes.len() >= dim {
                break;
            }
        }
        if bytes.len() < dim {
            // re-hash로 확장.
            let mut h2 = Sha256::new();
            h2.update(&current);
            current = h2.finalize().to_vec();
        }
    }
    // byte → [-1, 1] 매핑.
    let mut vec: Vec<f32> = bytes
        .into_iter()
        .map(|b| (b as f32 / 127.5) - 1.0)
        .collect();
    // L2 normalize — cosine similarity의 분모 항을 ||v|| = 1로 고정.
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for x in vec.iter_mut() {
            *x /= norm;
        }
    }
    vec
}

/// 사용자 활성 모델로 `Embedder`를 만들어요. 모델 파일이 없거나 ONNX feature가 꺼져 있으면
/// `fallback_to_mock`에 따라 `MockEmbedder`로 fallback하거나 명확한 에러를 돌려줘요.
///
/// 정책 (ADR-0042 §Decision):
/// - 사용자 PC에 ONNX Runtime 동적 라이브러리가 없거나 모델 파일이 없으면 RAG 자체는 동작해야 해요
///   (mock fallback). ranking 품질만 deterministic-mock이라 사용자에게 "정확도 낮음" 안내가 의무.
/// - `embed-onnx` feature가 꺼진 빌드(예: CI smoke build)도 동일 동작 — Mock 사용.
pub async fn default_embedder(
    target_dir: &Path,
    preferred_kind: Option<OnnxModelKind>,
    fallback_to_mock: bool,
) -> Result<Arc<dyn Embedder>, KnowledgeError> {
    if let Some(kind) = preferred_kind {
        if is_downloaded(target_dir, kind) {
            #[cfg(feature = "embed-onnx")]
            {
                match crate::embed_onnx::try_load_from_dir(target_dir, kind) {
                    Ok(emb) => {
                        // Warmup은 비용 있어 caller (Tauri setup)가 별도 호출 가능. 여기선 skip.
                        return Ok(Arc::new(emb));
                    }
                    Err(e) => {
                        if fallback_to_mock {
                            tracing::warn!(error = %e, "ONNX embedder 로드 실패 — mock으로 폴백");
                            return Ok(Arc::new(MockEmbedder::new(kind.dim())));
                        }
                        return Err(e);
                    }
                }
            }
            #[cfg(not(feature = "embed-onnx"))]
            {
                if fallback_to_mock {
                    tracing::warn!(
                        kind = kind.as_kebab(),
                        "embed-onnx feature가 꺼져 있어 ONNX embedder를 만들지 못했어요 — mock fallback",
                    );
                    return Ok(Arc::new(MockEmbedder::new(kind.dim())));
                }
                return Err(KnowledgeError::EmbeddingFailed(format!(
                    "embed-onnx feature 없이 빌드돼 {} 모델을 사용할 수 없어요",
                    kind.as_kebab()
                )));
            }
        }
        if fallback_to_mock {
            tracing::info!(
                kind = kind.as_kebab(),
                "임베딩 모델이 다운로드되지 않았어요 — mock 사용",
            );
            return Ok(Arc::new(MockEmbedder::new(kind.dim())));
        }
        return Err(KnowledgeError::EmbeddingFailed(format!(
            "{} 모델 파일이 디스크에 없어요. 먼저 다운로드해 주세요",
            kind.as_kebab()
        )));
    }
    // preferred_kind 없음 — fallback 분기.
    if fallback_to_mock {
        Ok(Arc::new(MockEmbedder::default()))
    } else {
        Err(KnowledgeError::EmbeddingFailed(
            "사용할 임베딩 모델을 지정해 주세요".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn deterministic_output() {
        let e = MockEmbedder::default();
        let texts = vec!["안녕하세요".to_string()];
        let a = e.embed(&texts).await.unwrap();
        let b = e.embed(&texts).await.unwrap();
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn dim_correctness() {
        let e = MockEmbedder::new(384);
        let texts = vec!["test".to_string()];
        let v = e.embed(&texts).await.unwrap();
        assert_eq!(v[0].len(), 384);
    }

    #[tokio::test]
    async fn dim_custom_size() {
        let e = MockEmbedder::new(128);
        assert_eq!(e.dim(), 128);
        let v = e.embed(&[String::from("x")]).await.unwrap();
        assert_eq!(v[0].len(), 128);
    }

    #[tokio::test]
    async fn empty_input_returns_empty_vec() {
        let e = MockEmbedder::default();
        let v = e.embed(&[]).await.unwrap();
        assert!(v.is_empty());
    }

    #[tokio::test]
    async fn batch_size_matches_input() {
        let e = MockEmbedder::default();
        let texts: Vec<String> = (0..5).map(|i| format!("text {i}")).collect();
        let v = e.embed(&texts).await.unwrap();
        assert_eq!(v.len(), 5);
        for x in &v {
            assert_eq!(x.len(), 384);
        }
    }

    #[tokio::test]
    async fn different_texts_yield_different_vectors() {
        let e = MockEmbedder::default();
        let a = e.embed(&[String::from("apple")]).await.unwrap();
        let b = e.embed(&[String::from("banana")]).await.unwrap();
        assert_ne!(a[0], b[0]);
    }

    #[tokio::test]
    async fn concurrent_calls_consistent() {
        use std::sync::Arc;
        let e: Arc<MockEmbedder> = Arc::new(MockEmbedder::default());
        let mut handles = Vec::new();
        for _ in 0..8 {
            let e_clone = Arc::clone(&e);
            handles.push(tokio::spawn(async move {
                e_clone.embed(&[String::from("동일 텍스트")]).await
            }));
        }
        let mut results = Vec::new();
        for h in handles {
            results.push(h.await.unwrap().unwrap());
        }
        for w in results.windows(2) {
            assert_eq!(w[0], w[1]);
        }
    }

    #[tokio::test]
    async fn l2_normalized() {
        let e = MockEmbedder::default();
        let v = e.embed(&[String::from("normalize check")]).await.unwrap();
        let norm: f32 = v[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        // L2 normalize 후 norm은 1.0 근처 (float 오차 허용).
        assert!((norm - 1.0).abs() < 1e-3, "expected ||v||≈1, got {norm}");
    }

    // ── default_embedder helper ──────────────────────────────────────

    #[tokio::test]
    async fn default_embedder_returns_mock_when_no_files_with_fallback() {
        let dir = TempDir::new().unwrap();
        let emb = default_embedder(dir.path(), Some(OnnxModelKind::BgeM3), true)
            .await
            .unwrap();
        assert_eq!(emb.dim(), OnnxModelKind::BgeM3.dim());
    }

    #[tokio::test]
    async fn default_embedder_errors_when_no_files_no_fallback() {
        let dir = TempDir::new().unwrap();
        let res = default_embedder(dir.path(), Some(OnnxModelKind::KureV1), false).await;
        assert!(matches!(res, Err(KnowledgeError::EmbeddingFailed(_))));
    }

    #[tokio::test]
    async fn default_embedder_returns_default_mock_when_kind_none_with_fallback() {
        let dir = TempDir::new().unwrap();
        let emb = default_embedder(dir.path(), None, true).await.unwrap();
        // default Mock = 384.
        assert_eq!(emb.dim(), 384);
    }

    #[tokio::test]
    async fn default_embedder_errors_when_kind_none_no_fallback() {
        let dir = TempDir::new().unwrap();
        let res = default_embedder(dir.path(), None, false).await;
        assert!(matches!(res, Err(KnowledgeError::EmbeddingFailed(_))));
    }
}

//! ONNX Runtime 기반 실 임베더 — Phase 9'.a (ADR-0042).
//!
//! 정책 (ADR-0042):
//! - `ort = "2.0.0-rc.10"` + `load-dynamic` feature — Tauri bundle은 정적 링크 부담 없이,
//!   사용자 PC에 `onnxruntime` 동적 라이브러리만 있으면 동작. 미설치 시 `OnnxEmbedder::from_files`가
//!   graceful 한국어 에러 반환 (panic X).
//! - `tokenizers = "0.20"` — HuggingFace Rust tokenizer. truncation max_len=512 (모델 한도 일치).
//! - mean pooling + L2 normalize — multilingual-e5 / bge-m3 / KURE-v1 표준 inference 패턴.
//! - `tokio::task::spawn_blocking`으로 ort `session.run()`을 wrap — Tauri main thread 블락 회피.
//! - cancel은 `CancellationToken` (start-of-batch + 매 batch 사이 검사). chunk 단위 mid-flight cancel은
//!   spawn_blocking 안에서 ort session 자체를 중단할 방법이 없어 batch 경계로만 보장 — caller가 batch
//!   사이즈를 작게 잡으면 사실상 즉시.
//!
//! References (negative space):
//! - OpenAI embedding API 직접 호출 → 외부 통신 0 위반. 거부.
//! - Sentence-Transformers Python sidecar → 콜드 스타트 + Python 부담. 거부.
//! - llama.cpp embedding endpoint → 한국어 임베딩 품질 낮음. 거부.
//! - 단일 모델만 지원 → 사용자 선택권 부족. 거부.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use ndarray::{Array1, Array2};
use ort::session::Session;
use ort::value::Value;
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};
use tokio_util::sync::CancellationToken;

use crate::embed::Embedder;
use crate::embed_download::OnnxModelKind;
use crate::error::KnowledgeError;

/// 모델당 시퀀스 max length — bge-m3 / KURE-v1 / multilingual-e5 모두 512 token이 표준.
const MAX_SEQ_LEN: usize = 512;

/// ONNX Runtime + tokenizer를 메모리에 hold하는 임베더.
///
/// 실 모델 추론은 `Embedder::embed`에서 `tokio::task::spawn_blocking`으로 호출 — caller는 async.
pub struct OnnxEmbedder {
    /// `ort::Session`은 Sync라 Arc로 감싸 다른 task에 전달 가능.
    session: Arc<Session>,
    tokenizer: Arc<Tokenizer>,
    kind: OnnxModelKind,
    dim: usize,
    cancel: CancellationToken,
}

impl OnnxEmbedder {
    /// 모델 + tokenizer 파일 경로로부터 임베더를 만든다. 파일이 없거나 파싱 실패 시 한국어 에러.
    ///
    /// 주의: ONNX Runtime 동적 라이브러리(`onnxruntime.dll` / `.so` / `.dylib`)가 사용자 PC에
    /// 없으면 ort가 graceful 에러를 반환 — 그대로 한국어 메시지로 wrap.
    pub fn from_files(
        model_path: PathBuf,
        tokenizer_path: PathBuf,
        kind: OnnxModelKind,
    ) -> Result<Self, KnowledgeError> {
        if !model_path.is_file() {
            return Err(KnowledgeError::EmbeddingFailed(format!(
                "임베딩 모델 파일을 찾지 못했어요: {}",
                model_path.display()
            )));
        }
        if !tokenizer_path.is_file() {
            return Err(KnowledgeError::EmbeddingFailed(format!(
                "토크나이저 파일을 찾지 못했어요: {}",
                tokenizer_path.display()
            )));
        }

        // Tokenizer는 단순 JSON 파일 — load-dynamic 없이 동작.
        let mut tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|e| {
            KnowledgeError::EmbeddingFailed(format!("토크나이저를 불러오지 못했어요: {e}"))
        })?;
        // 표준 truncation + padding — 배치 인퍼런스 안정성.
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: MAX_SEQ_LEN,
                ..Default::default()
            }))
            .map_err(|e| {
                KnowledgeError::EmbeddingFailed(format!("토크나이저 truncation 설정 실패: {e}"))
            })?;
        tokenizer.with_padding(Some(PaddingParams {
            strategy: PaddingStrategy::BatchLongest,
            ..Default::default()
        }));

        // ort Session — load-dynamic이라 ORT 라이브러리 미설치 시 commit_from_file에서 graceful 에러.
        // ort 2.0 RC10 API: Session::builder().commit_from_file(path).
        let session = Session::builder()
            .map_err(|e| {
                KnowledgeError::EmbeddingFailed(format!(
                    "ONNX Runtime 세션 빌더를 만들지 못했어요: {e}"
                ))
            })?
            .commit_from_file(&model_path)
            .map_err(|e| {
                KnowledgeError::EmbeddingFailed(format!(
                    "ONNX 모델을 불러오지 못했어요. ONNX Runtime 라이브러리가 설치돼 있는지 확인해 주세요: {e}"
                ))
            })?;

        Ok(Self {
            session: Arc::new(session),
            tokenizer: Arc::new(tokenizer),
            kind,
            dim: kind.dim(),
            cancel: CancellationToken::new(),
        })
    }

    /// 외부 cancel token을 주입 — caller가 IPC cancel과 묶을 때 사용.
    pub fn with_cancel(mut self, cancel: CancellationToken) -> Self {
        self.cancel = cancel;
        self
    }

    /// Graph 워밍업 — 첫 inference latency를 미리 지불. Embedder 생성 후 1회 권장.
    pub async fn warmup(&self) -> Result<(), KnowledgeError> {
        let _ = self.embed(&["워밍업".to_string()]).await?;
        Ok(())
    }

    /// 모델 종류 query.
    pub fn kind(&self) -> OnnxModelKind {
        self.kind
    }

    /// 동기 inference — `embed`가 spawn_blocking 안에서 호출.
    fn run_batch_blocking(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, KnowledgeError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // 배치 토크나이즈.
        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| {
                KnowledgeError::EmbeddingFailed(format!("토크나이저 배치 처리 실패: {e}"))
            })?;
        if encodings.is_empty() {
            return Ok(Vec::new());
        }
        let batch = encodings.len();
        let seq_len = encodings
            .iter()
            .map(|e| e.get_ids().len())
            .max()
            .unwrap_or(0);
        if seq_len == 0 {
            return Ok(vec![vec![0.0; self.dim]; batch]);
        }

        // input_ids / attention_mask / token_type_ids 텐서 (i64).
        let mut input_ids = Array2::<i64>::zeros((batch, seq_len));
        let mut attention_mask = Array2::<i64>::zeros((batch, seq_len));
        let mut token_type_ids = Array2::<i64>::zeros((batch, seq_len));
        for (row, enc) in encodings.iter().enumerate() {
            let ids = enc.get_ids();
            let attn = enc.get_attention_mask();
            let types = enc.get_type_ids();
            for col in 0..seq_len {
                if col < ids.len() {
                    input_ids[[row, col]] = ids[col] as i64;
                    attention_mask[[row, col]] = attn[col] as i64;
                    token_type_ids[[row, col]] = types[col] as i64;
                }
            }
        }

        // ort Value로 변환 — ort 2.0 RC10: Value::from_array(ndarray) → TensorRef.
        let input_ids_value = Value::from_array(input_ids).map_err(|e| {
            KnowledgeError::EmbeddingFailed(format!("input_ids 텐서 변환 실패: {e}"))
        })?;
        let attention_mask_for_run = attention_mask.clone();
        let attention_mask_value = Value::from_array(attention_mask_for_run).map_err(|e| {
            KnowledgeError::EmbeddingFailed(format!("attention_mask 텐서 변환 실패: {e}"))
        })?;
        let token_type_value = Value::from_array(token_type_ids).map_err(|e| {
            KnowledgeError::EmbeddingFailed(format!("token_type_ids 텐서 변환 실패: {e}"))
        })?;

        // 모델별로 input name이 다를 수 있어 session에서 query.
        // ort 2.0 RC10: session.run(inputs!{...}) 매크로 또는 vec of (name, value).
        let input_names: Vec<String> = self.session.inputs.iter().map(|i| i.name.clone()).collect();
        let mut session_inputs: Vec<(std::borrow::Cow<'static, str>, ort::value::DynValue)> =
            Vec::with_capacity(3);
        for name in &input_names {
            let dyn_value: ort::value::DynValue = match name.as_str() {
                "input_ids" => input_ids_value.clone().into_dyn(),
                "attention_mask" => attention_mask_value.clone().into_dyn(),
                "token_type_ids" => token_type_value.clone().into_dyn(),
                other => {
                    return Err(KnowledgeError::EmbeddingFailed(format!(
                        "예상하지 못한 입력 이름이에요: {other}"
                    )));
                }
            };
            session_inputs.push((std::borrow::Cow::Owned(name.clone()), dyn_value));
        }

        let outputs = self
            .session
            .run(session_inputs)
            .map_err(|e| KnowledgeError::EmbeddingFailed(format!("ONNX 추론 실행 실패: {e}")))?;

        // last_hidden_state — (batch, seq_len, hidden) f32. 일부 모델은 첫 번째 output이 pooled여서
        // 그대로 사용 가능. 우선 마지막 hidden state로 mean pooling을 시도.
        let first_output_name = self
            .session
            .outputs
            .first()
            .map(|o| o.name.clone())
            .ok_or_else(|| {
                KnowledgeError::EmbeddingFailed("ONNX 출력 메타가 비어 있어요".into())
            })?;
        let tensor = outputs
            .get(first_output_name.as_str())
            .ok_or_else(|| KnowledgeError::EmbeddingFailed("ONNX 출력이 비어 있어요".into()))?;
        let (shape_dyn, slice) = tensor
            .try_extract_tensor::<f32>()
            .map_err(|e| KnowledgeError::EmbeddingFailed(format!("출력 텐서 추출 실패: {e}")))?;
        let shape: Vec<usize> = shape_dyn.iter().map(|&d| d as usize).collect();

        // 두 가지 케이스:
        // - rank=3 (batch, seq, hidden): mean pooling.
        // - rank=2 (batch, hidden): pre-pooled — 그대로 사용.
        let pooled: Vec<Vec<f32>> = if shape.len() == 3 {
            let hidden = shape[2];
            mean_pool_with_mask(slice, &attention_mask, batch, seq_len, hidden)
        } else if shape.len() == 2 {
            let hidden = shape[1];
            (0..batch)
                .map(|b| {
                    let start = b * hidden;
                    slice[start..start + hidden].to_vec()
                })
                .collect()
        } else {
            return Err(KnowledgeError::EmbeddingFailed(format!(
                "예상하지 못한 출력 차원이에요: {shape:?}"
            )));
        };

        // L2 normalize.
        let normalized = pooled
            .into_iter()
            .map(|v| {
                let arr = Array1::from(v);
                let norm = arr.dot(&arr).sqrt();
                if norm > f32::EPSILON {
                    (arr / norm).to_vec()
                } else {
                    arr.to_vec()
                }
            })
            .collect::<Vec<_>>();

        // 첫 row의 길이로 self.dim 검증 — 모델/카탈로그 dim mismatch 조기 탐지.
        if let Some(first) = normalized.first() {
            if first.len() != self.dim {
                return Err(KnowledgeError::EmbeddingFailed(format!(
                    "모델 출력 차원이 카탈로그와 달라요. 기대 {}, 실제 {}",
                    self.dim,
                    first.len()
                )));
            }
        }

        Ok(normalized)
    }
}

/// Mean pooling — last_hidden_state * attention_mask, normalize by sum(mask).
fn mean_pool_with_mask(
    flat: &[f32],
    attention_mask: &Array2<i64>,
    batch: usize,
    seq_len: usize,
    hidden: usize,
) -> Vec<Vec<f32>> {
    let mut out = Vec::with_capacity(batch);
    for b in 0..batch {
        let mut sum = vec![0.0_f32; hidden];
        let mut count = 0u32;
        for s in 0..seq_len {
            let mask = attention_mask[[b, s]];
            if mask == 0 {
                continue;
            }
            count += 1;
            let base = (b * seq_len + s) * hidden;
            for h in 0..hidden {
                sum[h] += flat[base + h];
            }
        }
        let denom = (count.max(1)) as f32;
        for x in sum.iter_mut() {
            *x /= denom;
        }
        out.push(sum);
    }
    out
}

#[async_trait]
impl Embedder for OnnxEmbedder {
    fn dim(&self) -> usize {
        self.dim
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, KnowledgeError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        if self.cancel.is_cancelled() {
            return Err(KnowledgeError::Cancelled);
        }
        // session/tokenizer/cancel 모두 Arc/Token이라 cheap clone.
        let session = Arc::clone(&self.session);
        let tokenizer = Arc::clone(&self.tokenizer);
        let cancel = self.cancel.clone();
        let kind = self.kind;
        let dim = self.dim;
        let texts_owned = texts.to_vec();

        let result = tokio::task::spawn_blocking(move || {
            // 새 OnnxEmbedder shell을 spawn_blocking 안에서 만들지 않고, 필요한 필드만 직접 사용.
            let inner = OnnxEmbedder {
                session,
                tokenizer,
                kind,
                dim,
                cancel: cancel.clone(),
            };
            inner.run_batch_blocking(&texts_owned)
        })
        .await
        .map_err(|e| KnowledgeError::EmbeddingFailed(format!("백그라운드 추론 작업 실패: {e}")))?;
        result
    }
}

/// 한 모델 디렉터리 → OnnxEmbedder 시도. 미존재 시 graceful 에러.
pub fn try_load_from_dir(
    target_dir: &Path,
    kind: OnnxModelKind,
) -> Result<OnnxEmbedder, KnowledgeError> {
    let model_path = crate::embed_download::model_file_path(target_dir, kind);
    let tokenizer_path = crate::embed_download::tokenizer_file_path(target_dir, kind);
    OnnxEmbedder::from_files(model_path, tokenizer_path, kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn from_files_returns_error_when_model_missing() {
        let dir = TempDir::new().unwrap();
        let model = dir.path().join("model.onnx");
        let tok = dir.path().join("tokenizer.json");
        // 토크나이저만 있고 model이 없는 케이스.
        std::fs::write(&tok, b"{}").unwrap();
        let res = OnnxEmbedder::from_files(model, tok, OnnxModelKind::BgeM3);
        match res {
            Err(KnowledgeError::EmbeddingFailed(msg)) => {
                assert!(
                    msg.contains("모델") || msg.contains("찾지 못했어요"),
                    "한국어 메시지 필요 — got {msg}"
                );
            }
            other => panic!("expected EmbeddingFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn from_files_returns_error_when_tokenizer_missing() {
        let dir = TempDir::new().unwrap();
        let model = dir.path().join("model.onnx");
        let tok = dir.path().join("tokenizer.json");
        std::fs::write(&model, b"fake-onnx").unwrap();
        let res = OnnxEmbedder::from_files(model, tok, OnnxModelKind::BgeM3);
        match res {
            Err(KnowledgeError::EmbeddingFailed(msg)) => {
                assert!(msg.contains("토크나이저"));
            }
            other => panic!("expected EmbeddingFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn try_load_from_dir_propagates_missing() {
        let dir = TempDir::new().unwrap();
        let res = try_load_from_dir(dir.path(), OnnxModelKind::KureV1);
        assert!(matches!(res, Err(KnowledgeError::EmbeddingFailed(_))));
    }

    /// mean_pool은 pure — 직접 검증.
    #[test]
    fn mean_pool_respects_mask() {
        // batch=1, seq=2, hidden=2. 첫 token만 mask=1, 두 번째는 0.
        // last_hidden = [[1, 2], [99, 99]]. 결과는 [1, 2] (두 번째 무시).
        let flat = vec![1.0, 2.0, 99.0, 99.0];
        let mask = ndarray::array![[1i64, 0]];
        let pooled = mean_pool_with_mask(&flat, &mask, 1, 2, 2);
        assert_eq!(pooled, vec![vec![1.0, 2.0]]);
    }

    #[test]
    fn mean_pool_with_full_mask_is_average() {
        // batch=1, seq=2, hidden=2. 둘 다 mask=1. 평균 [1, 3] (rows [0,2] + [2,4]).
        let flat = vec![0.0, 2.0, 2.0, 4.0];
        let mask = ndarray::array![[1i64, 1]];
        let pooled = mean_pool_with_mask(&flat, &mask, 1, 2, 2);
        assert_eq!(pooled, vec![vec![1.0, 3.0]]);
    }

    #[test]
    fn mean_pool_zero_mask_returns_zeros_division_safe() {
        // 모든 mask=0이면 count=1로 fallback해 NaN 방지.
        let flat = vec![5.0, 7.0];
        let mask = ndarray::array![[0i64]];
        let pooled = mean_pool_with_mask(&flat, &mask, 1, 1, 2);
        // sum 항이 mask=0이라 0이 됨. count=max(0,1)=1. → [0,0].
        assert_eq!(pooled, vec![vec![0.0, 0.0]]);
    }
}

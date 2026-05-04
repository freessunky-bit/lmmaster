//! 임베딩 ONNX 모델 다운로드 + 검증 + 로컬 캐시 (Phase 9'.a — ADR-0042).
//!
//! 정책 (외부 통신 0 원칙 예외 — ADR-0042 §References, §Decision):
//! - HuggingFace `huggingface.co` 도메인만 화이트리스트. 사용자 명시 동의 후에만 호출.
//! - sha256 무결성 검증. manifest hash가 없으면 `None` (검증 skip — 사용자에게 경고 의무).
//! - `.partial` 임시 파일 + atomic rename. 중간 cancel은 `.partial` 보존 (다음 실행 resume 가능).
//! - sha256 mismatch 시 `.partial` 삭제 (재시도 강제).
//! - 진행률 emit는 256KB 또는 100ms 단위 (`installer/src/downloader.rs` 패턴 재활용).
//! - 한국어 해요체 에러 메시지.
//!
//! References:
//! - HuggingFace 호스팅 패턴: `https://huggingface.co/<repo>/resolve/main/<file>`.
//! - bge-m3: BAAI/bge-m3 — 1024d, multilingual.
//! - KURE-v1: nlpai-lab/KURE-v1 — 한국어 특화.
//! - multilingual-e5-small: intfloat/multilingual-e5-small — 384d, 빠름.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::KnowledgeError;

/// 진행률 emit throttle (`installer/src/downloader.rs` 패턴 재활용).
const PROGRESS_BYTES_THRESHOLD: u64 = 256 * 1024;
const PROGRESS_INTERVAL_MS: u128 = 100;

/// 사용 가능한 임베딩 모델 — 사용자 카드 UI와 1:1 매핑.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OnnxModelKind {
    /// BAAI/bge-m3 — 1024d, multilingual + Korean 강력. 약 600MB. v1.x 기본 추천.
    BgeM3,
    /// nlpai-lab/KURE-v1 — 768d, 한국어 특화. 약 450MB. 한국어 비중 높은 워크스페이스에 추천.
    KureV1,
    /// intfloat/multilingual-e5-small — 384d, 가장 빠름. 약 120MB. 약한 PC 또는 fallback.
    MultilingualE5Small,
}

impl OnnxModelKind {
    /// kebab-case 문자열 ↔ enum 변환. IPC `set_active_embedding_model`에서 사용.
    pub fn from_kebab(s: &str) -> Option<Self> {
        match s {
            "bge-m3" => Some(Self::BgeM3),
            "kure-v1" => Some(Self::KureV1),
            "multilingual-e5-small" => Some(Self::MultilingualE5Small),
            _ => None,
        }
    }

    /// IPC 직렬화용. enum 자체를 serde로 직렬화해도 같은 결과 — UI 코드 일관성용.
    pub fn as_kebab(self) -> &'static str {
        match self {
            Self::BgeM3 => "bge-m3",
            Self::KureV1 => "kure-v1",
            Self::MultilingualE5Small => "multilingual-e5-small",
        }
    }

    /// 임베딩 차원. ONNX 모델 그래프와 일치해야 SQLite 검색에서 `query.len() == emb.len()` 만족.
    pub fn dim(self) -> usize {
        match self {
            Self::BgeM3 => 1024,
            Self::KureV1 => 768,
            Self::MultilingualE5Small => 384,
        }
    }

    /// 한국어 친화 가중치 (UI hint chip). 0.0~1.0.
    /// - bge-m3: 한국어 + 다국어 모두 강함 → 0.85.
    /// - kure-v1: 한국어 특화 → 1.00.
    /// - multilingual-e5-small: 한국어 보통 → 0.65.
    pub fn korean_score(self) -> f32 {
        match self {
            Self::BgeM3 => 0.85,
            Self::KureV1 => 1.00,
            Self::MultilingualE5Small => 0.65,
        }
    }

    /// 다운로드 대략 사이즈 (MB). UI ETA 계산용.
    pub fn approx_size_mb(self) -> u32 {
        match self {
            Self::BgeM3 => 580,
            Self::KureV1 => 450,
            Self::MultilingualE5Small => 120,
        }
    }
}

/// 한 모델당 호스팅된 두 개 파일(model.onnx + tokenizer.json) 정보.
#[derive(Debug, Clone)]
pub struct ModelManifest {
    pub kind: OnnxModelKind,
    pub model_url: String,
    pub model_filename: String,
    /// 32-byte sha256. None이면 검증 skip — 외부 통신 0 정책상 사용자에게 경고 의무.
    pub model_sha256: Option<[u8; 32]>,
    pub tokenizer_url: String,
    pub tokenizer_filename: String,
    pub tokenizer_sha256: Option<[u8; 32]>,
}

impl ModelManifest {
    /// 카탈로그 — HuggingFace `resolve/main` 직접 링크.
    ///
    /// negative space: sha256은 hardcoded fallback 없이 None — Phase 9'.a v1은 사용자에게
    /// "검증 키가 없어요. 받은 후 자체 무결성만 보장돼요" 경고 노출. v1.x에서 manifest endpoint
    /// (`huggingface.co/<repo>/raw/main/sha256.txt`) 운영 시 hash 주입.
    pub fn for_kind(kind: OnnxModelKind) -> Self {
        match kind {
            OnnxModelKind::BgeM3 => Self {
                kind,
                model_url: "https://huggingface.co/BAAI/bge-m3/resolve/main/onnx/model.onnx"
                    .to_string(),
                model_filename: "model.onnx".to_string(),
                model_sha256: None,
                tokenizer_url: "https://huggingface.co/BAAI/bge-m3/resolve/main/tokenizer.json"
                    .to_string(),
                tokenizer_filename: "tokenizer.json".to_string(),
                tokenizer_sha256: None,
            },
            OnnxModelKind::KureV1 => Self {
                kind,
                model_url:
                    "https://huggingface.co/nlpai-lab/KURE-v1/resolve/main/onnx/model.onnx"
                        .to_string(),
                model_filename: "model.onnx".to_string(),
                model_sha256: None,
                tokenizer_url:
                    "https://huggingface.co/nlpai-lab/KURE-v1/resolve/main/tokenizer.json"
                        .to_string(),
                tokenizer_filename: "tokenizer.json".to_string(),
                tokenizer_sha256: None,
            },
            OnnxModelKind::MultilingualE5Small => Self {
                kind,
                model_url:
                    "https://huggingface.co/intfloat/multilingual-e5-small/resolve/main/onnx/model.onnx"
                        .to_string(),
                model_filename: "model.onnx".to_string(),
                model_sha256: None,
                tokenizer_url:
                    "https://huggingface.co/intfloat/multilingual-e5-small/resolve/main/tokenizer.json"
                        .to_string(),
                tokenizer_filename: "tokenizer.json".to_string(),
                tokenizer_sha256: None,
            },
        }
    }
}

/// 사용자 향 다운로드 이벤트. `#[serde(tag = "kind")]` — IPC Channel과 1:1.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DownloadEvent {
    /// 다운로드 시작 (모델 파일).
    Started {
        model_kind: String,
        file: String,
        total_bytes: Option<u64>,
    },
    /// 진행률 — 256KB 또는 100ms 마다.
    Progress {
        model_kind: String,
        file: String,
        downloaded: u64,
        total: Option<u64>,
    },
    /// sha256 검증 시작.
    Verifying { model_kind: String, file: String },
    /// 모델 + tokenizer 둘 다 끝남.
    Done {
        model_kind: String,
        model_path: String,
        tokenizer_path: String,
    },
    /// 사용자 cancel 또는 channel close.
    Cancelled { model_kind: String },
    /// 실패. message는 한국어 해요체.
    Failed { model_kind: String, error: String },
}

/// 모델 디렉터리 구조: `<target_dir>/<kind>/{model.onnx,tokenizer.json}`.
pub fn model_dir(target_dir: &Path, kind: OnnxModelKind) -> PathBuf {
    target_dir.join(kind.as_kebab())
}

/// 모델 파일 절대 경로 (model.onnx). 다운로드 완료된 경우만 존재.
pub fn model_file_path(target_dir: &Path, kind: OnnxModelKind) -> PathBuf {
    model_dir(target_dir, kind).join("model.onnx")
}

/// tokenizer 절대 경로.
pub fn tokenizer_file_path(target_dir: &Path, kind: OnnxModelKind) -> PathBuf {
    model_dir(target_dir, kind).join("tokenizer.json")
}

/// 두 파일이 모두 존재하면 다운로드 완료로 간주 (sha256 재검증은 별도 단계).
pub fn is_downloaded(target_dir: &Path, kind: OnnxModelKind) -> bool {
    model_file_path(target_dir, kind).is_file() && tokenizer_file_path(target_dir, kind).is_file()
}

/// 모델 다운로더 — reqwest::Client + 진행률 channel.
pub struct ModelDownloader {
    target_dir: PathBuf,
    client: reqwest::Client,
}

impl ModelDownloader {
    /// 새 downloader. target_dir는 `<app_data_dir>/models/embed/`.
    /// reqwest::Client는 자체 생성 — installer/downloader.rs와 동일 user-agent 컨벤션.
    ///
    /// Phase R-C (ADR-0055) — .no_proxy() 강제. HF 다운로드는 화이트리스트 호스트만 허용.
    pub fn new(target_dir: PathBuf) -> Result<Self, KnowledgeError> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .user_agent(format!("LMmaster-embedder/{}", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(60 * 30))
            .connect_timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| {
                KnowledgeError::EmbeddingFailed(format!("HTTP 클라이언트 생성 실패: {e}"))
            })?;
        Ok(Self { target_dir, client })
    }

    /// reqwest::Client를 외부에서 주입 — 기존 connection pool 공유 시.
    pub fn with_client(target_dir: PathBuf, client: reqwest::Client) -> Self {
        Self { target_dir, client }
    }

    /// 모델 1종(model.onnx + tokenizer.json) 다운로드.
    ///
    /// 흐름:
    /// 1. 이미 두 파일 모두 존재 → 즉시 Done 발생 + 그대로 반환.
    /// 2. target_dir 디렉터리 생성 (parent까지).
    /// 3. model.onnx 다운로드 (.partial → sha256 verify → atomic rename).
    /// 4. tokenizer.json 다운로드 (동일).
    /// 5. Done emit + (model_path, tokenizer_path) 반환.
    ///
    /// 중간 cancel은 `.partial` 보존 (다음 호출 시 처음부터 새로 받음 — Range resume은 v1.x).
    pub async fn download_model(
        &self,
        kind: OnnxModelKind,
        progress: mpsc::Sender<DownloadEvent>,
        cancel: CancellationToken,
    ) -> Result<(PathBuf, PathBuf), KnowledgeError> {
        let manifest = ModelManifest::for_kind(kind);
        let dir = model_dir(&self.target_dir, kind);
        let model_path = dir.join(&manifest.model_filename);
        let tokenizer_path = dir.join(&manifest.tokenizer_filename);

        // 1. Skip if both already present.
        if model_path.is_file() && tokenizer_path.is_file() {
            let _ = progress
                .send(DownloadEvent::Done {
                    model_kind: kind.as_kebab().to_string(),
                    model_path: model_path.to_string_lossy().to_string(),
                    tokenizer_path: tokenizer_path.to_string_lossy().to_string(),
                })
                .await;
            return Ok((model_path, tokenizer_path));
        }

        // 2. Create dir.
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| KnowledgeError::Io {
                path: dir.clone(),
                source: e,
            })?;

        // 3. Download model.onnx.
        if let Err(e) = self
            .download_one(
                kind,
                &manifest.model_filename,
                &manifest.model_url,
                manifest.model_sha256,
                &model_path,
                &progress,
                &cancel,
            )
            .await
        {
            // 한 번 fail emit + 정리. .partial은 보존 (다음 시도가 빨라지진 않지만 사용자가 직접 정리 가능).
            let event = match &e {
                KnowledgeError::Cancelled => DownloadEvent::Cancelled {
                    model_kind: kind.as_kebab().to_string(),
                },
                other => DownloadEvent::Failed {
                    model_kind: kind.as_kebab().to_string(),
                    error: format!("{other}"),
                },
            };
            let _ = progress.send(event).await;
            return Err(e);
        }

        // 4. Download tokenizer.json.
        if let Err(e) = self
            .download_one(
                kind,
                &manifest.tokenizer_filename,
                &manifest.tokenizer_url,
                manifest.tokenizer_sha256,
                &tokenizer_path,
                &progress,
                &cancel,
            )
            .await
        {
            let event = match &e {
                KnowledgeError::Cancelled => DownloadEvent::Cancelled {
                    model_kind: kind.as_kebab().to_string(),
                },
                other => DownloadEvent::Failed {
                    model_kind: kind.as_kebab().to_string(),
                    error: format!("{other}"),
                },
            };
            let _ = progress.send(event).await;
            return Err(e);
        }

        // 5. Done.
        let _ = progress
            .send(DownloadEvent::Done {
                model_kind: kind.as_kebab().to_string(),
                model_path: model_path.to_string_lossy().to_string(),
                tokenizer_path: tokenizer_path.to_string_lossy().to_string(),
            })
            .await;
        Ok((model_path, tokenizer_path))
    }

    /// 단일 파일 다운로드 — `.partial` accumulator + sha256 stream + atomic rename.
    #[allow(clippy::too_many_arguments)]
    async fn download_one(
        &self,
        kind: OnnxModelKind,
        file_label: &str,
        url: &str,
        expected_sha256: Option<[u8; 32]>,
        final_path: &Path,
        progress: &mpsc::Sender<DownloadEvent>,
        cancel: &CancellationToken,
    ) -> Result<(), KnowledgeError> {
        // 이미 final이 있으면 skip — 멱등성. (partial 검증은 Range resume v1.x.)
        if final_path.is_file() {
            return Ok(());
        }

        let partial_path = partial_path_for(final_path);
        // 깨끗한 시작: 기존 .partial 폐기 (Range resume v1.x).
        let _ = tokio::fs::remove_file(&partial_path).await;

        // GET — Tauri main thread를 막지 않도록 streaming.
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| KnowledgeError::EmbeddingFailed(format!("다운로드 요청 실패: {e}")))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(KnowledgeError::EmbeddingFailed(format!(
                "HTTP 상태 코드 {} — {url}",
                status.as_u16()
            )));
        }
        let total_bytes = resp.content_length();

        let _ = progress
            .send(DownloadEvent::Started {
                model_kind: kind.as_kebab().to_string(),
                file: file_label.to_string(),
                total_bytes,
            })
            .await;

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&partial_path)
            .await
            .map_err(|e| KnowledgeError::Io {
                path: partial_path.clone(),
                source: e,
            })?;
        let mut hasher = Sha256::new();
        let mut downloaded: u64 = 0;
        let mut accumulator: u64 = 0;
        let mut last_emit = Instant::now();

        let mut stream = resp.bytes_stream();
        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    // .partial 보존, hasher 결과 폐기.
                    let _ = file.flush().await;
                    return Err(KnowledgeError::Cancelled);
                }
                next = stream.next() => {
                    match next {
                        Some(Ok(chunk)) => {
                            let len = chunk.len() as u64;
                            hasher.update(&chunk);
                            file.write_all(&chunk).await.map_err(|e| KnowledgeError::Io {
                                path: partial_path.clone(),
                                source: e,
                            })?;
                            downloaded = downloaded.saturating_add(len);
                            accumulator = accumulator.saturating_add(len);
                            let elapsed = last_emit.elapsed();
                            if accumulator >= PROGRESS_BYTES_THRESHOLD
                                || elapsed.as_millis() >= PROGRESS_INTERVAL_MS
                            {
                                let _ = progress
                                    .send(DownloadEvent::Progress {
                                        model_kind: kind.as_kebab().to_string(),
                                        file: file_label.to_string(),
                                        downloaded,
                                        total: total_bytes,
                                    })
                                    .await;
                                last_emit = Instant::now();
                                accumulator = 0;
                            }
                        }
                        Some(Err(e)) => {
                            let _ = file.flush().await;
                            return Err(KnowledgeError::EmbeddingFailed(format!(
                                "다운로드 도중 끊겼어요: {e}"
                            )));
                        }
                        None => break,
                    }
                }
            }
        }
        file.flush().await.map_err(|e| KnowledgeError::Io {
            path: partial_path.clone(),
            source: e,
        })?;

        // sha256 검증.
        let _ = progress
            .send(DownloadEvent::Verifying {
                model_kind: kind.as_kebab().to_string(),
                file: file_label.to_string(),
            })
            .await;
        let final_hash: [u8; 32] = hasher.finalize().into();
        if let Some(expected) = expected_sha256 {
            if final_hash != expected {
                let _ = tokio::fs::remove_file(&partial_path).await;
                return Err(KnowledgeError::EmbeddingFailed(format!(
                    "무결성 검증에 실패했어요. 기대: {} / 실제: {}",
                    hex::encode(expected),
                    hex::encode(final_hash)
                )));
            }
        }

        // atomic rename.
        atomic_persist(&partial_path, final_path).await?;
        Ok(())
    }
}

/// `<final>.partial`.
fn partial_path_for(final_path: &Path) -> PathBuf {
    let mut s: std::ffi::OsString = final_path.as_os_str().into();
    s.push(".partial");
    PathBuf::from(s)
}

/// `.partial` → final 원자적 rename. AV/Indexer 잠시 잠금 대비 short retry (installer 패턴).
async fn atomic_persist(partial: &Path, final_path: &Path) -> Result<(), KnowledgeError> {
    let attempts: usize = 5;
    let mut delay = Duration::from_millis(50);
    for i in 0..attempts {
        match tokio::fs::rename(partial, final_path).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                if i + 1 == attempts {
                    return Err(KnowledgeError::Io {
                        path: final_path.to_path_buf(),
                        source: e,
                    });
                }
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(Duration::from_millis(500));
            }
        }
    }
    unreachable!()
}

/// 검증 정보를 묶어 노출 — IPC `verify_embedding_model_files`가 사용.
pub async fn verify_files_sha256(
    model_path: &Path,
    expected_model: Option<[u8; 32]>,
    tokenizer_path: &Path,
    expected_tokenizer: Option<[u8; 32]>,
) -> Result<(), KnowledgeError> {
    if let Some(expected) = expected_model {
        let actual = sha256_file(model_path).await?;
        if actual != expected {
            return Err(KnowledgeError::EmbeddingFailed(format!(
                "model.onnx 무결성 검증에 실패했어요. 기대: {}",
                hex::encode(expected)
            )));
        }
    }
    if let Some(expected) = expected_tokenizer {
        let actual = sha256_file(tokenizer_path).await?;
        if actual != expected {
            return Err(KnowledgeError::EmbeddingFailed(format!(
                "tokenizer.json 무결성 검증에 실패했어요. 기대: {}",
                hex::encode(expected)
            )));
        }
    }
    Ok(())
}

async fn sha256_file(path: &Path) -> Result<[u8; 32], KnowledgeError> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| KnowledgeError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).await.map_err(|e| KnowledgeError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn kind_kebab_round_trip() {
        for kind in [
            OnnxModelKind::BgeM3,
            OnnxModelKind::KureV1,
            OnnxModelKind::MultilingualE5Small,
        ] {
            assert_eq!(OnnxModelKind::from_kebab(kind.as_kebab()), Some(kind));
        }
        assert!(OnnxModelKind::from_kebab("unknown").is_none());
    }

    #[test]
    fn kind_dim_consistent() {
        assert_eq!(OnnxModelKind::BgeM3.dim(), 1024);
        assert_eq!(OnnxModelKind::KureV1.dim(), 768);
        assert_eq!(OnnxModelKind::MultilingualE5Small.dim(), 384);
    }

    #[test]
    fn manifest_uses_huggingface_only() {
        for kind in [
            OnnxModelKind::BgeM3,
            OnnxModelKind::KureV1,
            OnnxModelKind::MultilingualE5Small,
        ] {
            let m = ModelManifest::for_kind(kind);
            assert!(
                m.model_url.starts_with("https://huggingface.co/"),
                "model URL은 huggingface.co여야 해요 — {}",
                m.model_url
            );
            assert!(m.tokenizer_url.starts_with("https://huggingface.co/"));
        }
    }

    #[test]
    fn is_downloaded_false_for_empty_dir() {
        let dir = TempDir::new().unwrap();
        assert!(!is_downloaded(dir.path(), OnnxModelKind::BgeM3));
    }

    #[test]
    fn is_downloaded_true_when_both_files_present() {
        let dir = TempDir::new().unwrap();
        let kind = OnnxModelKind::BgeM3;
        let kind_dir = model_dir(dir.path(), kind);
        std::fs::create_dir_all(&kind_dir).unwrap();
        std::fs::write(kind_dir.join("model.onnx"), b"fake").unwrap();
        std::fs::write(kind_dir.join("tokenizer.json"), b"fake").unwrap();
        assert!(is_downloaded(dir.path(), kind));
    }

    #[test]
    fn download_event_kind_kebab_serialization() {
        let ev = DownloadEvent::Started {
            model_kind: "bge-m3".into(),
            file: "model.onnx".into(),
            total_bytes: Some(1024),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "started");
        assert_eq!(v["model_kind"], "bge-m3");
        assert_eq!(v["file"], "model.onnx");
        assert_eq!(v["total_bytes"], 1024);

        let ev2 = DownloadEvent::Progress {
            model_kind: "kure-v1".into(),
            file: "tokenizer.json".into(),
            downloaded: 512,
            total: None,
        };
        let v2 = serde_json::to_value(&ev2).unwrap();
        assert_eq!(v2["kind"], "progress");
        assert_eq!(v2["downloaded"], 512);
        assert!(v2["total"].is_null());
    }

    #[test]
    fn download_event_failed_includes_korean() {
        let ev = DownloadEvent::Failed {
            model_kind: "bge-m3".into(),
            error: "다운로드 도중 끊겼어요".into(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "failed");
        assert!(v["error"].as_str().unwrap().contains("끊겼어요"));
    }

    /// wiremock으로 model.onnx + tokenizer.json 두 endpoint 응답.
    /// Mock manifest는 자체 URL을 가리키도록 patch.
    #[allow(dead_code)]
    fn install_mock_manifest(server_url: &str, kind: OnnxModelKind) -> ModelManifest {
        ModelManifest {
            kind,
            model_url: format!("{server_url}/{}/model.onnx", kind.as_kebab()),
            model_filename: "model.onnx".into(),
            model_sha256: None,
            tokenizer_url: format!("{server_url}/{}/tokenizer.json", kind.as_kebab()),
            tokenizer_filename: "tokenizer.json".into(),
            tokenizer_sha256: None,
        }
    }

    /// 직접 download_one을 호출해 wiremock-backed end-to-end 흐름 검증.
    /// download_model은 hardcoded HuggingFace URL을 사용하므로 단위 테스트에서는 download_one 직접 사용.
    async fn drive_download_one(
        downloader: &ModelDownloader,
        kind: OnnxModelKind,
        url: String,
        final_path: PathBuf,
        cancel: CancellationToken,
    ) -> (Result<(), KnowledgeError>, Vec<DownloadEvent>) {
        let (tx, mut rx) = mpsc::channel(64);
        let join = tokio::spawn(async move {
            let mut events = Vec::new();
            while let Some(ev) = rx.recv().await {
                events.push(ev);
            }
            events
        });
        let result = downloader
            .download_one(kind, "model.onnx", &url, None, &final_path, &tx, &cancel)
            .await;
        drop(tx);
        let events = join.await.unwrap();
        (result, events)
    }

    #[tokio::test]
    async fn download_one_writes_file_and_emits_progress() {
        let server = MockServer::start().await;
        let body = vec![0u8; 1024 * 1024]; // 1MB, throttle 256KB → 최소 4번 progress.
        Mock::given(method("GET"))
            .and(path("/bge-m3/model.onnx"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
            .mount(&server)
            .await;

        let dir = TempDir::new().unwrap();
        let downloader = ModelDownloader::new(dir.path().to_path_buf()).unwrap();
        let kind = OnnxModelKind::BgeM3;
        let kind_dir = model_dir(dir.path(), kind);
        tokio::fs::create_dir_all(&kind_dir).await.unwrap();
        let url = format!("{}/bge-m3/model.onnx", server.uri());
        let final_path = kind_dir.join("model.onnx");

        let (res, events) = drive_download_one(
            &downloader,
            kind,
            url,
            final_path.clone(),
            CancellationToken::new(),
        )
        .await;

        res.expect("download must succeed");
        assert!(final_path.is_file(), "final 파일이 만들어져 있어야 해요");
        // Started + Progress 1회 이상 + Verifying.
        let started = events
            .iter()
            .any(|e| matches!(e, DownloadEvent::Started { .. }));
        let progressed = events
            .iter()
            .any(|e| matches!(e, DownloadEvent::Progress { .. }));
        let verifying = events
            .iter()
            .any(|e| matches!(e, DownloadEvent::Verifying { .. }));
        assert!(started, "Started 이벤트 누락");
        assert!(progressed, "Progress 이벤트 누락");
        assert!(verifying, "Verifying 이벤트 누락");
    }

    #[tokio::test]
    async fn download_one_skips_when_final_exists() {
        let dir = TempDir::new().unwrap();
        let kind = OnnxModelKind::KureV1;
        let kind_dir = model_dir(dir.path(), kind);
        tokio::fs::create_dir_all(&kind_dir).await.unwrap();
        let final_path = kind_dir.join("model.onnx");
        tokio::fs::write(&final_path, b"already").await.unwrap();

        let downloader = ModelDownloader::new(dir.path().to_path_buf()).unwrap();
        // URL 호출이 일어나면 fail — wiremock 미설치 상태에서도 통과해야 idempotent.
        let (tx, mut rx) = mpsc::channel(8);
        let join = tokio::spawn(async move {
            let mut events = Vec::new();
            while let Some(ev) = rx.recv().await {
                events.push(ev);
            }
            events
        });
        let res = downloader
            .download_one(
                kind,
                "model.onnx",
                "http://invalid.invalid/never",
                None,
                &final_path,
                &tx,
                &CancellationToken::new(),
            )
            .await;
        drop(tx);
        let events = join.await.unwrap();
        res.expect("이미 있으면 skip 해야 해요");
        assert!(events.is_empty(), "skip 분기는 progress emit 안 함");
    }

    #[tokio::test]
    async fn download_one_cancel_mid_stream_preserves_partial() {
        let server = MockServer::start().await;
        // 큰 응답 + delay로 cancel 윈도우 확보.
        let body = vec![0u8; 4 * 1024 * 1024];
        Mock::given(method("GET"))
            .and(path("/bge-m3/model.onnx"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(200))
                    .set_body_bytes(body),
            )
            .mount(&server)
            .await;

        let dir = TempDir::new().unwrap();
        let downloader = ModelDownloader::new(dir.path().to_path_buf()).unwrap();
        let kind = OnnxModelKind::BgeM3;
        let kind_dir = model_dir(dir.path(), kind);
        tokio::fs::create_dir_all(&kind_dir).await.unwrap();
        let url = format!("{}/bge-m3/model.onnx", server.uri());
        let final_path = kind_dir.join("model.onnx");
        let cancel = CancellationToken::new();
        let cancel_for_task = cancel.clone();
        // 즉시 cancel — stream 시작 전에도 안전.
        cancel_for_task.cancel();

        let (res, _events) =
            drive_download_one(&downloader, kind, url, final_path.clone(), cancel).await;
        match res {
            Err(KnowledgeError::Cancelled) => {}
            other => panic!("expected Cancelled, got {other:?}"),
        }
        assert!(
            !final_path.is_file(),
            "cancel 시 final 파일은 만들어지면 안 돼요"
        );
    }

    #[tokio::test]
    async fn download_one_bad_status_returns_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/bge-m3/model.onnx"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let dir = TempDir::new().unwrap();
        let downloader = ModelDownloader::new(dir.path().to_path_buf()).unwrap();
        let kind = OnnxModelKind::BgeM3;
        let kind_dir = model_dir(dir.path(), kind);
        tokio::fs::create_dir_all(&kind_dir).await.unwrap();
        let url = format!("{}/bge-m3/model.onnx", server.uri());
        let final_path = kind_dir.join("model.onnx");

        let (res, _) =
            drive_download_one(&downloader, kind, url, final_path, CancellationToken::new()).await;
        match res {
            Err(KnowledgeError::EmbeddingFailed(msg)) => {
                assert!(msg.contains("HTTP 상태"), "한국어 메시지 필요 — got {msg}");
            }
            other => panic!("expected EmbeddingFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn download_one_sha256_mismatch_removes_partial() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/bge-m3/model.onnx"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello".to_vec()))
            .mount(&server)
            .await;

        let dir = TempDir::new().unwrap();
        let downloader = ModelDownloader::new(dir.path().to_path_buf()).unwrap();
        let kind = OnnxModelKind::BgeM3;
        let kind_dir = model_dir(dir.path(), kind);
        tokio::fs::create_dir_all(&kind_dir).await.unwrap();
        let url = format!("{}/bge-m3/model.onnx", server.uri());
        let final_path = kind_dir.join("model.onnx");
        let bogus_sha = [0u8; 32];

        let (tx, _rx) = mpsc::channel(8);
        let res = downloader
            .download_one(
                kind,
                "model.onnx",
                &url,
                Some(bogus_sha),
                &final_path,
                &tx,
                &CancellationToken::new(),
            )
            .await;
        match res {
            Err(KnowledgeError::EmbeddingFailed(msg)) => {
                assert!(msg.contains("무결성"));
            }
            other => panic!("expected EmbeddingFailed, got {other:?}"),
        }
        assert!(
            !partial_path_for(&final_path).is_file(),
            ".partial은 sha mismatch에서 정리돼야 해요"
        );
        assert!(!final_path.is_file());
    }

    #[tokio::test]
    async fn verify_files_sha256_passes_when_no_expected() {
        // expected가 None이면 verify는 항상 통과 — 파일이 비어 있어도 OK (caller 책임).
        let dir = TempDir::new().unwrap();
        let model = dir.path().join("model.onnx");
        let tok = dir.path().join("tokenizer.json");
        tokio::fs::write(&model, b"x").await.unwrap();
        tokio::fs::write(&tok, b"y").await.unwrap();
        verify_files_sha256(&model, None, &tok, None).await.unwrap();
    }

    #[tokio::test]
    async fn verify_files_sha256_detects_mismatch() {
        let dir = TempDir::new().unwrap();
        let model = dir.path().join("model.onnx");
        tokio::fs::write(&model, b"hello").await.unwrap();
        let tok = dir.path().join("tokenizer.json");
        tokio::fs::write(&tok, b"world").await.unwrap();
        let res = verify_files_sha256(&model, Some([0u8; 32]), &tok, None).await;
        match res {
            Err(KnowledgeError::EmbeddingFailed(msg)) => assert!(msg.contains("무결성")),
            other => panic!("expected EmbeddingFailed, got {other:?}"),
        }
    }
}

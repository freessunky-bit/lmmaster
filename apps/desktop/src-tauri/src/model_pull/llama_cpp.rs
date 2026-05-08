//! Phase 13'.h.2.c.2 — LlamaCpp 모델 자동 다운로드.
//!
//! 정책 (`docs/research/phase-13ph2c2-llama-cpp-auto-download-decision.md`):
//! - `installer::Downloader` 재사용 (sha256 + atomic + backon + .partial resume + .no_proxy).
//! - 메인 GGUF + mmproj (vision 모델만) 두 차례 다운로드.
//! - cache_dir = `app_local_data_dir()/models`. `create_dir_all` idempotent mkdir.
//! - 진행 이벤트는 `DownloadEvent` → `ModelPullEvent` 변환 closure로 emit.

use std::path::Path;

use adapter_ollama::ModelPullEvent;
use installer::downloader::{DownloadRequest, Downloader};
use installer::progress::{DownloadEvent, ProgressSink};
use model_registry::manifest::ModelEntry;
use tauri::ipc::Channel;
use tokio_util::sync::CancellationToken;

use super::ModelPullApiError;
use crate::chat::llama_cpp::{derive_main_url, mmproj_filename, model_filename};

/// 메인 진입점 — `start_model_pull(LlamaCpp)`이 호출.
///
/// 흐름: cache_dir mkdir → quant 선택 (default first) → 메인 GGUF 다운로드 →
/// mmproj 다운로드(있으면) → Completed event.
///
/// 진행 이벤트는 동일 channel을 통해 ModelPullEvent로 emit. cancel cascade는 Channel close 시.
pub async fn pull_llama_model(
    entry: &ModelEntry,
    cache_dir: &Path,
    channel: &Channel<ModelPullEvent>,
    cancel: &CancellationToken,
) -> Result<(), ModelPullApiError> {
    // 1. cache_dir mkdir (idempotent).
    std::fs::create_dir_all(cache_dir).map_err(|e| ModelPullApiError::Internal {
        message: format!("cache 디렉터리를 만들지 못했어요: {e}"),
    })?;

    // 2. quant 선택 — default first (보통 Q4_K_M).
    let quant = entry
        .quantization_options
        .first()
        .ok_or_else(|| ModelPullApiError::Internal {
            message: format!("모델 카탈로그에 quant 옵션이 없어요: {}", entry.id),
        })?;

    let main_url = derive_main_url(entry, quant).ok_or_else(|| ModelPullApiError::Internal {
        message: format!("다운로드 URL을 만들 수 없어요: {}", entry.id),
    })?;
    let main_filename = model_filename(entry);
    let main_path = cache_dir.join(&main_filename);

    // 3. Downloader 초기화 (.no_proxy + 30분 timeout).
    let downloader = Downloader::new().map_err(|e| ModelPullApiError::Internal {
        message: format!("다운로더 초기화 실패: {e}"),
    })?;

    // 4. 메인 GGUF 다운로드.
    let main_sha =
        parse_optional_sha256(&quant.sha256).map_err(|e| ModelPullApiError::Internal {
            message: format!("sha256 파싱 실패 (quant {}): {e}", quant.label),
        })?;
    let main_req = DownloadRequest {
        url: main_url,
        final_path: main_path.clone(),
        expected_sha256: main_sha,
        size_hint: Some(quant.size_mb.saturating_mul(1024 * 1024)),
        max_retries: None,
    };
    let main_sink = make_sink(channel.clone(), cancel.clone(), main_filename.clone());
    let _ = channel.send(ModelPullEvent::Status {
        status: format!("받기 시작: {}", main_filename),
    });
    downloader
        .download(&main_req, cancel, &main_sink)
        .await
        .map_err(|e| ModelPullApiError::Internal {
            message: format!("모델 다운로드 실패: {e}"),
        })?;

    // 5. mmproj 다운로드 (vision 모델만).
    if let Some(mmproj) = entry.mmproj.as_ref() {
        let mmproj_name = mmproj_filename(mmproj, &entry.id);
        let mmproj_path = cache_dir.join(&mmproj_name);
        let mmproj_sha = match mmproj.sha256.as_deref() {
            Some(s) => parse_optional_sha256(s).map_err(|e| ModelPullApiError::Internal {
                message: format!("mmproj sha256 파싱 실패: {e}"),
            })?,
            None => None,
        };
        let mmproj_req = DownloadRequest {
            url: mmproj.url.clone(),
            final_path: mmproj_path,
            expected_sha256: mmproj_sha,
            size_hint: Some(mmproj.size_mb.saturating_mul(1024 * 1024)),
            max_retries: None,
        };
        let mmproj_sink = make_sink(channel.clone(), cancel.clone(), mmproj_name.clone());
        let _ = channel.send(ModelPullEvent::Status {
            status: format!("mmproj 받기 시작: {}", mmproj_name),
        });
        downloader
            .download(&mmproj_req, cancel, &mmproj_sink)
            .await
            .map_err(|e| ModelPullApiError::Internal {
                message: format!("mmproj 다운로드 실패: {e}"),
            })?;
    }

    Ok(())
}

/// 64-hex sha256 → `[u8; 32]`. 길이/문자 검증.
fn parse_sha256_hex(s: &str) -> Result<[u8; 32], String> {
    if s.len() != 64 {
        return Err(format!("sha256 길이가 64가 아니에요: {}", s.len()));
    }
    let mut out = [0u8; 32];
    hex::decode_to_slice(s, &mut out).map_err(|e| format!("hex 디코드 실패: {e}"))?;
    Ok(out)
}

/// placeholder (빈 문자열 또는 `"0".repeat(64)`)는 None — 검증 skip.
/// 큐레이터가 sha256을 채우지 않은 manifest 호환 (40+ entries).
fn parse_optional_sha256(s: &str) -> Result<Option<[u8; 32]>, String> {
    if s.is_empty() || s.chars().all(|c| c == '0') {
        return Ok(None);
    }
    parse_sha256_hex(s).map(Some)
}

/// `DownloadEvent` → `ModelPullEvent` 변환 sink. Channel close 시 cancel cascade (R-E.6).
fn make_sink(
    channel: Channel<ModelPullEvent>,
    cancel: CancellationToken,
    filename: String,
) -> impl ProgressSink {
    move |event: DownloadEvent| {
        let pull_event = match event {
            DownloadEvent::Started { total, .. } => ModelPullEvent::Status {
                status: match total {
                    Some(t) => format!("{} ({:.1} MB)", filename, (t as f64) / 1_048_576.0),
                    None => format!("{} 시작", filename),
                },
            },
            DownloadEvent::Progress {
                downloaded,
                total,
                speed_bps,
            } => ModelPullEvent::Progress {
                completed_bytes: downloaded,
                total_bytes: total.unwrap_or(0),
                speed_bps,
                eta_secs: total.and_then(|t| {
                    let remaining = t.saturating_sub(downloaded);
                    remaining.checked_div(speed_bps)
                }),
            },
            DownloadEvent::Verified { .. } => ModelPullEvent::Status {
                status: format!("{} sha256 검증 완료", filename),
            },
            DownloadEvent::Finished { .. } => ModelPullEvent::Status {
                status: format!("{} 저장 완료", filename),
            },
            DownloadEvent::Retrying {
                attempt,
                delay_ms,
                reason,
            } => ModelPullEvent::Status {
                status: format!("재시도 (#{attempt}, {delay_ms}ms): {reason}"),
            },
        };
        if channel.send(pull_event).is_err() {
            tracing::debug!(
                filename = %filename,
                "model_pull channel closed — cancelling backend"
            );
            cancel.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sha256_hex_valid() {
        let s = "a".repeat(64);
        let bytes = parse_sha256_hex(&s).unwrap();
        assert_eq!(bytes.len(), 32);
        assert_eq!(bytes[0], 0xaa);
    }

    #[test]
    fn parse_sha256_hex_wrong_length() {
        let s = "a".repeat(63);
        let err = parse_sha256_hex(&s).unwrap_err();
        assert!(err.contains("길이가 64가 아니에요"));
    }

    #[test]
    fn parse_sha256_hex_invalid_chars() {
        let s = "z".repeat(64);
        let err = parse_sha256_hex(&s).unwrap_err();
        assert!(err.contains("hex 디코드 실패"));
    }

    #[test]
    fn parse_optional_sha256_placeholder_returns_none() {
        let placeholder = "0".repeat(64);
        let result = parse_optional_sha256(&placeholder).unwrap();
        assert!(result.is_none(), "placeholder 0...0은 None이어야 해요");
    }

    #[test]
    fn parse_optional_sha256_empty_returns_none() {
        let result = parse_optional_sha256("").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn parse_optional_sha256_real_returns_some() {
        let real = "882e8d2db44dc554fb0ea5077cb7e4bc49e7342a1f0da57901c0802ea21a0863";
        let result = parse_optional_sha256(real).unwrap();
        assert!(result.is_some());
    }
}

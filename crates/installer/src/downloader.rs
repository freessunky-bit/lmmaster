//! Resumable HTTP 다운로더 — streaming sha256 + Range resume + backon retry + atomic rename.
//!
//! 정책 (ADR-0017, Phase 1A 보강 §2, ADR-0021):
//! - 큰 파일(installer 100~500MB, GGUF 1~7GB)은 reqwest::stream() 으로 chunk 단위 처리.
//! - `.partial` 임시 파일에 누적 → sha256 stream → atomic rename to final.
//! - 취소 시 `.partial` 보존 (다음 실행 resume 가능). hash mismatch만 .partial 제거.
//! - retry는 backon 1.6의 jitter exponential. 단 cancel/hash mismatch는 fatal.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use backon::{ExponentialBuilder, Retryable};
use bytes::Bytes;
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

use crate::error::DownloadError;
use crate::progress::{DownloadEvent, ProgressSink};

const PROGRESS_BYTES_THRESHOLD: u64 = 256 * 1024;
const PROGRESS_INTERVAL_MS: u128 = 100;

/// 다운로드 요청.
#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub url: String,
    /// 검증 성공 후 atomic rename할 최종 경로. 부모 디렉터리는 사전 존재 가정 (호출자가 보장).
    pub final_path: PathBuf,
    /// 32-byte sha256. None이면 검증 skip (위험 — 가급적 항상 제공).
    pub expected_sha256: Option<[u8; 32]>,
    /// Content-Length가 없는 서버 대비 hint. 진행률 표시용.
    pub size_hint: Option<u64>,
    /// 최대 retry 횟수 (default 5).
    pub max_retries: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct DownloadOutcome {
    pub final_path: PathBuf,
    pub bytes: u64,
    pub sha256_hex: String,
    /// resume으로 시작했는가 (.partial이 이미 존재했음).
    pub resumed: bool,
}

pub struct Downloader {
    client: reqwest::Client,
}

impl Downloader {
    /// 자체 reqwest::Client 생성. 기본 user-agent + connection pool.
    ///
    /// Phase R-C (ADR-0055) — .no_proxy() 강제 (시스템 HTTP_PROXY/HTTPS_PROXY 무시).
    /// 모델 다운로드는 화이트리스트 호스트(HuggingFace 등)로만 허용 + rogue proxy MITM 방어.
    pub fn new() -> Result<Self, DownloadError> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .user_agent(format!("LMmaster-installer/{}", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(60 * 30)) // 큰 모델 대비 30분.
            .connect_timeout(Duration::from_secs(15))
            .pool_idle_timeout(Duration::from_secs(60))
            .build()?;
        Ok(Self { client })
    }

    /// 외부에서 만든 client를 주입 (Detector 등과 connection pool 공유 시).
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// 메인 진입점. cancel 토큰이 cancelled되면 즉시 .partial 보존하고 Err(Cancelled).
    pub async fn download<S>(
        &self,
        req: &DownloadRequest,
        cancel: &CancellationToken,
        sink: &S,
    ) -> Result<DownloadOutcome, DownloadError>
    where
        S: ProgressSink,
    {
        if req.url.is_empty() {
            return Err(DownloadError::InvalidRequest("empty url".into()));
        }

        let parent = req.final_path.parent().ok_or_else(|| {
            DownloadError::InvalidRequest(format!(
                "final_path has no parent: {}",
                req.final_path.display()
            ))
        })?;
        if !parent.exists() {
            return Err(DownloadError::InvalidRequest(format!(
                "parent dir not found: {}",
                parent.display()
            )));
        }

        let partial_path = partial_path_for(&req.final_path);
        let max_retries = req.max_retries.unwrap_or(5);

        let outcome = (|| async {
            self.attempt_download(req, &partial_path, cancel, sink)
                .await
        })
        .retry(
            ExponentialBuilder::default()
                .with_min_delay(Duration::from_millis(500))
                .with_max_delay(Duration::from_secs(8))
                .with_max_times(max_retries as usize)
                .with_jitter(),
        )
        .when(|e: &DownloadError| e.is_retryable())
        .notify(|err, dur| {
            tracing::warn!(error = %err, retry_after_ms = dur.as_millis() as u64, "download retry");
            sink.emit(DownloadEvent::Retrying {
                attempt: 0, // backon은 attempt index를 직접 제공하지 않음 — 0 stub.
                delay_ms: dur.as_millis() as u64,
                reason: err.to_string(),
            });
        })
        .await?;

        Ok(outcome)
    }

    async fn attempt_download<S: ProgressSink>(
        &self,
        req: &DownloadRequest,
        partial_path: &Path,
        cancel: &CancellationToken,
        sink: &S,
    ) -> Result<DownloadOutcome, DownloadError> {
        let resume_from = match tokio::fs::metadata(partial_path).await {
            Ok(meta) => meta.len(),
            Err(_) => 0,
        };
        let resumed = resume_from > 0;

        // hasher: resume 시 기존 .partial 바이트를 먼저 hashing.
        let mut hasher = Sha256::new();
        if resume_from > 0 {
            let mut f = tokio::fs::File::open(partial_path).await?;
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                let n = f.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
            }
        }

        // Range request.
        let mut request = self.client.get(&req.url);
        if resume_from > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={}-", resume_from));
        }
        let resp = request.send().await?;

        let status = resp.status();
        let server_supports_range = status.as_u16() == 206;
        if !status.is_success() {
            return Err(DownloadError::BadStatus {
                status: status.as_u16(),
                url: req.url.clone(),
            });
        }

        // 서버가 Range를 무시(200) 했다면 hasher + .partial 리셋.
        let (mut resume_from, mut hasher) = if !server_supports_range && resume_from > 0 {
            tracing::info!("server ignored Range header (status 200) — restarting from byte 0");
            // .partial 잘라내기 + hasher 재초기화.
            tokio::fs::File::create(partial_path).await?; // truncate
            (0u64, Sha256::new())
        } else {
            (resume_from, hasher)
        };

        // total = Content-Length + resume_from (for 206) 또는 Content-Length (for 200, after reset).
        let content_length = resp.content_length();
        let total = content_length.map(|cl| {
            if server_supports_range {
                resume_from + cl
            } else {
                cl
            }
        });

        sink.emit(DownloadEvent::Started {
            url: req.url.clone(),
            total,
            resume_from,
        });

        // append/create 모드.
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true) // resume인 경우 기존 데이터 보존하며 추가.
            .open(partial_path)
            .await?;

        // server가 Range를 무시했으면 .partial이 이미 truncate됐으므로 append==write에서 0부터.
        let mut downloaded = resume_from;
        let mut accumulator: u64 = 0;
        let mut last_emit = Instant::now();
        let mut last_total_emitted = downloaded;

        let mut stream = resp.bytes_stream();
        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    file.flush().await.ok();
                    return Err(DownloadError::Cancelled);
                }
                next = stream.next() => {
                    match next {
                        Some(Ok(chunk)) => {
                            let len = chunk.len() as u64;
                            hasher.update(&chunk);
                            file.write_all(&chunk).await?;
                            downloaded = downloaded.saturating_add(len);
                            accumulator = accumulator.saturating_add(len);

                            // throttle: 256KB 또는 100ms.
                            let elapsed = last_emit.elapsed();
                            if accumulator >= PROGRESS_BYTES_THRESHOLD
                                || elapsed.as_millis() >= PROGRESS_INTERVAL_MS
                            {
                                let speed = compute_speed_bps(downloaded.saturating_sub(last_total_emitted), elapsed);
                                sink.emit(DownloadEvent::Progress {
                                    downloaded,
                                    total,
                                    speed_bps: speed,
                                });
                                last_emit = Instant::now();
                                last_total_emitted = downloaded;
                                accumulator = 0;
                            }
                        }
                        Some(Err(e)) => {
                            file.flush().await.ok();
                            return Err(DownloadError::Http(e));
                        }
                        None => break,
                    }
                }
            }
            // hasher에 borrow 충돌이 없는지 — Bytes는 Clone하지 않아도 함수에 &로 빌려줄 수 있음.
            // 하지만 위 구문이 `chunk`를 move하므로 Bytes는 단일 사용으로 OK.
            let _ = &mut hasher; // hint to compiler.
            let _ = &mut resume_from; // unused but kept as intent placeholder.
        }
        file.flush().await?;

        // sha256 검증.
        let final_hash: [u8; 32] = hasher.finalize().into();
        let final_hex = hex::encode(final_hash);
        if let Some(expected) = req.expected_sha256 {
            if final_hash != expected {
                let expected_hex = hex::encode(expected);
                tracing::warn!(
                    expected = %expected_hex,
                    actual = %final_hex,
                    "sha256 mismatch — removing .partial"
                );
                let _ = tokio::fs::remove_file(partial_path).await;
                return Err(DownloadError::HashMismatch {
                    expected: expected_hex,
                    actual: final_hex,
                });
            }
        }
        sink.emit(DownloadEvent::Verified {
            sha256_hex: final_hex.clone(),
        });

        // atomic rename: .partial → final. AV-locked 환경 대비 short retry.
        atomic_persist(partial_path, &req.final_path).await?;

        sink.emit(DownloadEvent::Finished {
            final_path: req.final_path.clone(),
            bytes: downloaded,
        });

        Ok(DownloadOutcome {
            final_path: req.final_path.clone(),
            bytes: downloaded,
            sha256_hex: final_hex,
            resumed,
        })
    }
}

/// `<final>.partial` 형태로 임시 경로 생성.
fn partial_path_for(final_path: &Path) -> PathBuf {
    let mut s: std::ffi::OsString = final_path.as_os_str().into();
    s.push(".partial");
    PathBuf::from(s)
}

fn compute_speed_bps(bytes: u64, elapsed: Duration) -> u64 {
    let secs = elapsed.as_secs_f64();
    if secs <= 0.001 {
        return 0;
    }
    (bytes as f64 / secs) as u64
}

/// `.partial` → final 원자적 rename. 같은 볼륨일 때 std::fs::rename은 atomic.
/// AV/Indexer가 잠시 잡고 있을 수 있으니 짧은 retry로 ride out.
async fn atomic_persist(partial: &Path, final_path: &Path) -> Result<(), DownloadError> {
    let partial = partial.to_path_buf();
    let final_path = final_path.to_path_buf();
    let attempts: usize = 5;
    let mut delay = Duration::from_millis(50);
    for i in 0..attempts {
        match tokio::fs::rename(&partial, &final_path).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                if i + 1 == attempts {
                    return Err(DownloadError::Io(e));
                }
                tracing::debug!(error = %e, attempt = i + 1, "rename failed — retry after {:?}", delay);
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(Duration::from_millis(500));
            }
        }
    }
    unreachable!()
}

// 사용하지 않는 import 경고 회피 (Bytes는 미래 chunk-by-chunk 처리에서 사용 가능).
#[allow(dead_code)]
fn _unused_bytes_import() -> Option<Bytes> {
    None
}

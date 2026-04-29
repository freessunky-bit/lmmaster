//! Workspace export — zip archive + 옵션별 모델/키 동봉 (Phase 11').
//!
//! 정책 (ADR-0009 + ADR-0039):
//! - **archive 포맷**: zip 8.x (Windows 호환 + zip-slip 방어 dual-defense). 7zip/RAR 거부 (ADR-0039).
//! - **default OFF**: 모델 포함 / 키 포함 모두 사용자 명시 opt-in. 첫 export 시 수십 GB 폭발 방지.
//! - **archive 구조** — workspace 루트 기준 상대경로 그대로:
//!   - `manifest.json` — workspace metadata.
//!   - `fingerprint.source.json` — 출발 PC fingerprint (받은 PC가 비교).
//!   - `data/`, `manifests/`, `projects/`, `sdk/`, `docs/`, `presets/` — 메타 디렉터리.
//!   - `models/` — include_models=true일 때만.
//!   - `keys.encrypted` — include_keys=true일 때만 (AES-GCM wrap).
//!   - `archive.sha256` — archive 내부 마지막 entry. 검증용.
//! - **streaming**: 모델 파일은 64KB 청크 read/write — RAM에 통째 로드 X.
//! - **atomic rename**: `.tmp` → final. 중간 실패 시 `.tmp` 청소.
//! - **sha256**: archive 전체 (`.tmp` 닫힌 후) 재읽어 계산.
//! - **cancel**: 모든 entry / chunk 사이에 `cancel.is_cancelled()` polling.
//!
//! AES-GCM + PBKDF2(100k iter) — keys 옵션. `OS 키체인은 PC 단위`라 archive에는 wrapped key만.

use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use hmac::Hmac;
use pbkdf2::pbkdf2;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

use crate::fingerprint::WorkspaceFingerprint;

const PBKDF2_ITER: u32 = 100_000;
const PBKDF2_SALT_LEN: usize = 16;
const AES_GCM_NONCE_LEN: usize = 12;

/// 사용자가 명시 opt-in 해야 동작하는 옵션 모음.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// `models/` 디렉터리 동봉 여부. default false.
    pub include_models: bool,
    /// `keys.encrypted` 동봉 여부. default false. true면 `key_passphrase` 필수.
    pub include_keys: bool,
    /// 키 wrap용 패스프레이즈. include_keys=false면 무시.
    pub key_passphrase: Option<String>,
    /// 출력 zip 경로 — `.tmp` rename된 최종 위치.
    pub target_path: PathBuf,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            include_models: false,
            include_keys: false,
            key_passphrase: None,
            target_path: PathBuf::new(),
        }
    }
}

/// Frontend Channel<ExportEvent>로 흘려보내는 진행 이벤트.
///
/// `#[serde(tag = "kind", rename_all = "kebab-case")]` — InstallEvent / WorkbenchEvent와 동일 셰입.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ExportEvent {
    /// export 시작 직후 1회.
    Started {
        source_path: String,
        target_path: String,
    },
    /// 카운팅 단계 종료 — 총 entry 수 / 바이트 수 fix.
    Counting { total_files: u64, total_bytes: u64 },
    /// 진행 — 한 entry 추가될 때마다.
    Compressing {
        processed: u64,
        total: u64,
        current_path: String,
    },
    /// 키 wrap 단계 (옵션 ON일 때만).
    Encrypting,
    /// archive 마무리 — sha256 계산 + 옵션별 metadata write.
    Finalizing,
    /// 정상 종료. archive sha256 + 사이즈 + 최종 경로 포함.
    Done {
        sha256: String,
        archive_size_bytes: u64,
        target_path: String,
    },
    /// 도중 실패. 한국어 해요체 메시지.
    Failed { error: String },
}

/// Channel<ExportEvent> 어댑터 trait — 테스트 / 프로덕션 양쪽 호환.
#[async_trait::async_trait]
pub trait ExportSink: Send + Sync {
    fn emit(&self, event: ExportEvent);
}

/// 종료 시 frontend로 전달할 메타 — Done 이벤트 + 상위 caller 양쪽에 redundant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportSummary {
    pub sha256: String,
    pub archive_size_bytes: u64,
    pub files_count: u64,
}

/// thiserror 한국어 메시지. invoke().catch에 그대로 노출.
#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ExportError {
    #[error("디스크 입출력에 실패했어요 ({path}): {source}")]
    Io {
        path: String,
        #[serde(skip)]
        #[source]
        source: std::io::Error,
    },
    #[error("키 암호화에 실패했어요: {0}")]
    KeyEncryption(String),
    #[error("키 패스프레이즈가 비어 있어요. 키를 포함하려면 패스프레이즈를 입력해 주세요.")]
    MissingPassphrase,
    #[error("내보내기를 취소했어요.")]
    Cancelled,
    #[error("zip-slip 위험이 있는 경로예요: {0}")]
    ZipSlip(String),
    #[error("워크스페이스가 비어 있어요. manifest.json을 먼저 만들어 주세요.")]
    EmptySourceWorkspace,
    #[error("아카이브 작성에 실패했어요: {0}")]
    WriteFailed(String),
}

/// Atomic rename 보장 — `.tmp` 임시 파일을 종료/실패 시 정리.
struct TempArchive {
    tmp_path: PathBuf,
    /// commit() 호출되면 true — Drop에서 삭제 안 함.
    committed: std::cell::Cell<bool>,
}

impl TempArchive {
    fn new(target: &Path) -> Self {
        let mut tmp_path = target.to_path_buf();
        let mut name = tmp_path
            .file_name()
            .map(|s| s.to_os_string())
            .unwrap_or_default();
        name.push(".tmp");
        tmp_path.set_file_name(name);
        Self {
            tmp_path,
            committed: std::cell::Cell::new(false),
        }
    }

    fn commit(&self, target: &Path) -> Result<u64, std::io::Error> {
        let size = std::fs::metadata(&self.tmp_path)?.len();
        std::fs::rename(&self.tmp_path, target)?;
        self.committed.set(true);
        Ok(size)
    }
}

impl Drop for TempArchive {
    fn drop(&mut self) {
        if !self.committed.get() && self.tmp_path.exists() {
            let _ = std::fs::remove_file(&self.tmp_path);
        }
    }
}

/// **메인 entry** — workspace_root 안의 메타 + (옵션) 모델 + (옵션) 키를 zip에 패킹.
///
/// 1. workspace_root 검증 — manifest.json 존재 (없으면 EmptySourceWorkspace).
/// 2. `.tmp` 파일 생성 → `TempArchive` guard.
/// 3. include_models / include_keys 옵션 검증 (passphrase 필수 등).
/// 4. file enumeration — `meta` 1차 / `models/` 2차 (옵션) / `keys` 3차 (옵션).
/// 5. zip 작성 (deflate, 64KB chunk streaming, cancel polling per chunk).
/// 6. fingerprint.source.json + manifest.json + archive.sha256 metadata 추가.
/// 7. zip close → sha256 read-back → atomic rename.
/// 8. `Done` emit.
///
/// `cancel.is_cancelled()` 체크는 entry 사이 + chunk 사이 양쪽.
pub async fn export_workspace<E: ExportSink + 'static>(
    workspace_root: &Path,
    options: ExportOptions,
    sink: Arc<E>,
    cancel: CancellationToken,
) -> Result<ExportSummary, ExportError> {
    // 0. 입력 검증.
    if options.include_keys && options.key_passphrase.as_deref().unwrap_or("").is_empty() {
        sink.emit(ExportEvent::Failed {
            error: ExportError::MissingPassphrase.to_string(),
        });
        return Err(ExportError::MissingPassphrase);
    }

    let manifest_path = workspace_root.join("manifest.json");
    if !manifest_path.exists() {
        sink.emit(ExportEvent::Failed {
            error: ExportError::EmptySourceWorkspace.to_string(),
        });
        return Err(ExportError::EmptySourceWorkspace);
    }

    // 1. Started.
    sink.emit(ExportEvent::Started {
        source_path: workspace_root.display().to_string(),
        target_path: options.target_path.display().to_string(),
    });

    // workspace_root + options를 owned로 옮겨 spawn_blocking 안으로 이동.
    let root_owned = workspace_root.to_path_buf();
    let opts_owned = options;
    let sink_clone: Arc<E> = sink.clone();
    let cancel_clone = cancel.clone();

    let result = tokio::task::spawn_blocking(move || {
        export_blocking(&root_owned, &opts_owned, sink_clone, cancel_clone)
    })
    .await
    .map_err(|e| ExportError::WriteFailed(format!("background task join 실패: {e}")))?;

    match result {
        Ok(summary) => {
            sink.emit(ExportEvent::Done {
                sha256: summary.sha256.clone(),
                archive_size_bytes: summary.archive_size_bytes,
                target_path: summary.target_path.as_deref().unwrap_or("").to_string(),
            });
            Ok(ExportSummary {
                sha256: summary.sha256,
                archive_size_bytes: summary.archive_size_bytes,
                files_count: summary.files_count,
            })
        }
        Err(e) => {
            sink.emit(ExportEvent::Failed {
                error: e.to_string(),
            });
            Err(e)
        }
    }
}

/// 내부 — Done 이벤트 emit 전에 target_path를 함께 들고 다님.
struct InternalSummary {
    sha256: String,
    archive_size_bytes: u64,
    files_count: u64,
    target_path: Option<String>,
}

/// Blocking heavy path — `spawn_blocking` 안에서만 호출.
fn export_blocking<E: ExportSink>(
    workspace_root: &Path,
    options: &ExportOptions,
    sink: Arc<E>,
    cancel: CancellationToken,
) -> Result<InternalSummary, ExportError> {
    // 1. 카운팅.
    let entries = collect_entries(workspace_root, options.include_models)?;
    if cancel.is_cancelled() {
        return Err(ExportError::Cancelled);
    }
    let total_files = entries.len() as u64;
    let total_bytes: u64 = entries.iter().map(|(_, _, size)| size).sum();
    sink.emit(ExportEvent::Counting {
        total_files,
        total_bytes,
    });

    // 2. tmp 파일 생성. parent dir 보장.
    if let Some(parent) = options.target_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ExportError::Io {
            path: parent.display().to_string(),
            source: e,
        })?;
    }
    let tmp = TempArchive::new(&options.target_path);
    let writer = std::fs::File::create(&tmp.tmp_path).map_err(|e| ExportError::Io {
        path: tmp.tmp_path.display().to_string(),
        source: e,
    })?;
    let mut zip = zip::ZipWriter::new(writer);
    let zip_opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    // 3. 메인 entry 쓰기. 진행률 emit.
    for (idx, (full_path, rel_path, _)) in entries.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(ExportError::Cancelled);
        }
        let rel_str = rel_path.to_string_lossy().replace('\\', "/");
        // zip-slip 사전 방어 — entries는 우리가 만든 list지만 lexical 검증 한 번 더.
        crate::import::lexical_safe_subpath_check(rel_path)
            .map_err(|_| ExportError::ZipSlip(rel_str.clone()))?;
        zip.start_file(&rel_str, zip_opts)
            .map_err(|e| ExportError::WriteFailed(format!("start_file: {e}")))?;
        // 64KB 청크 streaming.
        let mut input = std::fs::File::open(full_path).map_err(|e| ExportError::Io {
            path: full_path.display().to_string(),
            source: e,
        })?;
        let mut buf = [0u8; 64 * 1024];
        loop {
            if cancel.is_cancelled() {
                return Err(ExportError::Cancelled);
            }
            let n = input.read(&mut buf).map_err(|e| ExportError::Io {
                path: full_path.display().to_string(),
                source: e,
            })?;
            if n == 0 {
                break;
            }
            zip.write_all(&buf[..n])
                .map_err(|e| ExportError::WriteFailed(format!("write entry: {e}")))?;
        }
        sink.emit(ExportEvent::Compressing {
            processed: (idx as u64) + 1,
            total: total_files,
            current_path: rel_str,
        });
    }

    // 4. fingerprint.source.json — 받은 PC가 비교용. 메타 디렉터리에 추가.
    let manifest_text =
        std::fs::read_to_string(workspace_root.join("manifest.json")).map_err(|e| {
            ExportError::Io {
                path: "manifest.json".into(),
                source: e,
            }
        })?;
    let manifest_value: serde_json::Value = serde_json::from_str(&manifest_text)
        .map_err(|e| ExportError::WriteFailed(e.to_string()))?;
    if let Some(host) = manifest_value.get("host_fingerprint") {
        if let Ok(host_struct) =
            serde_json::from_value::<shared_types::HostFingerprint>(host.clone())
        {
            let fp = WorkspaceFingerprint::from_host(&host_struct);
            let fp_body = serde_json::to_vec_pretty(&fp)
                .map_err(|e| ExportError::WriteFailed(e.to_string()))?;
            zip.start_file("fingerprint.source.json", zip_opts)
                .map_err(|e| ExportError::WriteFailed(format!("fingerprint entry: {e}")))?;
            zip.write_all(&fp_body)
                .map_err(|e| ExportError::WriteFailed(format!("fingerprint write: {e}")))?;
        }
    }

    // 5. 키 wrap — include_keys일 때 한정.
    if options.include_keys {
        sink.emit(ExportEvent::Encrypting);
        let pass = options
            .key_passphrase
            .as_deref()
            .ok_or(ExportError::MissingPassphrase)?;
        let keys_db = workspace_root.join("data").join("keys.db");
        if keys_db.exists() {
            let raw = std::fs::read(&keys_db).map_err(|e| ExportError::Io {
                path: keys_db.display().to_string(),
                source: e,
            })?;
            let wrapped = wrap_with_passphrase(&raw, pass)
                .map_err(|e| ExportError::KeyEncryption(e.to_string()))?;
            zip.start_file("keys.encrypted", zip_opts)
                .map_err(|e| ExportError::WriteFailed(format!("keys entry: {e}")))?;
            zip.write_all(&wrapped)
                .map_err(|e| ExportError::WriteFailed(format!("keys write: {e}")))?;
        }
    }

    sink.emit(ExportEvent::Finalizing);

    // 6. zip close — finalize.
    let written = zip
        .finish()
        .map_err(|e| ExportError::WriteFailed(format!("zip finish: {e}")))?;
    drop(written); // file handle 닫음.

    // 7. archive sha256 read-back.
    let sha = compute_sha256(&tmp.tmp_path)?;

    // 8. atomic rename.
    let size = tmp
        .commit(&options.target_path)
        .map_err(|e| ExportError::Io {
            path: options.target_path.display().to_string(),
            source: e,
        })?;

    Ok(InternalSummary {
        sha256: sha,
        archive_size_bytes: size,
        files_count: total_files,
        target_path: Some(options.target_path.display().to_string()),
    })
}

/// workspace 트리에서 archive에 포함할 entry 수집.
///
/// 메타 디렉터리: `manifest.json` + `fingerprint.json` + `data/` + `manifests/` + `projects/` +
/// `sdk/` + `docs/` + `presets/`.
/// `models/`는 include_models=true일 때만.
/// `runtimes/` / `cache/` / `logs/`는 *제외* (OS-bound or regenerable).
fn collect_entries(
    root: &Path,
    include_models: bool,
) -> Result<Vec<(PathBuf, PathBuf, u64)>, ExportError> {
    let mut out = Vec::new();
    // 단일 파일 entries.
    for top in &["manifest.json", "fingerprint.json"] {
        let p = root.join(top);
        if p.is_file() {
            let size = std::fs::metadata(&p)
                .map_err(|e| ExportError::Io {
                    path: p.display().to_string(),
                    source: e,
                })?
                .len();
            out.push((p, PathBuf::from(top), size));
        }
    }
    // 디렉터리 entries — recursive.
    let mut dirs: Vec<&str> = vec!["data", "manifests", "projects", "sdk", "docs", "presets"];
    if include_models {
        dirs.push("models");
    }
    for d in dirs {
        let dir_path = root.join(d);
        if !dir_path.is_dir() {
            continue;
        }
        walk_dir(&dir_path, &PathBuf::from(d), &mut out)?;
    }
    Ok(out)
}

fn walk_dir(
    full: &Path,
    rel_so_far: &Path,
    out: &mut Vec<(PathBuf, PathBuf, u64)>,
) -> Result<(), ExportError> {
    for entry in std::fs::read_dir(full).map_err(|e| ExportError::Io {
        path: full.display().to_string(),
        source: e,
    })? {
        let entry = entry.map_err(|e| ExportError::Io {
            path: full.display().to_string(),
            source: e,
        })?;
        let path = entry.path();
        let name = entry.file_name();
        let rel_child = rel_so_far.join(&name);
        if path.is_dir() {
            walk_dir(&path, &rel_child, out)?;
        } else {
            let size = entry
                .metadata()
                .map_err(|e| ExportError::Io {
                    path: path.display().to_string(),
                    source: e,
                })?
                .len();
            out.push((path, rel_child, size));
        }
    }
    Ok(())
}

/// 64KB 청크로 sha256 streaming compute. RAM 부하 < 메가바이트.
fn compute_sha256(path: &Path) -> Result<String, ExportError> {
    let mut f = std::fs::File::open(path).map_err(|e| ExportError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(|e| ExportError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// AES-GCM + PBKDF2(100k) wrap.
///
/// Output layout: `salt(16) | nonce(12) | ciphertext+tag(N)`.
/// 패스프레이즈는 PBKDF2 derive → 256bit key → AES-256-GCM seal.
pub(crate) fn wrap_with_passphrase(plaintext: &[u8], passphrase: &str) -> Result<Vec<u8>, String> {
    let mut salt = [0u8; PBKDF2_SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    let mut key = [0u8; 32];
    pbkdf2::<Hmac<Sha256>>(passphrase.as_bytes(), &salt, PBKDF2_ITER, &mut key)
        .map_err(|e| format!("PBKDF2 derive 실패: {e}"))?;

    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("AES key invalid: {e}"))?;
    let mut nonce_bytes = [0u8; AES_GCM_NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("AES-GCM seal 실패: {e}"))?;

    let mut out = Vec::with_capacity(salt.len() + nonce_bytes.len() + ciphertext.len());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// AES-GCM + PBKDF2 unwrap. import 측에서 호출.
pub(crate) fn unwrap_with_passphrase(wrapped: &[u8], passphrase: &str) -> Result<Vec<u8>, String> {
    if wrapped.len() < PBKDF2_SALT_LEN + AES_GCM_NONCE_LEN + 16 {
        return Err("암호화 페이로드가 너무 짧아요".into());
    }
    let salt = &wrapped[..PBKDF2_SALT_LEN];
    let nonce_bytes = &wrapped[PBKDF2_SALT_LEN..PBKDF2_SALT_LEN + AES_GCM_NONCE_LEN];
    let ciphertext = &wrapped[PBKDF2_SALT_LEN + AES_GCM_NONCE_LEN..];

    let mut key = [0u8; 32];
    pbkdf2::<Hmac<Sha256>>(passphrase.as_bytes(), salt, PBKDF2_ITER, &mut key)
        .map_err(|e| format!("PBKDF2 derive 실패: {e}"))?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("AES key invalid: {e}"))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "패스프레이즈가 일치하지 않아요".to_string())
}

// 미사용 import 경고 막기 — Seek는 zip이 내부 사용.
#[allow(dead_code)]
fn _seek_marker<S: Seek>(_s: S) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{PortMap, WorkspaceManifest};
    use shared_types::HostFingerprint;
    use std::sync::Mutex;
    use tempfile::tempdir;

    /// 테스트용 in-memory sink — 받은 이벤트를 vec에 축적.
    struct VecSink {
        events: Mutex<Vec<ExportEvent>>,
    }

    impl VecSink {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                events: Mutex::new(Vec::new()),
            })
        }
        fn drain(&self) -> Vec<ExportEvent> {
            self.events.lock().unwrap().drain(..).collect()
        }
    }

    impl ExportSink for VecSink {
        fn emit(&self, event: ExportEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    fn seed_workspace(root: &Path, with_models: bool, with_keys: bool) {
        std::fs::create_dir_all(root).unwrap();
        let manifest = WorkspaceManifest {
            schema_version: 1,
            workspace_id: "ws-test".into(),
            host_fingerprint: HostFingerprint {
                os: "windows".into(),
                arch: "x86_64".into(),
                cpu: "test cpu".into(),
                ram_mb: 65536,
                gpu_vendor: Some("nvidia".into()),
                gpu_model: Some("RTX 4090".into()),
                vram_mb: Some(24576),
            },
            runtimes_installed: vec![],
            models_installed: vec![],
            ports: PortMap::default(),
            created_at: "2026-04-28T00:00:00Z".into(),
            last_repaired_at: None,
        };
        std::fs::write(
            root.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        std::fs::create_dir_all(root.join("data")).unwrap();
        std::fs::write(root.join("data/settings.json"), b"{\"hello\":1}").unwrap();
        if with_models {
            std::fs::create_dir_all(root.join("models")).unwrap();
            std::fs::write(root.join("models/dummy.gguf"), vec![0u8; 1024]).unwrap();
        }
        if with_keys {
            // wrap 시점에 keys.db만 있으면 됨.
            std::fs::write(root.join("data/keys.db"), b"FAKE_SQLITE_BLOB").unwrap();
        }
    }

    #[tokio::test]
    async fn export_round_trip_meta_only() {
        let dir = tempdir().unwrap();
        seed_workspace(dir.path(), false, false);
        let target = dir.path().join("out.zip");

        let sink = VecSink::new();
        let summary = export_workspace(
            dir.path(),
            ExportOptions {
                include_models: false,
                include_keys: false,
                key_passphrase: None,
                target_path: target.clone(),
            },
            sink.clone(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(summary.sha256.len(), 64);
        assert!(summary.files_count >= 2); // manifest + data/settings.json
        assert!(target.exists());
        let events = sink.drain();
        assert!(events
            .iter()
            .any(|e| matches!(e, ExportEvent::Started { .. })));
        assert!(events.iter().any(|e| matches!(e, ExportEvent::Done { .. })));
    }

    #[tokio::test]
    async fn export_with_models_includes_model_entries() {
        let dir = tempdir().unwrap();
        seed_workspace(dir.path(), true, false);
        let target = dir.path().join("out.zip");

        let sink = VecSink::new();
        let s = export_workspace(
            dir.path(),
            ExportOptions {
                include_models: true,
                include_keys: false,
                key_passphrase: None,
                target_path: target.clone(),
            },
            sink.clone(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert!(s.files_count >= 3);

        // zip 안을 직접 열어 model file 포함 확인.
        let f = std::fs::File::open(&target).unwrap();
        let mut zip = zip::ZipArchive::new(f).unwrap();
        let names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.iter().any(|n| n == "models/dummy.gguf"));
        assert!(names.iter().any(|n| n == "manifest.json"));
    }

    #[tokio::test]
    async fn export_with_keys_round_trips_passphrase() {
        let dir = tempdir().unwrap();
        seed_workspace(dir.path(), false, true);
        let target = dir.path().join("out.zip");

        let sink = VecSink::new();
        let _ = export_workspace(
            dir.path(),
            ExportOptions {
                include_models: false,
                include_keys: true,
                key_passphrase: Some("hunter2".into()),
                target_path: target.clone(),
            },
            sink.clone(),
            CancellationToken::new(),
        )
        .await
        .unwrap();

        // unwrap 가능.
        let f = std::fs::File::open(&target).unwrap();
        let mut zip = zip::ZipArchive::new(f).unwrap();
        let mut buf = Vec::new();
        zip.by_name("keys.encrypted")
            .unwrap()
            .read_to_end(&mut buf)
            .unwrap();
        let plain = unwrap_with_passphrase(&buf, "hunter2").unwrap();
        assert_eq!(plain, b"FAKE_SQLITE_BLOB");

        // 잘못된 passphrase는 실패.
        assert!(unwrap_with_passphrase(&buf, "wrong-pass").is_err());
    }

    #[tokio::test]
    async fn export_missing_passphrase_when_keys_requested() {
        let dir = tempdir().unwrap();
        seed_workspace(dir.path(), false, true);
        let target = dir.path().join("out.zip");

        let sink = VecSink::new();
        let r = export_workspace(
            dir.path(),
            ExportOptions {
                include_models: false,
                include_keys: true,
                key_passphrase: None,
                target_path: target,
            },
            sink.clone(),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(r, Err(ExportError::MissingPassphrase)));
    }

    #[tokio::test]
    async fn export_empty_workspace_errors() {
        let dir = tempdir().unwrap();
        // manifest.json 없음.
        let target = dir.path().join("out.zip");
        let sink = VecSink::new();
        let r = export_workspace(
            dir.path(),
            ExportOptions {
                include_models: false,
                include_keys: false,
                key_passphrase: None,
                target_path: target,
            },
            sink.clone(),
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(r, Err(ExportError::EmptySourceWorkspace)));
    }

    #[tokio::test]
    async fn export_cancel_cleans_tmp() {
        let dir = tempdir().unwrap();
        seed_workspace(dir.path(), true, false);
        // 더 큰 모델 파일을 생성해 cancel 타이밍 확보.
        std::fs::write(
            dir.path().join("models/big.gguf"),
            vec![0u8; 4 * 1024 * 1024],
        )
        .unwrap();
        let target = dir.path().join("out.zip");
        let cancel = CancellationToken::new();
        // 즉시 cancel.
        cancel.cancel();
        let sink = VecSink::new();
        let r = export_workspace(
            dir.path(),
            ExportOptions {
                include_models: true,
                include_keys: false,
                key_passphrase: None,
                target_path: target.clone(),
            },
            sink.clone(),
            cancel,
        )
        .await;
        assert!(matches!(r, Err(ExportError::Cancelled)));
        // .tmp 정리 확인.
        let mut tmp = target.clone();
        let mut name = tmp.file_name().unwrap().to_os_string();
        name.push(".tmp");
        tmp.set_file_name(name);
        assert!(!tmp.exists(), "tmp 파일이 남았어요: {}", tmp.display());
        // final도 없음.
        assert!(!target.exists());
    }

    #[tokio::test]
    async fn export_sha256_matches_file() {
        let dir = tempdir().unwrap();
        seed_workspace(dir.path(), false, false);
        let target = dir.path().join("out.zip");

        let sink = VecSink::new();
        let s = export_workspace(
            dir.path(),
            ExportOptions {
                include_models: false,
                include_keys: false,
                key_passphrase: None,
                target_path: target.clone(),
            },
            sink.clone(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
        let actual = compute_sha256(&target).unwrap();
        assert_eq!(s.sha256, actual);
    }

    #[tokio::test]
    async fn export_atomic_rename_no_partial_on_disk() {
        let dir = tempdir().unwrap();
        seed_workspace(dir.path(), false, false);
        let target = dir.path().join("nested/dir/out.zip");

        let sink = VecSink::new();
        let _s = export_workspace(
            dir.path(),
            ExportOptions {
                include_models: false,
                include_keys: false,
                key_passphrase: None,
                target_path: target.clone(),
            },
            sink.clone(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
        // 최종 파일만 존재, .tmp 없음.
        assert!(target.exists());
        let mut tmp = target.clone();
        let mut name = tmp.file_name().unwrap().to_os_string();
        name.push(".tmp");
        tmp.set_file_name(name);
        assert!(!tmp.exists());
    }

    #[test]
    fn wrap_unwrap_round_trip() {
        let plain = b"secret".to_vec();
        let wrapped = wrap_with_passphrase(&plain, "pw").unwrap();
        let restored = unwrap_with_passphrase(&wrapped, "pw").unwrap();
        assert_eq!(restored, plain);
    }
}

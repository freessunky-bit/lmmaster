//! Archive 추출 — zip / tar.gz / dmg.
//!
//! 정책 (Phase 1A.3.b.2 + 1A.3.b.3 보강 리서치):
//! - zip 8.x (sync) + `tokio::task::spawn_blocking`. CVE-2025-29787 fix 포함.
//! - tar + flate2 per-entry streaming (progress + cancel checkpoint).
//! - **Zip-slip 방어**: zip은 `ZipFile::enclosed_name()` (8.x 안전) + 우리 lexical 추가 체크.
//!   tar는 directly entry.path() → lexical 검증 (RootDir/Prefix reject, ParentDir depth -1 reject).
//! - **Cancel**: `Arc<AtomicBool>` flag, 외부 watcher가 set, blocking task가 entry 사이마다 polling.
//! - **dmg (macOS only)**: `hdiutil attach -plist -nobrowse -readonly -noautoopen -noverify -mountrandom`
//!   → plist 파싱 (`plist` crate 1.x) → `/usr/bin/ditto` 복사 (메타데이터/xattr/코드사인 보존)
//!   → `MountGuard` Drop으로 detach 보장 (busy 시 -force fallback) + 100ms `try_wait` cancel polling.
//!   non-macOS는 `DmgRequiresMac`. 결정 근거는 `docs/research/phase-1a3b3-decision.md` 참고.
#![allow(clippy::doc_lazy_continuation)]

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtractFormat {
    Zip,
    TarGz,
    Dmg,
}

/// 파일 확장자 기반 format 추정. `None`이면 호출자가 명시적으로 spec.
pub fn detect_format(path: &Path) -> Option<ExtractFormat> {
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    if name.ends_with(".zip") {
        Some(ExtractFormat::Zip)
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        Some(ExtractFormat::TarGz)
    } else if name.ends_with(".dmg") {
        Some(ExtractFormat::Dmg)
    } else {
        None
    }
}

#[derive(Debug, Error)]
pub enum ExtractError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("zip-slip detected: entry path {0} would escape extract target")]
    ZipSlip(String),

    #[error("extraction cancelled")]
    Cancelled,

    #[error("dmg extraction is macOS only")]
    DmgRequiresMac,

    #[error("dmg attach 실패: {0}")]
    DmgAttachFailed(String),

    #[error("dmg plist parse 실패: {0}")]
    DmgPlist(String),

    #[error("ditto 복사 실패 (exit code {0:?})")]
    DittoFailed(Option<i32>),

    #[error("background task join failed: {0}")]
    Join(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractOutcome {
    pub entries: u64,
    pub total_bytes: u64,
    pub format: ExtractFormat,
}

/// Cancel-safe async extract entry point.
pub async fn extract(
    archive: &Path,
    target: &Path,
    fmt: ExtractFormat,
    cancel: &CancellationToken,
) -> Result<ExtractOutcome, ExtractError> {
    if !target.exists() {
        tokio::fs::create_dir_all(target).await?;
    }
    let archive = archive.to_path_buf();
    let target = target.to_path_buf();

    // Cancel watcher: 외부 task가 atomic flag를 set. blocking 작업이 entry 사이에 polling.
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let f2 = cancel_flag.clone();
    let cancel_clone = cancel.clone();
    let watcher = tokio::spawn(async move {
        cancel_clone.cancelled().await;
        f2.store(true, Ordering::Relaxed);
    });

    let result: Result<ExtractOutcome, ExtractError> = match fmt {
        ExtractFormat::Zip => tokio::task::spawn_blocking(move || {
            extract_zip_blocking(&archive, &target, cancel_flag)
        })
        .await
        .map_err(|e| ExtractError::Join(e.to_string()))
        .and_then(|inner| inner),
        ExtractFormat::TarGz => tokio::task::spawn_blocking(move || {
            extract_tar_gz_blocking(&archive, &target, cancel_flag)
        })
        .await
        .map_err(|e| ExtractError::Join(e.to_string()))
        .and_then(|inner| inner),
        ExtractFormat::Dmg => {
            #[cfg(target_os = "macos")]
            {
                tokio::task::spawn_blocking(move || {
                    dmg_macos::extract_dmg_blocking(&archive, &target, cancel_flag)
                })
                .await
                .map_err(|e| ExtractError::Join(e.to_string()))
                .and_then(|inner| inner)
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = (archive, target, cancel_flag);
                Err(ExtractError::DmgRequiresMac)
            }
        }
    };

    watcher.abort();
    result
}

fn extract_zip_blocking(
    archive: &Path,
    target: &Path,
    cancel: Arc<AtomicBool>,
) -> Result<ExtractOutcome, ExtractError> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file))?;

    let mut entries = 0u64;
    let mut total_bytes = 0u64;

    for i in 0..zip.len() {
        if cancel.load(Ordering::Relaxed) {
            return Err(ExtractError::Cancelled);
        }
        let mut entry = zip.by_index(i)?;
        // ZipFile::enclosed_name (zip 8.x) — abs prefix + .. components 거부.
        let safe_name = entry
            .enclosed_name()
            .ok_or_else(|| ExtractError::ZipSlip(entry.name().to_string()))?;
        // 추가 lexical 검증 — 이중 방어.
        let safe_rel = lexical_safe_subpath(&safe_name)?;
        let dest = target.join(&safe_rel);

        if entry.is_dir() {
            std::fs::create_dir_all(&dest)?;
        } else {
            if let Some(parent) = dest.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            let mut out = std::fs::File::create(&dest)?;
            let mut buf = [0u8; 64 * 1024];
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return Err(ExtractError::Cancelled);
                }
                let n = entry.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                std::io::Write::write_all(&mut out, &buf[..n])?;
            }
        }
        entries += 1;
        total_bytes = total_bytes.saturating_add(entry.size());
    }

    Ok(ExtractOutcome {
        entries,
        total_bytes,
        format: ExtractFormat::Zip,
    })
}

fn extract_tar_gz_blocking(
    archive: &Path,
    target: &Path,
    cancel: Arc<AtomicBool>,
) -> Result<ExtractOutcome, ExtractError> {
    let file = std::fs::File::open(archive)?;
    let gz = flate2::read::GzDecoder::new(std::io::BufReader::new(file));
    let mut ar = tar::Archive::new(gz);
    ar.set_preserve_permissions(true);
    ar.set_overwrite(true);

    let mut entries = 0u64;
    let mut total_bytes = 0u64;

    for entry_res in ar.entries()? {
        if cancel.load(Ordering::Relaxed) {
            return Err(ExtractError::Cancelled);
        }
        let mut entry = entry_res?;
        let entry_path = entry.path()?.into_owned();
        // tar는 enclosed_name 없음 → 우리가 직접 lexical 검증.
        let safe_rel = lexical_safe_subpath(&entry_path)?;
        let dest = target.join(&safe_rel);

        let size = entry.size();
        let header_kind = entry.header().entry_type();

        // 디렉터리 엔트리는 mkdir만 하고 다음으로.
        if header_kind.is_dir() {
            std::fs::create_dir_all(&dest)?;
            entries += 1;
            continue;
        }

        // 파일 엔트리: 부모 디렉터리 보장 후 unpack.
        // tar::Entry::unpack는 자동 부모 생성을 하지 않으므로 직접 만든다.
        if let Some(parent) = dest.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }
        entry.unpack(&dest)?;
        entries += 1;
        total_bytes = total_bytes.saturating_add(size);
    }

    Ok(ExtractOutcome {
        entries,
        total_bytes,
        format: ExtractFormat::TarGz,
    })
}

/// macOS dmg 추출 — `hdiutil attach` + plist 파싱 + `ditto` 복사 + Drop-guard detach.
///
/// **불변량**:
/// - `MountGuard`는 `dev-entry`를 받자마자 생성되고, return/?/panic 어디서든 drop이 실행돼
///   `hdiutil detach`를 호출. busy(exit 16) 시 `-force` fallback. read-only 마운트라 안전.
/// - ditto는 `/usr/bin/ditto`로 절대 경로 호출 (`PATH` 의존 없음).
/// - 100ms `try_wait` 폴링으로 cancel 응답 — `Arc<AtomicBool>` true면 child kill 후 Cancelled.
/// - 진행률은 indeterminate (ditto는 native progress 없음). entries/total_bytes는 종료 후 walkdir.
#[cfg(target_os = "macos")]
mod dmg_macos {
    use super::{ExtractError, ExtractFormat, ExtractOutcome};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    /// Drop 시 `hdiutil detach`. busy(exit 16) 시 `-force` 재시도. read-only라 force-detach 안전.
    pub(super) struct MountGuard {
        device: String,
    }

    impl Drop for MountGuard {
        fn drop(&mut self) {
            // 1차: 정상 detach.
            let try1 = Command::new("hdiutil")
                .args(["detach", &self.device])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            if matches!(try1, Ok(s) if s.success()) {
                return;
            }
            // 2차: -force. read-only 마운트는 unflushed write가 없어 데이터 손실 위험 없음.
            let _ = Command::new("hdiutil")
                .args(["detach", "-force", &self.device])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }

    /// `hdiutil attach` plist에서 `(dev-entry, mount-point)` 첫 매칭 추출.
    /// `system-entities`의 partition scheme entry는 dev-entry만 있고 mount-point가 없어 자동으로 제외됨.
    pub(super) fn parse_attach_plist(stdout: &[u8]) -> Result<(String, PathBuf), ExtractError> {
        let value = plist::Value::from_reader(std::io::Cursor::new(stdout))
            .map_err(|e| ExtractError::DmgPlist(format!("plist 디코드 실패: {e}")))?;
        let entities = value
            .as_dictionary()
            .and_then(|d| d.get("system-entities"))
            .and_then(|e| e.as_array())
            .ok_or_else(|| ExtractError::DmgPlist("system-entities 배열 없음".into()))?;
        for ent in entities {
            let Some(d) = ent.as_dictionary() else {
                continue;
            };
            let dev = d.get("dev-entry").and_then(|x| x.as_string());
            let mp = d.get("mount-point").and_then(|x| x.as_string());
            if let (Some(dev), Some(mp)) = (dev, mp) {
                if !mp.is_empty() {
                    return Ok((dev.to_string(), PathBuf::from(mp)));
                }
            }
        }
        Err(ExtractError::DmgPlist(
            "마운트 가능한 system-entity 없음".into(),
        ))
    }

    pub(super) fn extract_dmg_blocking(
        archive: &Path,
        target: &Path,
        cancel: Arc<AtomicBool>,
    ) -> Result<ExtractOutcome, ExtractError> {
        if cancel.load(Ordering::Relaxed) {
            return Err(ExtractError::Cancelled);
        }

        // 마운트 디렉터리(`-mountrandom`)는 tempfile로 — `/Volumes/<DMG-name>` 충돌 방지.
        let mount_root = tempfile::Builder::new()
            .prefix("lmmaster-dmg-")
            .tempdir()
            .map_err(ExtractError::Io)?;

        // hdiutil attach. EULA pager가 뜰 경우 stdin "qn\n"으로 quit + 자동 동의 거부.
        let mut child = Command::new("hdiutil")
            .args([
                "attach",
                "-plist",
                "-nobrowse",
                "-readonly",
                "-noautoopen",
                "-noverify",
                "-mountrandom",
            ])
            .arg(mount_root.path())
            .arg(archive)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| ExtractError::DmgAttachFailed(format!("hdiutil spawn 실패: {e}")))?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write as _;
            // EULA 발생 시 pager 종료 + 자동 동의 거부. attach 자체엔 영향 없음.
            let _ = stdin.write_all(b"qn\n");
        }
        let output = child.wait_with_output().map_err(|e| {
            ExtractError::DmgAttachFailed(format!("hdiutil wait_with_output 실패: {e}"))
        })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ExtractError::DmgAttachFailed(format!(
                "hdiutil exit {:?}: {}",
                output.status.code(),
                stderr.trim()
            )));
        }

        let (device, mount_point) = parse_attach_plist(&output.stdout)?;
        // device를 받자마자 guard 생성 — 이후 어떤 path로 빠져나가도 detach 보장.
        let _guard = MountGuard {
            device: device.clone(),
        };

        if cancel.load(Ordering::Relaxed) {
            return Err(ExtractError::Cancelled);
        }

        // ditto 복사. /usr/bin/ditto 절대 경로 (PATH 의존 X).
        let mut ditto_child = Command::new("/usr/bin/ditto")
            .arg(&mount_point)
            .arg(target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(ExtractError::Io)?;

        // 100ms try_wait 폴링 — cancel 발생 시 child kill.
        let exit_status = loop {
            if cancel.load(Ordering::Relaxed) {
                let _ = ditto_child.kill();
                let _ = ditto_child.wait();
                return Err(ExtractError::Cancelled);
            }
            match ditto_child.try_wait().map_err(ExtractError::Io)? {
                Some(status) => break status,
                None => std::thread::sleep(Duration::from_millis(100)),
            }
        };
        if !exit_status.success() {
            // stderr capture는 wait 후 streaming이 닫혔으니 별도 읽기.
            let mut stderr_buf = Vec::new();
            if let Some(mut s) = ditto_child.stderr.take() {
                use std::io::Read as _;
                let _ = s.read_to_end(&mut stderr_buf);
            }
            tracing::warn!(
                "ditto 실패: code={:?} stderr={}",
                exit_status.code(),
                String::from_utf8_lossy(&stderr_buf).trim()
            );
            return Err(ExtractError::DittoFailed(exit_status.code()));
        }

        // ditto는 진행률을 보고하지 않으므로, 종료 후 walkdir로 entries/total_bytes 집계.
        let mut entries: u64 = 0;
        let mut total_bytes: u64 = 0;
        for entry in walkdir::WalkDir::new(target)
            .min_depth(1)
            .into_iter()
            .filter_map(Result::ok)
        {
            entries += 1;
            if entry.file_type().is_file() {
                if let Ok(meta) = entry.metadata() {
                    total_bytes = total_bytes.saturating_add(meta.len());
                }
            }
        }

        // _guard drop → detach.
        Ok(ExtractOutcome {
            entries,
            total_bytes,
            format: ExtractFormat::Dmg,
        })
    }
}

/// 경로의 컴포넌트만 검사해 target 외부로 escape 가능성을 거부. 파일시스템 접근 없음.
/// - RootDir/Prefix → 절대 경로 → 거부
/// - ParentDir 누적이 NormalDir보다 많아지면 → 거부 (target 밖으로 나감)
fn lexical_safe_subpath(rel: &Path) -> Result<PathBuf, ExtractError> {
    let mut depth: i32 = 0;
    let mut clean = PathBuf::new();
    for comp in rel.components() {
        match comp {
            std::path::Component::Normal(s) => {
                clean.push(s);
                depth += 1;
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return Err(ExtractError::ZipSlip(rel.display().to_string()));
                }
                clean.pop();
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err(ExtractError::ZipSlip(rel.display().to_string()));
            }
        }
    }
    Ok(clean)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_format_by_extension() {
        assert_eq!(
            detect_format(Path::new("foo.zip")),
            Some(ExtractFormat::Zip)
        );
        assert_eq!(
            detect_format(Path::new("foo.tar.gz")),
            Some(ExtractFormat::TarGz)
        );
        assert_eq!(
            detect_format(Path::new("foo.tgz")),
            Some(ExtractFormat::TarGz)
        );
        assert_eq!(
            detect_format(Path::new("foo.dmg")),
            Some(ExtractFormat::Dmg)
        );
        assert!(detect_format(Path::new("foo.exe")).is_none());
        assert!(detect_format(Path::new("noext")).is_none());
    }

    #[test]
    fn lexical_safe_subpath_normal_paths() {
        let p = lexical_safe_subpath(Path::new("dir/file.txt")).unwrap();
        assert_eq!(p, PathBuf::from("dir/file.txt"));
        let p = lexical_safe_subpath(Path::new("a/./b/c")).unwrap();
        assert_eq!(p, PathBuf::from("a/b/c"));
    }

    #[test]
    fn lexical_safe_subpath_collapses_inner_parent() {
        let p = lexical_safe_subpath(Path::new("a/b/../c")).unwrap();
        assert_eq!(p, PathBuf::from("a/c"));
    }

    #[test]
    fn lexical_safe_subpath_rejects_escape() {
        assert!(lexical_safe_subpath(Path::new("../escape")).is_err());
        assert!(lexical_safe_subpath(Path::new("a/../../escape")).is_err());
    }

    #[test]
    fn lexical_safe_subpath_rejects_absolute() {
        // Windows에서는 "/abs"가 RootDir로, "C:\\abs"는 Prefix로 옴.
        assert!(lexical_safe_subpath(Path::new("/abs/path")).is_err());
    }

    #[tokio::test]
    async fn extract_zip_roundtrip_with_subdirs() {
        use std::io::Write;

        let dir = tempfile::TempDir::new().unwrap();
        let archive_path = dir.path().join("test.zip");

        // Create test zip.
        {
            let f = std::fs::File::create(&archive_path).unwrap();
            let mut zip = zip::ZipWriter::new(f);
            let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("hello.txt", opts).unwrap();
            zip.write_all(b"hello world").unwrap();
            zip.start_file("sub/nested.txt", opts).unwrap();
            zip.write_all(b"nested content").unwrap();
            zip.finish().unwrap();
        }

        let target = dir.path().join("out");
        let cancel = CancellationToken::new();
        let outcome = extract(&archive_path, &target, ExtractFormat::Zip, &cancel)
            .await
            .expect("extract ok");

        assert_eq!(outcome.entries, 2);
        assert_eq!(outcome.format, ExtractFormat::Zip);
        assert_eq!(
            std::fs::read_to_string(target.join("hello.txt")).unwrap(),
            "hello world"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("sub/nested.txt")).unwrap(),
            "nested content"
        );
    }

    #[tokio::test]
    async fn extract_tar_gz_roundtrip() {
        let dir = tempfile::TempDir::new().unwrap();
        let archive_path = dir.path().join("test.tar.gz");

        // Create test tar.gz.
        {
            let f = std::fs::File::create(&archive_path).unwrap();
            let gz = flate2::write::GzEncoder::new(f, flate2::Compression::default());
            let mut tar_w = tar::Builder::new(gz);
            // Add a file.
            let content = b"alpha";
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar_w
                .append_data(&mut header, "alpha.txt", &content[..])
                .unwrap();
            // Add a sub-file.
            let content2 = b"beta in subdir";
            let mut header2 = tar::Header::new_gnu();
            header2.set_size(content2.len() as u64);
            header2.set_mode(0o644);
            header2.set_cksum();
            tar_w
                .append_data(&mut header2, "sub/beta.txt", &content2[..])
                .unwrap();
            tar_w.into_inner().unwrap().finish().unwrap();
        }

        let target = dir.path().join("out");
        let cancel = CancellationToken::new();
        let outcome = extract(&archive_path, &target, ExtractFormat::TarGz, &cancel)
            .await
            .expect("extract ok");
        assert_eq!(outcome.entries, 2);
        assert_eq!(outcome.format, ExtractFormat::TarGz);
        assert_eq!(
            std::fs::read_to_string(target.join("alpha.txt")).unwrap(),
            "alpha"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("sub/beta.txt")).unwrap(),
            "beta in subdir"
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[tokio::test]
    async fn extract_dmg_returns_unsupported_on_non_mac() {
        let dir = tempfile::TempDir::new().unwrap();
        let archive_path = dir.path().join("fake.dmg");
        std::fs::write(&archive_path, b"fake content").unwrap();
        let target = dir.path().join("out");
        let cancel = CancellationToken::new();
        let r = extract(&archive_path, &target, ExtractFormat::Dmg, &cancel).await;
        // non-macOS는 항상 DmgRequiresMac.
        assert!(matches!(r, Err(ExtractError::DmgRequiresMac)));
    }

    #[tokio::test]
    async fn extract_zip_with_zipslip_entry_rejected() {
        use std::io::Write;

        let dir = tempfile::TempDir::new().unwrap();
        let archive_path = dir.path().join("evil.zip");

        // 정상 zip을 만들 때 ".." path는 zip 라이브러리가 직접 지원하지 않음.
        // 대신 zip에 `../escape.txt` 를 직접 작성 — zip crate의 SimpleFileOptions로는 path traversal을 막아야 하므로
        // 우리가 사용하는 ZipWriter::start_file이 path를 검증하는지 확인하는 sanity test.
        // (zip 8.x ZipWriter는 raw path를 그대로 받으므로 evil zip 작성은 가능.)
        {
            let f = std::fs::File::create(&archive_path).unwrap();
            let mut zip = zip::ZipWriter::new(f);
            let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
            // start_file_from_path가 path를 sanitize할 수 있어 raw start_file 사용.
            zip.start_file("../escape.txt", opts).unwrap();
            zip.write_all(b"pwned").unwrap();
            zip.finish().unwrap();
        }

        let target = dir.path().join("out");
        let cancel = CancellationToken::new();
        let r = extract(&archive_path, &target, ExtractFormat::Zip, &cancel).await;
        // enclosed_name은 ../escape를 거부하거나, 우리 lexical 체크가 거부.
        match r {
            Err(ExtractError::ZipSlip(_)) => {}
            other => {
                // 일부 zip 8.x는 enclosed_name이 None을 반환 (이 경우 우리가 ZipSlip으로 변환)
                // 또는 zip crate가 path를 저장하기 전에 sanitize. 둘 다 acceptable.
                // 이 테스트는 evil zip이 통과되지 않음만 확인.
                assert!(
                    !target.join("escape.txt").exists() && !dir.path().join("escape.txt").exists(),
                    "extraction must not write outside target; got result: {other:?}"
                );
            }
        }
    }

    // ── macOS 전용 테스트 ────────────────────────────────────────────────────
    // 이 모듈은 macOS에서만 컴파일된다. 통합 테스트는 `#[ignore]` 처리해
    // `cargo test -- --ignored extract_dmg`로 명시적으로 실행 (CI runner 분리).

    #[cfg(target_os = "macos")]
    #[test]
    fn dmg_parse_plist_finds_first_mountable_entity() {
        // hdiutil attach -plist 출력의 최소 재현체.
        // 첫 entity는 mount-point 없음(파티션 스킴) → skip. 두 번째 entity가 매칭.
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>system-entities</key>
    <array>
        <dict>
            <key>content-hint</key><string>Apple_partition_scheme</string>
            <key>dev-entry</key><string>/dev/disk5</string>
        </dict>
        <dict>
            <key>content-hint</key><string>Apple_HFS</string>
            <key>dev-entry</key><string>/dev/disk5s1</string>
            <key>mount-point</key><string>/Volumes/LMmasterTest</string>
            <key>volume-kind</key><string>hfs</string>
        </dict>
    </array>
</dict>
</plist>
"#;
        let (dev, mp) = super::dmg_macos::parse_attach_plist(xml).expect("plist 파싱 성공해야 함");
        assert_eq!(dev, "/dev/disk5s1");
        assert_eq!(mp, std::path::PathBuf::from("/Volumes/LMmasterTest"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn dmg_parse_plist_rejects_when_no_mount_point() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
    <key>system-entities</key>
    <array>
        <dict>
            <key>dev-entry</key><string>/dev/disk5</string>
        </dict>
    </array>
</dict>
</plist>
"#;
        let r = super::dmg_macos::parse_attach_plist(xml);
        assert!(matches!(r, Err(ExtractError::DmgPlist(_))));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn dmg_parse_plist_rejects_when_not_dict() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<array><string>broken</string></array>
</plist>
"#;
        let r = super::dmg_macos::parse_attach_plist(xml);
        assert!(matches!(r, Err(ExtractError::DmgPlist(_))));
    }

    /// 실제 hdiutil/ditto 통합 테스트 — macOS에서 `cargo test -- --ignored`로만 실행.
    /// 1MB HFS+ dmg를 hdiutil로 만들고, sentinel 파일을 마운트 안에 작성한 뒤 detach.
    /// extract() 호출 → target에 sentinel 복원 확인 + ExtractOutcome 검증.
    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[ignore = "macOS hdiutil 호출 — `cargo test -p installer -- --ignored extract_dmg`로 실행"]
    async fn extract_dmg_macos_roundtrip() {
        use std::process::Command;

        let dir = tempfile::TempDir::new().unwrap();
        let dmg_path = dir.path().join("test.dmg");
        let scratch = dir.path().join("scratch");
        std::fs::create_dir_all(&scratch).unwrap();

        // 1) 빈 1MB HFS+ dmg 생성.
        let create = Command::new("hdiutil")
            .args([
                "create",
                "-size",
                "2m",
                "-fs",
                "HFS+",
                "-volname",
                "LMmasterTest",
                "-ov",
            ])
            .arg(&dmg_path)
            .output()
            .expect("hdiutil create 실행");
        assert!(
            create.status.success(),
            "hdiutil create 실패: stderr={}",
            String::from_utf8_lossy(&create.stderr)
        );

        // 2) 마운트 → sentinel 작성 → detach (read-write 마운트 후 다시 read-only 검증).
        let attach = Command::new("hdiutil")
            .args(["attach", "-plist", "-nobrowse", "-mountrandom"])
            .arg(&scratch)
            .arg(&dmg_path)
            .output()
            .expect("hdiutil attach 실행");
        assert!(
            attach.status.success(),
            "fixture attach 실패: stderr={}",
            String::from_utf8_lossy(&attach.stderr)
        );
        let (device, mount_point) =
            super::dmg_macos::parse_attach_plist(&attach.stdout).expect("attach plist parse");
        std::fs::write(mount_point.join("hello.txt"), b"hello dmg").unwrap();
        let detach = Command::new("hdiutil")
            .args(["detach"])
            .arg(&device)
            .status()
            .expect("hdiutil detach 실행");
        assert!(detach.success(), "fixture detach 실패");

        // 3) extract 호출.
        let target = dir.path().join("out");
        let cancel = CancellationToken::new();
        let outcome = extract(&dmg_path, &target, ExtractFormat::Dmg, &cancel)
            .await
            .expect("extract dmg 성공해야 함");
        assert_eq!(outcome.format, ExtractFormat::Dmg);
        assert!(
            outcome.entries >= 1,
            "최소 1 entry 이상이어야 함: got {}",
            outcome.entries
        );
        let copied = std::fs::read_to_string(target.join("hello.txt"))
            .expect("hello.txt 가 target에 존재해야 함");
        assert_eq!(copied, "hello dmg");

        // 4) hdiutil info로 dangling mount 없음 확인 (로그만 — 환경 의존성 강함).
        if let Ok(info) = Command::new("hdiutil").args(["info"]).output() {
            let stdout = String::from_utf8_lossy(&info.stdout);
            assert!(
                !stdout.contains(&device),
                "extract 종료 후 device {device}가 마운트 상태에 남아있음:\n{stdout}"
            );
        }
    }
}

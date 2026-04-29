# ADR-0039: Portable workspace export/import — single zip + AES-GCM key wrap

- Status: Accepted
- Date: 2026-04-29
- Phase: 11'

## Context

ADR-0009 "Portable workspace"는 6 pillar 약속 중 하나로 manifest 기반 워크스페이스를 정의했지만, 사용자가 실제로 워크스페이스를 *옮기는 경험*은 절반만 구현돼 있었어요:

- ✅ `crates/portable-workspace`에 fingerprint / 3-tier repair / manifest / paths 모듈 구현 (Phase 3'.c).
- ❌ zip/tar 패킹 export 부재 — 사용자는 폴더를 통째로 복사해야 했어요.
- ❌ verify-and-unpack import 부재 — 받은 PC에서 fingerprint repair만 자동, 패키징 검증은 수동.

Phase 11'는 6 pillar 약속을 완성하는 사용자 경험 절반을 채워요. Obsidian Sync / Notion export 수준의 한 번 클릭으로 zip 파일을 만들고, 다른 PC에서 검증 후 가져오는 흐름을 제공해요.

## Decision

### 1. 아카이브 포맷: zip 8.x (단일 .zip)

- `zip = "8"` workspace dep 사용. CVE-2025-29787 (symlink path-traversal) 수정 포함.
- deflate 압축 — Windows / macOS / Linux 기본 도구로 풀 수 있어요.
- **Single archive** — split archive (`.zip.001`, `.zip.002` …) 거부. NTFS / ext4 / APFS 모두 단일 파일로 충분.

### 2. 옵션 default OFF (사용자 명시 opt-in)

```rust
ExportOptions {
    include_models: false,    // 모델 파일 동봉
    include_keys: false,      // API 키 (AES-GCM wrap) 동봉
    key_passphrase: None,
    target_path: PathBuf,
}
```

- 첫 export에서 수십 GB 폭발 방지 (모델 파일은 Q4_K_M GGUF가 4~8 GB).
- 키 포함은 별도 패스프레이즈 필수 (`MissingPassphrase` 에러).

### 3. 키 wrap: AES-GCM + PBKDF2(100k iter)

- `aes-gcm = "0.10"` + `pbkdf2 = "0.12"` + `hmac = "0.12"` workspace dep 추가.
- 출력 layout: `salt(16) | nonce(12) | ciphertext+tag(N)`.
- OS 키체인 secret은 PC 단위라 archive에는 wrapped key만 — 받은 PC가 패스프레이즈로 unwrap.

### 4. 무결성 검증: archive sha256

- export 끝나기 전 `.tmp` 파일 전체에 sha256 계산 (64KB chunk streaming).
- import 측에서 `expected_sha256` 옵션이 있으면 비교 (`Sha256Mismatch` 에러).
- 사용자가 USB로 옮기는 동안 손상되거나, 다운로드 중간 끊겨도 감지.

### 5. dual zip-slip 방어 (import)

- `ZipFile::enclosed_name()` (zip 8.x) — 절대 경로 + `..` 컴포넌트 거부 1차.
- `lexical_safe_subpath_check` — RootDir/Prefix 거부 + ParentDir 누적이 NormalDir보다 많으면 거부 (2차).
- `installer::extract` 패턴 재사용 — Phase 1A.3.b.2 보강 리서치에서 결정한 패턴.

### 6. atomic rename: `.tmp` → final

- export: `target.zip.tmp` 작성 → close → sha256 계산 → rename. 중간 실패 시 Drop guard가 `.tmp` 삭제.
- import: `tempfile::TempDir`에 unpack → 검증 → target_workspace_root에 rename. 실패 시 tempdir Drop으로 자동 정리.

### 7. Channel<ExportEvent> / Channel<ImportEvent>

- Phase 1A.3.c InstallEvent / Phase 5'.b WorkbenchEvent와 동일 패턴.
- `#[serde(tag = "kind", rename_all = "kebab-case")]` tagged enum.
- Started / Counting / Compressing / Encrypting / Finalizing / Done / Failed (export).
- Started / Verifying / Extracting / DecryptingKeys / RepairTier / Done / Failed (import).
- `PortableRegistry` (export_id / import_id ↔ CancellationToken) — 동시 다중 작업 + cancel 보장.

### 8. ConflictPolicy 3분기 (import)

- `Skip` — 이미 존재하면 `TargetExists` 에러.
- `Overwrite` — `remove_dir_all` 후 import.
- `Rename` — `_imported_<unix_timestamp>` suffix 디렉터리 생성 (default).

### 9. fingerprint.source.json 동봉

- export 측 PC의 `WorkspaceFingerprint`를 archive에 추가 — 받은 PC가 manifest.host_fingerprint와 비교해 tier 산출.
- ADR-0022 §8 3-tier (green / yellow / red)와 호환.

## Consequences

### Positive

- ✅ 사용자가 USB / 클라우드로 단일 .zip 파일을 옮기는 친화적 경험 (Obsidian Sync 수준).
- ✅ 모델 파일 / 키 / 메타데이터를 옵션별로 선택 가능 — 사이즈 vs 편의성 trade-off 사용자 결정.
- ✅ archive sha256으로 손상 자동 감지.
- ✅ 외부 통신 0 정책 유지 — 모든 작업 로컬.
- ✅ ADR-0009 6 pillar 약속의 사용자 경험 절반 완성.

### Negative

- ⚠️ archive 사이즈가 클 수 있어요 (모델 포함 시 수십 GB). 압축 시간 + 디스크 IO 부담.
- ⚠️ AES-GCM + PBKDF2(100k)는 충분한 보안이지만 패스프레이즈 분실 시 키 복구 불가. 사용자 책임 명시.
- ⚠️ 아직 file-picker dialog는 없어 사용자가 절대 경로를 직접 입력해야 해요. v1.x에서 `tauri-plugin-dialog` 추가 검토.

### 미정 / 후순위 이월

- v1.x — `tauri-plugin-dialog` 통합으로 OS-native 파일 선택 dialog.
- v1.x — split archive 옵션 (FAT32 4GB 한계 사용자 대응).
- v1.x — incremental / differential export (변경분만).
- v1.x — 클라우드 sync provider 추상화 (rclone / S3 호환).

## Alternatives considered

### 1. ❌ 7zip / RAR
- 더 높은 압축률이지만 추가 의존성 + 라이선스 부담 (RAR 폐쇄 라이선스).
- zip 8.x deflate가 GGUF 모델 파일에는 충분 (이미 압축된 binary라 추가 압축률 < 5%).

### 2. ❌ 모델 포함 default ON
- 사용자가 첫 export 클릭 시 수십 GB가 갑자기 발생할 수 있어요.
- "내 PC에 무엇이 있는지 다 모르는 상태"에서 옵션 ON은 위험 — 명시적 opt-in이 안전.

### 3. ❌ Split archive (`.zip.001` / `.zip.002` / …)
- Windows NTFS / macOS APFS / Linux ext4 모두 단일 파일로 충분.
- FAT32(4GB 한계)에 USB로 옮기는 사용자는 사용자가 압축 도구로 split 가능. v1 에서는 단순화.

### 4. ❌ 클라우드 sync (S3 / Dropbox 직접 통합)
- 외부 통신 0 정책 위반. Phase 11' scope에서 거부.
- 사용자가 사용자 도구(rclone / Dropbox 데스크톱 앱)로 manual sync하면 충분.

### 5. ❌ ChaCha20-Poly1305
- AES-GCM 대비 차별점 없음. AES-NI 하드웨어 가속이 보편적이라 AES-GCM이 표준.

### 6. ❌ Argon2 (vs PBKDF2)
- Argon2가 더 강하지만 의존성 트리 + 첫 패스프레이즈 derive 시간 trade-off.
- PBKDF2(100k)는 OWASP 권장치 충족 + 사용자 친화적 응답성.

## Test invariant (v1 동안 깨지면 안 되는 동작)

### 11'.a Export
- 모델 미포함 export → 다른 PC import 후 catalog에서 다시 받아야 함 (manifest는 전달).
- 모델 포함 export → import 후 즉시 사용 가능.
- 키 미포함 export — 사용자가 새 PC에서 재발급.
- 키 포함 + 잘못된 패스프레이즈 → `WrongPassphrase` 에러 (panic X).
- Archive sha256 mismatch → `Sha256Mismatch` 에러.
- Cancel mid-export → `.tmp` 정리, target 미생성.
- Atomic rename — 부분 파일 디스크에 남으면 안 돼요.

### 11'.b Import
- 손상된 archive → `Corrupted` 에러 (사용자 행동 가능).
- 다른 OS archive → `RepairTier::Red` + 한국어 fallback "이 PC와 OS 계열이 달라요".
- conflict_policy 모든 분기 (Skip → TargetExists / Overwrite → 기존 삭제 / Rename → suffix 디렉터리).
- 잘못된 패스프레이즈 + 키 포함 archive → `WrongPassphrase`.
- Cancel mid-import → 임시 디렉터리 정리, target unchanged.

### 11'.c UI
- Export 옵션 dialog의 default = 모델 OFF + 키 OFF.
- 키 ON 시에만 패스프레이즈 입력 노출.
- Esc 키로 옵션 dialog 닫힘 (idle phase 복귀).
- 진행률 progress bar + 현재 entry 경로 표시.
- 완료 후 sha256 + 사이즈 + 경로 표시 (다른 PC에서 검증 가능).
- import 측 verify → preview ("어떤 PC, 언제, 사이즈, 모델 포함 여부").
- repair_tier 안내 (green / yellow / red) — 사용자 한국어 해요체.

## References

- ADR-0009 (Portable workspace)
- ADR-0022 §8 (3-tier fingerprint repair)
- `docs/research/phase-8p-9p-10p-residual-plan.md` §1.8 (Phase 11' 상세 설계)
- `crates/installer/src/extract.rs` (zip-slip dual-defense 패턴)
- OWASP Password Storage Cheat Sheet (PBKDF2 iter 권장치)
- RFC 5208 / RFC 8018 (PKCS #5 / PBKDF2)
- NIST SP 800-38D (GCM 모드)

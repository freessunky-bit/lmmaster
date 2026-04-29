# Phase 1A.3.b.3 — dmg macOS 추출 결정 노트

> 보강 리서치 (2026-04-26) 종합. 별도 reinforcement 문서 대신 결정 노트로 압축.

## 1. 핵심 결정

| 항목 | 결정 | 근거 |
|---|---|---|
| 마운트 도구 | `hdiutil attach -plist -nobrowse -readonly -noautoopen -noverify -mountrandom <tmp> <dmg>` | Apple 공식 + Homebrew Cask 패턴 |
| stdin 처리 | `"qn\n"` 파이프 (EULA pager 회피, 자동 동의는 안 함) | Homebrew brew/unpack_strategy/dmg.rb |
| plist 파싱 | `plist` crate 1.7+ (target-cfg = macos) | `serde_plist`은 2024 이후 unmaintained, `dmg` crate가 의존 |
| Detach 보장 | `MountGuard { device }` Drop impl — 비-force 시도 → exit 16 시 -force fallback | `dmg` crate의 `expect("could not detach")` 개선판 |
| 복사 도구 | `/usr/bin/ditto <mount_point> <target>` | xattr/resource fork/ACL 보존, .app 코드사인/공증 무결성 유지 (`cp -R`은 깨뜨림) |
| Cancel 통합 | `std::process::Command::spawn` + 100ms `try_wait` 폴링 + 취소 시 `child.kill()` | spawn_blocking 안에서 tokio::process는 두 런타임 충돌 위험. 기존 zip 64KB 폴링 cadence와 동일 |
| Entry/byte 카운트 | ditto 종료 후 `walkdir`로 post-walk | ditto는 진행률 출력이 없음 (`-V`는 파싱 불안정) |
| 다중 볼륨 dmg | `system-entities`에서 첫 번째 mount-point만 사용. >1이면 warn 로그 | LM Studio/Ollama류는 단일 볼륨. 멀티 볼륨은 별도 enhancement |
| EULA 자동 수락 | 하지 않음. `qn\n`만 보내고 attach가 hang하면 명확한 한국어 에러 | 사용자 동의 없이 라이선스 수락은 법적 리스크 |
| 테스트 픽스처 | `hdiutil create -size 1m -fs HFS+ -volname LMmasterTest -ov <out.dmg>` | pure-Rust dmg 작성 불가 (HFS+/APFS 미공개), Tauri bundler도 동일 패턴 |

## 2. 새 ExtractError 변형

```rust
#[error("dmg attach 실패 ({0})")]
DmgAttachFailed(String),

#[error("dmg plist parse 실패: {0}")]
DmgPlist(String),

#[error("ditto 복사 실패 (exit code {0:?})")]
DittoFailed(Option<i32>),
```

## 3. Cargo.toml 추가 (target-cfg = macos)

```toml
# workspace.dependencies
plist = "1"
walkdir = "2"

# crates/installer/Cargo.toml
[target.'cfg(target_os = "macos")'.dependencies]
plist.workspace = true
walkdir.workspace = true
```

## 4. 테스트 전략

- **Win/Linux CI**: 기존 `extract_dmg_returns_unsupported_on_non_mac` 유지 — `DmgRequiresMac` 즉시 반환.
- **macOS**: `#[cfg(target_os = "macos")] #[tokio::test] #[ignore]` — `cargo test -- --ignored extract_dmg`로만 실행.
- 픽스처: `hdiutil create`로 1MB HFS+ dmg 생성 → mount/sentinel 작성/detach → extract 호출 → assert.
- Cancel 테스트: 50MB dmg + 100ms 후 cancel → `Err(Cancelled)` + target 정리 + `hdiutil info`에 dangling mount 없음 확인.

## 5. 참고 구현체

- [`dmg` crate](https://docs.rs/dmg/) — Drop 패턴 idiom 차용 (의존성으로 추가하지 않음 — `-noautoopen`/EULA/progress 미지원)
- [Homebrew brew/unpack_strategy/dmg.rb](https://github.com/Homebrew/brew/blob/master/Library/Homebrew/unpack_strategy/dmg.rb) — gold standard
- [node-appdmg PR #190](https://github.com/LinusU/node-appdmg/pull/190) — exit 16 (busy) retry-with-force 근거

## 6. 비목표 (이번 sub-phase 범위 외)

- EULA-protected dmg 자동 수락 / `hdiutil convert` CDR fallback — Phase 후순위 (현재 LM Studio/Ollama류 dmg는 EULA 없음).
- 멀티 볼륨 dmg 전체 마운트.
- 진행률 세밀화 (Progress::Indeterminate phase=extracting 단일 이벤트로 시작).
- DMG signature/notarization 검증 — Tauri-plugin-updater의 영역.

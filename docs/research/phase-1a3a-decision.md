# Phase 1A.3.a — 다운로더 구현 결정 (보강 없이 1A 종합 적용)

> Phase 1A 보강(`docs/research/phase-1a-reinforcement.md` §2)에서 다운로드 stack을 충분히 구체적으로 다뤘다. 본 sub-phase는 그 결정을 코드로 옮기는 단계 — 별도 보강 리서치 없이 진행.

## 적용할 이전 결정 요약 (참조 only)

| 항목 | 결정 (출처) |
|---|---|
| HTTP client | `reqwest 0.12` + `stream` feature (이미 워크스페이스) |
| Resume | `Range: bytes=<from>-` 헤더; 응답 코드 206=partial OK, 200=서버가 Range 무시 → 재시작 + hasher 리셋 |
| Streaming sha256 | `sha2::Sha256::update`를 매 chunk마다. hasher 상태가 진실원 — 디스크 재읽기 금지 |
| Retry | `backon 1.6` `ExponentialBuilder::default().with_jitter()` (RUSTSEC-2025-0012로 backoff crate 사용 금지) |
| Atomic rename | `atomicwrites 0.4` (Win `ReplaceFileW`, ACL 보존) + AV-locked 대비 backon 5회 retry |
| Cancellation | `tokio::select! { _ = cancel.cancelled() => ..., next = stream.next() => ... }` |
| 취소 시 정리 | `.partial` 보존 — 다음 실행에서 resume |
| Hash mismatch 시 | `.partial` 삭제 + 재시도는 backon이 결정 |
| Progress 전송 | 256KB 또는 100ms 누적 후 send (rustup/cargo/indicatif 표준) |
| ETag/If-Modified-Since | 큰 파일 download엔 무관. 작은 manifest fetch는 별도 cache 모듈(1A.3.b 이후) |
| Tauri 2 updater 재사용 | **금지** — Range/resume/sha256 미구현 |

## 본 sub-phase 산출물

1. `crates/installer/Cargo.toml` — backon, atomicwrites, tempfile, sha2, reqwest, tokio-util[sync] 등.
2. `crates/installer/src/lib.rs` — public API.
3. `crates/installer/src/downloader.rs` — `Downloader` 구조체, `DownloadRequest`/`DownloadEvent`/`DownloadOutcome` 타입, `download()` 함수.
4. `crates/installer/src/error.rs` — `DownloadError` (thiserror).
5. `crates/installer/tests/download_test.rs` — wiremock 기반 통합 테스트:
   - 정상 다운로드 + sha256 검증
   - sha256 mismatch → 에러 + .partial 삭제
   - Range 응답(206)으로 resume
   - 서버 Range 무시(200) → 재시작
   - 취소(CancellationToken)
   - retry on 5xx
   - progress event throttle 동작

## 다음 sub-phase (1A.3.b) 예고

- Pinokio install action executor — manifest의 `install` 객체(다중 platform, args, post_install_check) 실행기
- `tauri-plugin-shell` 통합 + capability JSON에 `OllamaSetup.exe`, `lms` 등록
- Tauri 2 `Channel<DownloadEvent>` 직접 통합
- `post_install_check` — manifest evaluator 재호출하여 검증

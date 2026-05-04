# Phase R-C — Network + Correctness 결정 노트

> 2026-05-03. GPT Pro 정적 검수 30건 중 v0.0.1 ship-blocker 신뢰성/네트워크(S7+C1+R3+C3) 4건을 본 sub-phase에서 해소.

## 1. 결정 요약

- **D1 (S7)**: 외부 통신 reqwest::Client 7개 사이트에 `.no_proxy()` 추가. ADR-0013 + ADR-0026 일관 + rogue proxy MITM 방어.
- **D2 (R3)**: 모든 `.unwrap_or_else(|_| reqwest::Client::new())` 패턴 폴백 제거 → `.expect("reqwest Client builder must succeed (TLS init)")`. 폴백이 정책 우회 통로.
- **D3 (C1)**: chat_stream 3개 어댑터(adapter-ollama/lmstudio/llama-cpp) 모두 `delta_emitted` flag 추가. transport 에러 시 delta 1건 이상 emit됐으면 graceful Completed.
- **D4 (C3)**: `derive_filename`에 path traversal 검증 5건 추가 — `..` / `.` / `\` / 제어 문자 / Windows drive letter prefix 거부. 6 회귀 invariant.
- **D5**: ADR-0055 (network policy + stream resilience) 단일 ADR로 4건 묶음 (모두 *외부 경계 enforcement* 같은 본질).

## 2. 채택안

### D1 — `.no_proxy()` 추가 사이트

| 사이트 | 목적 | 추가 전 | 추가 후 |
|---|---|---|---|
| `crates/registry-fetcher/src/lib.rs:94` | catalog + manifest fetch (GitHub/jsDelivr) | 없음 | `.no_proxy()` |
| `crates/auto-updater/src/source.rs:62` | release feed (GitHub) | 없음 | `.no_proxy()` |
| `crates/installer/src/downloader.rs:55` | 모델/installer download (HF + 등) | 없음 | `.no_proxy()` |
| `crates/knowledge-stack/src/embed_download.rs:219` | HF 임베더 download | 없음 | `.no_proxy()` |
| `apps/desktop/src-tauri/src/telemetry/submit.rs:208` | GlitchTip self-hosted | 없음 | `.no_proxy()` |
| `apps/desktop/src-tauri/src/lib.rs:221` | HF metadata cron | 없음 | `.no_proxy()` |
| `apps/desktop/src-tauri/src/hf_search.rs:124` | HF Search API | 없음 | `.no_proxy()` |

기존 적용 사이트(adapter-* / runtime-detector / scanner / runner-llama-cpp / core-gateway / bench-harness / installer/action.rs)는 변경 없음.

### D2 — `.expect()` 폴백 제거 사이트

총 9 사이트:
- `crates/registry-fetcher/src/lib.rs`
- `crates/auto-updater/src/source.rs`
- `crates/adapter-ollama/src/lib.rs`
- `crates/adapter-lmstudio/src/lib.rs`
- `crates/adapter-llama-cpp/src/lib.rs`
- `crates/installer/src/action.rs::with_downloader`
- `crates/bench-harness/src/workbench_responder.rs::default_client`
- `apps/desktop/src-tauri/src/lib.rs:221`
- `apps/desktop/src-tauri/src/hf_search.rs`

모두 `.unwrap_or_else(|_| reqwest::Client::new())` → `.expect("reqwest Client builder must succeed (TLS init)")`로 통일.

테스트 사이트(`reqwest::Client::new()` 단독, `cfg(test)` 영역) 3건은 그대로 유지 (실 빌드 영향 0):
- `crates/registry-fetcher/src/fetcher.rs:402` (test)
- `apps/desktop/src-tauri/src/hf_search.rs:193` (test)
- `crates/auto-updater/src/error.rs:81` (test)

### D3 — chat_stream graceful early disconnect

3개 어댑터 동일 패턴:

```rust
let mut delta_emitted = false;

loop {
    tokio::select! {
        () = cancel.cancelled() => return ChatOutcome::Cancelled,
        next = stream.next() => match next {
            Some(Ok(bytes)) => {
                // ...
                if !text.is_empty() {
                    delta_emitted = true;
                    on_event(ChatEvent::Delta { text });
                }
            }
            Some(Err(e)) => {
                // Phase R-C — delta 1건 이상이면 graceful early disconnect.
                if delta_emitted {
                    tracing::warn!(error = %e, "스트림 중단 — 부분 응답으로 마감");
                    on_event(ChatEvent::Completed { took_ms });
                    return ChatOutcome::Completed;
                }
                let msg = format!("응답 읽기 실패: {e}");
                on_event(ChatEvent::Failed { message: msg.clone() });
                return ChatOutcome::Failed(msg);
            }
            None => { /* EOF — 기존대로 Completed */ }
        }
    }
}
```

### D4 — `derive_filename` path traversal hardening

```rust
pub(crate) fn derive_filename(url: &str) -> Result<String, ActionError> {
    // 기존 path 추출 + 빈 결과 거부.
    let last = ...;
    if last.is_empty() { return Err(...); }

    // Phase R-C 신규 검증:
    if last == "." || last == ".." {
        return Err(InvalidSpec("path traversal"));
    }
    if last.chars().any(|c| c == '/' || c == '\\') {
        return Err(InvalidSpec("path separator"));
    }
    if last.chars().any(|c| c == '\0' || c.is_control()) {
        return Err(InvalidSpec("control char"));
    }
    if last.len() >= 2
        && last.as_bytes()[1] == b':'
        && last.as_bytes()[0].is_ascii_alphabetic()
    {
        return Err(InvalidSpec("Windows drive letter"));
    }
    Ok(last.to_string())
}
```

## 3. 기각안 + 이유

| # | 기각안 | 이유 |
|---|---|---|
| 1 | 모든 reqwest 클라이언트에 system proxy 옵션 추가 | 정책 일관성 깨짐. corporate user는 외부 브라우저로 명시적 우회 가능 |
| 2 | C1 graceful 정책을 catch-all (모든 transport 에러 → Completed) | delta 0건일 때(connect failed) Completed → 사용자 혼란 |
| 3 | C1을 별개 ChatOutcome::CompletedPartial variant | frontend 추가 분기 + i18n. 현재 graceful Completed + warn log가 ROI 균형 |
| 4 | R3 폴백을 Result 전파 (`.build()?`) | 모든 caller 시그니처 변경 — Tauri command, async spawn 다수. expect로 fail-fast가 간결 |
| 5 | C3 derive_filename을 url::Url crate로 정규화 | url crate dep 추가 + 기존 파서 동작 변경. 명시적 검증 5건이 더 audit 친화적 |
| 6 | C3 검증을 cache_dir.join 후 canonicalize + starts_with | 비동기 / 디렉터리 미존재 이슈 + 검증 시점 늦음. URL 단계 거부 간결 |
| 7 | chat_stream graceful을 timer 기반 (silent N초 → Completed) | 복잡 + 테스트 어려움. 단순 flag가 의도 명확 |
| 8 | `.no_proxy()`를 환경별로 (개발 빌드만) | 정책 일관성 깨짐. proxy 사용은 *명시 opt-in*이 맞음 |
| 9 | derive_filename Windows drive letter 검증을 cfg(windows)만 | macOS/Linux에서 `C:foo.exe`라는 파일명 헷갈림. 모든 OS 거부 정합 |
| 10 | C1 fix를 chat IPC layer에서 | 어댑터별 transport 에러 의미가 다름 (NDJSON vs SSE). 어댑터 layer 정합 |
| 11 | derive_filename에 file extension 화이트리스트 | manifest 큐레이션이 이미 사용 (확장자 검증 + sha256). 본 함수 단계는 *path 안전성*만 담당 |
| 12 | reqwest 클라이언트를 workspace 단일 lazy_static | 각 사이트별 timeout/connect_timeout 설정 다름. 단일 클라이언트는 ROI 낮음 |
| 13 | C1 delta_emitted를 Atomic으로 race 방지 | tokio::select!는 single-task — 동시 access 0. 단순 mut bool로 충분 |
| 14 | derive_filename에 max length 제한 | OS별 max filename 다름 (Windows 260, Linux 255). cache_dir.join 시 OS 자체 거부 — 별도 검증 ROI 낮음 |

## 4. 미정 / 후순위 이월

- **catalog 외 manifest signature verify** — `ollama.json`, `lm-studio.json` 등도 verify 적용 (현재 catalog만). v1.x 후속 ADR.
- **proxy 명시 opt-in 설정** — Settings 페이지에 "회사 proxy 사용" 토글 + corporate user 대상. v1.x 후속 — 현재는 명시적 외부 브라우저 우회.
- **C3 화이트리스트 호스트 검증** — derive_filename은 *filename 안전성*만, *호스트 화이트리스트*는 manifest 큐레이션이 담당. v1.x에서 manifest URL https + 화이트리스트 검증 추가 가능.
- **C1 partial response UI 표시** — 현재 사용자는 "Completed"로 인식 (정상 마감 토스트). v1.x에서 `tracing::warn` 기반 Diagnostics 카드 추가 검토.
- **lmmaster-desktop crate 단위 테스트 Windows DLL 한계** — 변경 없음. R-A/R-B에서 documented + workspace 통합 테스트가 회귀 보호.

## 5. 테스트 invariant

본 sub-phase가 깨면 안 되는 invariant:

1. **외부 통신 클라이언트 .no_proxy 적용**: 7개 사이트 모두 `.no_proxy()` 호출. grep으로 회귀 가드.
2. **폴백 제거**: `unwrap_or_else(|_| reqwest::Client::new())` 패턴 0건 (test 사이트 제외).
3. **chat_stream delta_emitted graceful**: transport 에러 + delta_emitted=true → Completed 반환. (단위 테스트는 `cfg(test)` 영역 제한 — wiremock으로 검증 가능, 현재는 코드 audit으로 충당.)
4. **chat_stream EOF (None)**: 기존대로 Completed (regression 0).
5. **derive_filename 정상 케이스**: GitHub release URL → 파일명 정확 추출.
6. **derive_filename `..` 거부**.
7. **derive_filename `.` 거부**.
8. **derive_filename `\` 거부** (Windows path separator).
9. **derive_filename 제어 문자 거부** (`\0`, `\n`).
10. **derive_filename Windows drive letter 거부** (`C:foo.exe`).
11. **derive_filename path 중간 `..` 통과**: last segment가 정상 파일이면 OK.

본 sub-phase 신규 invariant: **+6** (installer derive_filename 6건). 기존 invariant 0건 깨짐.

## 6. 다음 페이즈 인계

### 진입 조건

- ✅ R-C.1 (S7 .no_proxy + R3 폴백 제거) 완료
- ✅ R-C.2 (C1 chat_stream graceful) 완료
- ✅ R-C.3 (R3 폴백 제거 — R-C.1과 묶음)
- ✅ R-C.4 (C3 derive_filename hardening) 완료
- ✅ R-C.5 (ADR-0055 + 결정 노트) 완료
- ⏳ commit + push (사용자 승인 대기)

### 의존성

- **Phase R-D** (Frontend Polish) — K1+K2+K3 i18n emoji 제거 + Catalog hardcoded fallback + thiserror 한국어 + `errors.path-denied`/`errors.path-traversal` 키 추가. 본 R-C와 무관 → 병렬 가능.
- **Phase R-E** (Architecture v1.x) — A1 chat protocol decoupling + A2 bench trait + C2 OpenAI compat 공통화 + P1 KnowledgeStorePool + P4 channel cancel + R2 cancellation token + T3 wiremock — POST v0.0.1 release.
- **#31 (Knowledge IPC tokenized)** — R-A 분리분.
- **#38 (knowledge-stack caller wiring)** — R-B 분리분.

### 위험 노트

- **`.no_proxy()` 강제로 corporate user 외부 통신 깨짐**: GitHub/HF 직접 연결 불가 환경에서 catalog refresh / 모델 다운로드 fail. README + Diagnostics에서 proxy 환경 안내 필요 (Phase R-D 묶음).
- **chat_stream graceful 잘못된 trust**: 모델 추론 중 *실제 에러*(server crash, OOM)인데 delta가 emit된 후라면 Completed로 마감. 사용자가 "응답이 잘렸나?" 인지 어려움. 현재는 `tracing::warn` 로그만 — Diagnostics UI 표시는 v1.x.
- **derive_filename 단일 검증**: cache_dir.join 후 canonicalize 검증은 별도 layer (target_path_is_safe). 두 layer가 정합 — derive_filename은 *URL 단계* 1차 거부.
- **`.expect()` panic in production**: TLS init 실패는 사실상 0 케이스. 발생 시 panic이 audit trail. crash report (Phase 13'.c)로 사용자에게 표면화.

### 다음 standby

**Phase R-D.1** (K1+K2+K3 i18n 마무리) — 한국어/영어 키 누락 검사 + 이모지 인라인 거부 (lucide-react로 대체 — Phase 14' v1 정책 일관) + `errors.path-denied` (R-A) + `errors.path-traversal` (R-C) + thiserror 메시지 frontend 노출 정책 확정. UI 변경 동반이라 vitest a11y 테스트 묶음.

# ADR-0055 — Network 정책 강화: no_proxy + 폴백 제거 + chat 스트림 graceful + URL filename validation

* **상태**: Accepted (2026-05-03). Phase R-C 머지와 함께 적용.
* **선행**: ADR-0026 (Auto-Updater + GitHub Releases 화이트리스트). ADR-0013 (Gemini boundary — 외부 통신 0 정책). ADR-0052 (R-A path boundary — 같은 패턴 재적용). ADR-0042 (Real Embedder ONNX cascade — HF 도메인 화이트리스트).
* **컨텍스트**: 2026-05-02 GPT Pro 검수에서 v0.0.1 ship-blocker 4건이 신뢰성/네트워크 카테고리에서 식별됨.
  1. **S7**: 외부 통신 클라이언트 일부에 `.no_proxy()` 누락 (registry-fetcher / hf_search / lib.rs auto-updater spawn / installer downloader / knowledge-stack embed_download / telemetry / auto-updater::source). 시스템 `HTTP_PROXY`/`HTTPS_PROXY` env가 설정된 환경에서 rogue proxy 통한 MITM 가능.
  2. **C1**: chat_stream의 `Some(Err(e))` 분기가 *모든 transport 에러*를 Failed로 처리. delta가 1건 이상 emit된 후 graceful early disconnect도 Failed로 보고됨 → 사용자에게 부분 응답 noise.
  3. **R3**: 다수 reqwest::Client 생성 사이트가 `.build().unwrap_or_else(|_| reqwest::Client::new())` 패턴 — fallback이 시스템 proxy를 *준수*해 정책 우회 통로.
  4. **C3**: `installer::derive_filename(url)` 함수가 path traversal 검증 없음 — `..` / `.` / `\` / 제어 문자 / Windows drive letter prefix 거부 X. cache_dir.join 시 escape 가능.
* **결정 노트**: `docs/research/phase-r-c-network-correctness-decision.md`

## 결정

1. **모든 *외부 통신* reqwest::Client에 `.no_proxy()` 강제** — 본 sub-phase 적용 사이트:
   - `crates/registry-fetcher/src/lib.rs` (catalog + manifest fetch)
   - `crates/auto-updater/src/source.rs` (release feed)
   - `crates/installer/src/downloader.rs` (모델/installer 다운로드)
   - `crates/knowledge-stack/src/embed_download.rs` (HF 임베더 다운로드)
   - `apps/desktop/src-tauri/src/telemetry/submit.rs` (GlitchTip)
   - `apps/desktop/src-tauri/src/lib.rs:221` (HF metadata cron)
   - `apps/desktop/src-tauri/src/hf_search.rs` (HF Search API)
   - 기존 `.no_proxy()` 적용된 로컬 통신 클라이언트(adapter-* / runtime-detector / scanner / runner-llama-cpp / core-gateway / bench-harness)는 변경 없음.
2. **`Client::new()` 폴백 제거** — 모든 `.build().unwrap_or_else(|_| reqwest::Client::new())` 패턴을 `.expect("reqwest Client builder must succeed (TLS init)")`로 전환. 폴백은 정책(.no_proxy + timeout)을 *우회*하므로 제거.
3. **chat_stream graceful early disconnect** — adapter-ollama / adapter-lmstudio / adapter-llama-cpp 모두 `delta_emitted` flag 추가. transport 에러 발생 시:
   - `delta_emitted == true` → `tracing::warn!` + `Completed` 반환 (부분 응답 정상 마감, ChatOutcome::Completed).
   - `delta_emitted == false` → 기존대로 `Failed` (실 에러).
4. **`derive_filename` path traversal 강화** — `crates/installer/src/action.rs::derive_filename`에 5개 검증 추가:
   - `last == "." || last == ".."` → InvalidSpec
   - `last.contains('/' or '\\')` → InvalidSpec (Windows path separator escape 방어)
   - `last.contains('\0' or control)` → InvalidSpec
   - Windows drive letter prefix(`C:`, `D:`, ...) → InvalidSpec
   - 빈 결과는 기존대로 InvalidSpec
5. **테스트 invariant 6건** — `derive_filename` 회귀 가드:
   - `derive_filename_rejects_dot_segment` (`.` 단독)
   - `derive_filename_rejects_parent_dir_segment` (`..` 단독)
   - `derive_filename_rejects_backslash_in_filename` (Windows 분리자)
   - `derive_filename_rejects_control_chars` (`\n` / `\0`)
   - `derive_filename_rejects_windows_drive_letter` (`C:foo.exe`)
   - `derive_filename_accepts_legitimate_traversal_in_path` (path 중간 `..` OK if last segment normal)

## 근거

- **`.no_proxy()` 강제 = ADR-0013 + ADR-0026 자연 확장**: 이미 로컬 어댑터(adapter-* / runtime-detector)는 적용됨. 외부 통신 사이트도 동일 정책으로 수렴 — rogue proxy 통한 MITM 방어 + corporate proxy 의도치 않은 우회 방지.
- **폴백 제거 = fail-fast 정공**: `reqwest::Client::builder().build()` 실패는 *TLS init 이슈*만 — 사실상 전체 OS TLS 라이브러리 깨졌을 때만. 이런 환경에서 Client::new() 폴백이 작동할 가능성 0. 실패 시 panic이 audit trail 명확.
- **delta_emitted graceful**: 사용자 PC가 모델 추론 중간에 transport drop(LM Studio/Ollama 시 의도적 disconnect 포함) 시 부분 응답이 이미 화면에 표시됨. 그 후 "Failed" toast가 뜨면 사용자가 혼란. delta가 emit됐다면 *마감 정상* 표시가 정합.
- **derive_filename path traversal**: `cache_dir.join("..")` 는 `cache_dir`의 부모 디렉터리를 가리킴 — 공격자가 cache_dir 외부에 임의 파일 작성 가능. R-A.2 portable import path boundary와 동일한 공격 벡터 (다른 surface).
- **path 중간 `..` 통과 OK**: URL `https://x/foo/../bar.exe`의 last segment는 `bar.exe`. 실제 경로 traversal은 *마지막 segment*에서만 발생. URL 정규화는 reqwest/HTTP 서버가 처리.

## 거부된 대안

1. **모든 reqwest 클라이언트에 system proxy 옵션 추가**: 정책 일관성 깨짐. corporate user는 명시적으로 `tauri-plugin-shell::open` 또는 외부 브라우저로 우회 가능.
2. **C1 graceful 정책을 catch-all (모든 transport 에러를 Completed)**: delta 0건일 때(connect failed 등 진짜 에러)도 Completed → 사용자가 "왜 빈 응답?" 혼란. delta 1건 이상이라는 *증거* 필요.
3. **C1을 별개 ChatOutcome::CompletedPartial variant로**: frontend 추가 분기 필요 + i18n. 현재 graceful Completed + warn log가 ROI 균형 (사용자 UX는 "정상 완료"로 인식).
4. **R3 폴백을 `Result` 전파로 (.build()?)**: 모든 caller 시그니처 변경 필요 (Tauri command, async spawn 등). expect로 fail-fast가 더 간결 + production은 거의 발생 X.
5. **C3 derive_filename을 url::Url crate로 정규화**: url crate dep 추가 + 기존 단순 파서 동작 변경. 명시적 검증 5건이 더 audit 친화적 + dep 추가 0.
6. **C3 검증을 cache_dir.join 후 canonicalize + starts_with**: R-A.2 패턴이지만 비동기 / 디렉터리 미존재 이슈 + 검증 시점 늦음. URL 단계에서 거부가 간결 + 일관.
7. **chat_stream graceful 정책을 timer 기반 (마지막 delta로부터 N초 silent → Completed)**: 복잡 + 테스트 어려움. 단순 flag가 의도 명확.
8. **`.no_proxy()`를 환경별로(개발 빌드만)**: 정책 일관성 깨짐. proxy 사용은 *명시 opt-in*이 맞음 (env var 설정만으로는 부족).
9. **derive_filename Windows drive letter 검증을 cfg(windows)만**: macOS/Linux에서 `C:foo.exe`라는 파일명을 받아들이면 cross-platform 헷갈림. 모든 OS에서 거부가 정합.
10. **C1 fix를 chat IPC layer (apps/desktop/src-tauri/src/chat/)에서**: 어댑터별로 transport 에러 의미가 다름 (Ollama NDJSON vs OpenAI SSE). 어댑터 layer가 정합.

## 결과 / 영향

- **`crates/registry-fetcher/src/lib.rs`**: `.no_proxy()` + `.expect()` 폴백 제거.
- **`crates/auto-updater/src/source.rs`**: `.no_proxy()` + `.expect()` 폴백 제거.
- **`crates/installer/src/downloader.rs`**: `.no_proxy()` 추가.
- **`crates/installer/src/action.rs`**: `.expect()` 폴백 제거 (with_downloader) + `derive_filename` 5건 path traversal 검증 추가 + 6 회귀 invariant.
- **`crates/knowledge-stack/src/embed_download.rs`**: `.no_proxy()` 추가.
- **`crates/adapter-ollama/src/lib.rs`**: `delta_emitted` graceful + `.expect()` 폴백 제거.
- **`crates/adapter-lmstudio/src/lib.rs`**: `delta_emitted` graceful + `.expect()` 폴백 제거.
- **`crates/adapter-llama-cpp/src/lib.rs`**: `delta_emitted` graceful + `.expect()` 폴백 제거.
- **`crates/bench-harness/src/workbench_responder.rs::default_client`**: `.expect()` 폴백 제거.
- **`apps/desktop/src-tauri/src/lib.rs:221`**: `.no_proxy()` + `.expect()` 폴백 제거.
- **`apps/desktop/src-tauri/src/hf_search.rs`**: `.no_proxy()` + `.expect()` 폴백 제거.
- **`apps/desktop/src-tauri/src/telemetry/submit.rs`**: `.no_proxy()` 추가.
- **백워드 호환**:
  - 사용자 corporate proxy 환경에서 외부 통신은 *직접 연결 시도* — proxy 우회가 의도된 동작.
  - chat_stream graceful: 기존 "Failed" toast가 뜨던 케이스 일부가 정상 마감 표시 — UX 개선.
  - derive_filename: 기존 `..` URL은 InvalidSpec → 호출자(install_app IPC)가 한국어 에러 toast 노출.
- **테스트**: installer 6 신규 + 기존 / adapter-ollama 21 / adapter-lmstudio 12 / adapter-llama-cpp 10 / 회귀 0건.
- **외부 통신 정책**: 화이트리스트 변경 0 (4 도메인 그대로). 정책 *enforcement* 강화.

## References

- 결정 노트: `docs/research/phase-r-c-network-correctness-decision.md`
- GPT Pro 검수: 2026-05-02 30-issue static review (S7+C1+R3+C3 4건 본 ADR로 해소)
- 코드:
  - `crates/registry-fetcher/src/lib.rs` (no_proxy + expect)
  - `crates/auto-updater/src/source.rs` (no_proxy + expect)
  - `crates/installer/src/{downloader,action}.rs` (no_proxy + expect + derive_filename)
  - `crates/knowledge-stack/src/embed_download.rs` (no_proxy)
  - `crates/adapter-{ollama,lmstudio,llama-cpp}/src/lib.rs` (delta_emitted + expect)
  - `crates/bench-harness/src/workbench_responder.rs` (expect)
  - `apps/desktop/src-tauri/src/{lib.rs,hf_search.rs,telemetry/submit.rs}` (no_proxy + expect)
- 관련 ADR: 0013 (Gemini boundary), 0026 (auto-updater allowlist), 0042 (real embedder cascade), 0052 (R-A path boundary)
- 후속: Phase R-D (i18n + Catalog hardcoded fallback + thiserror 한국어), Phase R-E (architecture v1.x — POST v0.0.1)

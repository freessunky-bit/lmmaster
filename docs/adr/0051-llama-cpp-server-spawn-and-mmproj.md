# ADR-0051 — llama.cpp `llama-server` 자동 spawn + mmproj 페어링

* **상태**: Proposed (2026-05-03). Phase 13'.h.2.b/c 머지 시 Accepted로 승급.
* **선행**: ADR-0050 (3단 사다리 + 비전 IPC) — "부분 채택" 잔여분. ADR-0043 (외부 binary env override 패턴). ADR-0042 (HF 다운로드 + sha256 + atomic rename).
* **컨텍스트**: Phase 13'.h.1 + 13'.h.2.a 머지로 Ollama / LM Studio 두 어댑터에서 vision(이미지 입력) 모델이 작동. 그러나 `crates/adapter-llama-cpp/src/lib.rs`는 거의 전부 `unimplemented!("M2")` — llama.cpp 직접 사용자(고급)는 vision 모델을 같은 카탈로그 + 같은 IPC로 못 씀. 본 ADR은 그 격차를 메운다.
* **결정 노트**: `docs/research/phase-13ph2bc-llama-server-mmproj-decision.md`

## 결정

1. **`runner-llama-cpp` crate 신설** — adapter는 OpenAI compat HTTP client wrapping 책임, runner는 process lifecycle(spawn/port/health/stderr_map). 어댑터 단위 테스트가 wiremock만으로 격리됨.
2. **`llama-server` binary 발견 = `LMMASTER_LLAMA_SERVER_PATH` env override** — 사용자가 직접 build 또는 download. 미설정 시 한국어 안내 + Settings에 link. (Phase 9'.b LlamaQuantizer 패턴 = ADR-0043 차용)
3. **mmproj 페어링 = `ModelEntry::mmproj: Option<MmprojSpec>` 백워드 호환 필드** — `#[serde(default)]`로 기존 entries 영향 0. v1.x 시드는 gemma-3-4b 1건 백필. 자동 다운로드 IPC는 v1.x 후속(13'.h.2.c.2).
4. **Vision payload = OpenAI compat content array** — `[{type: "text"}, {type: "image_url", image_url: {url: "data:image/jpeg;base64,..."}}]` data URL 인라인. `adapter-lmstudio::convert_message_to_openai` 패턴 재사용.
5. **자식 프로세스 lifecycle**: `kill_on_drop(true)` + Windows `CREATE_NO_WINDOW` 플래그 + `TcpListener::bind("127.0.0.1:0")` 포트 자동 할당 + `/health` 200ms × 60초 backoff polling + stderr 라인 한국어 매핑.
6. **신규 IPC 0건** — 기존 `start_chat`이 `RuntimeKind::LlamaCpp` 분기로 자동 처리. ACL 변경 0건. 13'.h.1 (Ollama) + 13'.h.2.a (LM Studio)와 동일 패턴.
7. **수동 셋업 in-app 친절 wizard** (사용자 명시 결정 2026-05-03) — env override(#2) 채택의 친화도 격차를 `LlamaCppSetupWizard` 7단계 stepper로 메움. GPU 자동 감지(hardware-probe) → 권장 빌드 카드 → ggml-org Releases 직링 → 압축/경로 안내 → OS별 환경변수 명령어 + clipboard 복사 → 자동 검증. Phase 12' guide system(ADR-0040) 인프라 차용. 진입점 3종(Settings / Diagnostics / 첫 vision 시도 시 자동 modal). i18n ko/en.
8. **셋업 가이드 자동 갱신 시스템** (사용자 명시 결정 2026-05-03) — `manifests/guides/llama-cpp-setup.json` 단일 bundle을 registry-fetcher 인프라(ADR-0026 + 13'.a)로 6h polling. minisign 서명 검증(ADR-0047 차용). 큐레이터가 새 빌드 회귀(§1.7 Gemma4 CUDA SIGABRT 등) 발견 시 마커 + 워크어라운드 추가 → CI 자동 서명 → 6시간 내 사용자에 도착. 가이드 정적 = stale 위험.

## 근거

- **runner crate 분리** — `runtime-manager::supervisor` 모듈이 이미 placeholder로 존재 (`crates/runtime-manager/src/lib.rs:88-89` "Phase 5'+ llama.cpp 자식 프로세스 모드"). adapter-koboldcpp / adapter-vllm도 미래에 spawn 필요할 수 있어 일관 패턴.
- **env override** — llama.cpp 직접 사용자는 binary 직접 관리에 익숙. Tauri sidecar는 빌드 매트릭스 20+ binary × 100~500MB로 ROI 매우 낮음.
- **mmproj manifest 필드** — vision 모델은 GGUF + mmproj 두 파일 페어링. URL/sha256/size를 manifest에 명시 = 큐레이션 보장 + 사용자 직접 다운로드 안내 가능.
- **OpenAI compat 우선** — llama.cpp 본체에서 LLaVA-style endpoint deprecated. 기존 adapter-lmstudio 패턴 재사용 = 코드 중복 최소화.
- **신규 IPC 0건** — 13'.h.1/13'.h.2.a 패턴 일관. ACL drift 0으로 보안 surface 변경 없음.

## 거부된 대안

1. **adapter-llama-cpp 내부 spawn 모듈 inline**: SRP 위반. 어댑터 단위 테스트가 process 의존성 발생.
2. **Tauri sidecar `bundle.externalBin`**: 빌드 매트릭스 20+ binary, macOS notarization + Windows code signing 비용 폭증.
3. **자동 다운로드 + GPU detect**: cuBLAS/Vulkan/Metal/ROCm 백엔드별 분기 룰 + 별개 ADR 필요. v2 마이그레이션 → §결정 7 가이드 wizard로 ROI 재평가, 가이드만으로 일반 흐름 충족 시 자동 다운로드는 v2 그대로 유지.
4. **mmproj sha256 옵션 X — 항상 강제**: HF 모델 중 mmproj sha256 미명시 다수. ADR-0042 정책(None이면 사용자 경고)과 일관.
5. **별개 IPC `chat_with_image`** (ADR-0050 §"결정" 5번 원안): 13'.h.1/13'.h.2.a 머지 시점에 `start_chat` `messages[].images` 자동 처리 방향으로 합쳐짐 — 일관성.
6. **mmproj 자동 다운로드 본 sub-phase 포함**: model.gguf + mmproj 페어링 IPC + 진행률 + cancel + sha256 = 자체 sub-phase. 본 ADR은 *경로를 받으면 작동* 범위.
7. **다중 instance pool**: llama-server는 모델 1개만 로드. 다중 모델 동시 채팅은 다중 server. 단일 사용자 데스크톱에서 ROI 낮음 — v2.
8. **사용자 자율 셋업 — 가이드 X**: README 링크만 노출하면 OS별 환경변수 trial-and-error로 막힘. Phase 12' guide system 인프라 이미 있어 wizard 비용 작음. 사용자 명시 결정.
9. **가이드 정적 — 갱신 X**: llama.cpp 거의 매일 빌드 + 회귀 이슈 빌드별 변동 → 정적 가이드 1주만 지나도 stale. registry-fetcher 인프라 이미 있어 갱신 비용 작음. 사용자 명시 결정.

## 결과 / 영향

- **신규 워크스페이스 멤버**: `crates/runner-llama-cpp/`. `Cargo.toml` 워크스페이스 members 1건 추가.
- **adapter-llama-cpp Cargo.toml**: `tokio-util`(CancellationToken) + `futures-util`(StreamExt) + `runner-llama-cpp`(path dep) + dev-dep `wiremock` 추가.
- **ModelEntry**: `mmproj: Option<MmprojSpec>` 신규 필드. `MmprojSpec { url, sha256: Option<String>, size_mb }`. `#[serde(default)]` 백워드 호환.
- **카탈로그**: gemma-3-4b 1건만 백필 (v1.x 유일 vision_support 모델).
- **build-catalog-bundle.mjs**: validator에 `mmproj.url` https-only + huggingface.co 화이트리스트 검증 추가.
- **chat IPC**: `chat/mod.rs::start_chat`에 `RuntimeKind::LlamaCpp` 분기 추가. 기존 `UnsupportedRuntime` 자리.
- **외부 통신 0 정책**: localhost-only spawn(`127.0.0.1:0`), HF 도메인은 manifest URL만(다운로드는 사용자 직접 또는 v1.x 후속 IPC).
- **ACL drift 0**: capabilities/main.json 변경 없음.
- **테스트**: runner-llama-cpp ~10 invariant + adapter-llama-cpp ~7 invariant + model-registry +2 invariant + chat IPC +1 회귀 invariant = ~20 신규 테스트.

## References

- 결정 노트: `docs/research/phase-13ph2bc-llama-server-mmproj-decision.md`
- 코드:
  - `crates/runner-llama-cpp/` (신규)
  - `crates/adapter-llama-cpp/src/lib.rs` (chat_stream 추가, unimplemented! 제거)
  - `crates/model-registry/src/manifest.rs` (MmprojSpec 추가)
  - `apps/desktop/src-tauri/src/chat/mod.rs` (LlamaCpp 분기)
  - `manifests/snapshot/models/agents/gemma-3-4b.json` (mmproj 백필)
- 관련 ADR: 0042 (real embedder cascade), 0043 (workbench external binary env override), 0050 (Workbench 사다리 + 비전 IPC, 부분 채택)
- llama.cpp `tools/server`: https://github.com/ggml-org/llama.cpp/tree/master/tools/server (보강 리서치 §1에서 인용 보강 예정)

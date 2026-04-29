# Execution Phases

> LMmaster 작업 진행 전략. 각 페이즈는 **보강 리서치 → 설계 조정 → 프로덕션 구현 → 검증** 4단계로 진행한다.
> 스켈레톤(unimplemented!) 수준이 아닌, 실제로 동작하고 테스트가 통과하는 프로덕션 슬라이스를 매 페이즈에 산출한다.
>
> **2026-04-26 v1 포지셔닝 pivot 반영**: `docs/PIVOT.md` + ADR-0016/0017/0018.
> **2026-04-27 초격차 강화 보강**: 글로벌 경쟁 리서치(LM Studio·Ollama·Jan·Msty·AnythingLLM·Cherry·Open WebUI·GPT4All·Pinokio·Foundry Local·LLaMA-Factory·HF AutoTrain·Tailscale·JetBrains Toolbox 등) 결과를 7-점 thesis + 8 갭(G1~G8)으로 합산해 Phase 2'~6'에 반영. 자세한 종합은 `docs/PRODUCT.md`.

## 페이즈 운영 원칙

1. **보강 리서치 (Reinforcement)** — 시작 전 글로벌 라이브러리·GitHub·베스트 프랙티스를 조사. 결과는 `docs/research/<phase>-reinforcement.md`에 기록. 필요 시 ADR 추가/수정.
2. **설계 조정 (Adjust)** — 리서치 결과를 architecture 문서·ADR·repo tree에 반영. 변경 면적이 크면 새 ADR.
3. **프로덕션 구현 (Implement)** — 코드 작성. `unimplemented!()` 금지(해당 페이즈 책임 영역 한정). 로깅·에러 처리·테스트 포함.
4. **검증 (Verify)** — `cargo build`/`cargo test`/`pnpm build`/dev 실행 모두 통과. 실측 결과를 페이즈 문서에 기록.

## 세션 분할 정책

토큰 한계로 단일 세션에서 완성도가 떨어질 위험이 보이면:
- 빌드/테스트 통과 체크포인트에서 멈춘다.
- `docs/RESUME.md`에 현재 상태 + 다음 단계 + 미해결 결정을 기록.
- 사용자에게 새 세션 시작을 권장. 메모리는 자동 보존되므로 컨텍스트 손실 없음.

페이즈 자체를 sub-phase로 쪼개도 됨(예: Phase 1A: detector, Phase 1B: installer).

## 페이즈 표 (pivot 반영, 2026-04-26 / 초격차 보강 2026-04-27)

| Phase | 코드 | 상태 | 핵심 산출물 | 초격차 강화 (2026-04-27) | 보강 리서치 영역 |
|---|---|---|---|---|---|
| α | docs | ✅ | 18 ADR + 아키텍처 + 가이드 + 스캐폴딩 + PIVOT 산출물 | PRODUCT.md 신설 — USP/차별화/기능/가이드 단일 진입점 | 완료 |
| **0** | M0 | ✅ 코드 / 사용자 검증 대기 | Tauri+Axum end-to-end boot, /health 실연동, 디자인 토큰 적용 셸 | — | Tauri 2+embedded HTTP, Axum auto-port, dark React shells, Rust+TS 모노레포, 한국어 폰트 stack |
| **1A (new)** | M1' | 진행 중 (1A.4.b 완료 / 1A.4.c 다음) | **외부 런타임 감지·자동 설치 + 한국어 첫실행 마법사**: Ollama silent install (MIT), LM Studio 공식 설치 안내(EULA), `lms` CLI / Ollama API probe, 버전 detect, GPU/CUDA 자동 판단, manifest 기반 declarative 설치, Channel<InstallEvent> IPC | Phase 1A.3.b의 매니페스트는 이미 Pinokio 패턴 → Phase 2' G5(2-tier 거버넌스)에 자연 연결 | Pinokio detector/installer, tauri-plugin-shell ACL, LM Studio EULA, Ollama silent install, GPU detect (nvml/wgpu/system_profiler), xstate v5, Ark UI Steps |
| **2' (new)** | M2' | | **hardware probe(경량) + 카테고리 큐레이션 카탈로그**: 에이전트/캐릭터/코딩/사운드/온디바이스 5 카테고리 × 5~10 모델, Foundry-style hardware-aware 추천 배지, 점진 공개 UX | **Thesis #1 한국어 1순위** (EXAONE 4.0 32B/1.2B, HyperCLOVA-X SEED 8B Omni, Polyglot-Ko, K-EXAONE 236B-A23B) + **Thesis #4 / G5 2-tier 거버넌스** (Verified/Community Pinokio 패턴) + **G4 하드웨어 벤치마크** (LM Studio Hardware tab 패턴, 30초 token/sec 측정) | Foundry Local 카탈로그 패턴, Cherry Studio assistant preset 구조, HF metadata API, Pinokio Verified governance, LM Studio Runtime mgmt + Hardware tab, deterministic recommender (안전 마진) |
| **3' (new)** | M3' | | **Gateway proxy → LM Studio/Ollama 라우팅** + SDK + portable workspace + 기존 웹앱 통합 데모 + API 키 발급 GUI | **Thesis #2 wrap-not-replace + per-webapp scoped key** + **Thesis #3 portable workspace fingerprint repair** + **G6 per-app data segregation** + **G8 multi-provider routing policy** (local-first / cloud-fallback / 정책 driven) — ADR 0006 확장 (0022 신설 권장) | Axum SSE proxy + 직렬화(GPU contention), workspace fingerprint repair, OpenAI SDK shape, custom URL scheme, AnythingLLM workspace 패턴, Open WebUI multi-tenant |
| **4** | M4 | | 9개 한국어 UX 화면 완성 + 첫실행 마법사 통합 + command palette + 키보드 접근성 | **G2 Korean preset 100+ 번들** (Cherry Studio 300+ assistant 패턴 — 코딩/번역/법률/마케팅/의료/교육/리서치) + Tailscale-style 게이트웨이 status pill + Raycast command palette 토큰 reference + virtual list 24px row | shadcn/Radix dense info patterns, command palette UX (Raycast/Linear), virtual list, ko voice & tone consistency, Tailscale windowed UI, Toss UX writing 8원칙 |
| **4.5' (new)** | M4.5' | 신설 권장 | **Knowledge Stack zero-config RAG** — PDF/CSV/MD/DOCX/YouTube 드롭 → 즉시 채팅, per-workspace document isolation | **G1 Msty + GPT4All LocalDocs 합집합 패턴** + **Thesis #1 한글 정규화 임베딩** (HCX-Seed tokenizer alignment, 한자 mixed-script normalize) — ADR 0023 신설 권장 | Msty Knowledge Stack, GPT4All LocalDocs, Page Assist sidebar RAG, AnythingLLM Workspace document, on-device Korean embedding (HCX-Seed/EXAONE) |
| **5' (new)** | M5' | | **워크벤치 v1**: 양자화(llama-quantize) + LoRA 파인튜닝(LLaMA-Factory CLI), JSONL 데이터 인입 자동변환, 5단계 플로우, Ollama/LM Studio 등록 1-click | **Thesis #5 Korean 데이터 정합성 검증** (HCX-Seed tokenizer alignment + 한자 mixed-script + 한글-only 필터) + **GGUF→Ollama Modelfile 자동 생성** + **Korean QA evals** (pytest-shaped fixtures, AI Toolkit / Foundry Toolkit "evals-as-tests" 차용) | LLaMA-Factory CLI/configs, Unsloth optional accelerator, MLX-LM (mac), llama-quantize 진행률 파싱, JSONL chat 자동변환, AI Toolkit/Foundry Toolkit eval test runner |
| **6' (new)** | M6' | | **자동 갱신 폴러** (LM Studio/Ollama/모델 카탈로그/본체 self-update) + Gemini 한국어 도우미(opt-in, fallback templates) + STT/TTS 브릿지 + v1 출시 체크리스트 | **Thesis #6 자가스캔 + 옵트인 로컬 LLM 한국어 요약** + **Thesis #7 Pipelines 패턴 (gateway-side filter)** — ADR 0024 신설 권장 + **G3 Agent/Skill manifest** (app/model manifest 동형) + **G7 MCP host capability** + JetBrains Toolbox 3.3 "다음 실행 때 적용" 토스트 + 오프라인 미러 | tauri-plugin-updater 사인, GitHub releases polling, LM Studio changelog API, faster-whisper/piper, code signing & notarization (Win/mac), MCP spec, Open WebUI Pipelines, JetBrains Toolbox 3.3 |

## 보강 리서치 항목 카테고리

각 페이즈 보강 단계는 가능한 범위에서 다음을 포함한다:

- **공식 문서** — 해당 OSS 최신 안정 버전의 공식 reference.
- **레퍼런스 구현** — GitHub의 평판 좋은 구현체(별점·메인테이너 활동·최근성).
- **베스트 프랙티스** — 블로그 / 컨퍼런스 발표 / 표준 책의 정착된 패턴.
- **실패 사례** — 같은 문제를 시도해 실패한 사례(github issue, post-mortem).
- **라이선스/법** — 새 의존성이 매트릭스에 미치는 영향.

## 검증 체크리스트 (페이즈 종료 시)

- [ ] `cargo fmt --all -- --check` 통과
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 통과
- [ ] `cargo build --workspace` 통과
- [ ] `cargo test --workspace` 통과 (해당 페이즈에 추가된 테스트 포함)
- [ ] `pnpm install && pnpm -r build` 통과
- [ ] 데스크톱 dev 실행 후 한국어 UI 표시 확인
- [ ] 페이즈에 해당하는 핵심 user journey 1개 이상 수동 검증
- [ ] 페이즈 문서(`docs/research/<phase>-reinforcement.md`) 작성
- [ ] CHANGELOG/RESUME 업데이트

## 페이즈 종료 형식

각 페이즈 종료 시 다음을 사용자에게 보고:
- 무엇이 동작하는지 (1줄 시나리오)
- 측정값 (실측 latency / 빌드 시간 / 번들 크기 등)
- 다음 페이즈로 가기 전 결정해야 할 사항
- (필요 시) 새 ADR 제안

---

## 초격차 강화 Thesis & 로드맵 갭 (2026-04-27)

> 글로벌 경쟁 리서치 종합 — 자세한 분석은 `docs/PRODUCT.md` §4.2 / §6 / 부록 A·C 참조.

### 7-점 강화 Thesis (Phase 2'~6'에 분배)

| # | Thesis | 주 페이즈 | 부 페이즈 |
|---|---|---|---|
| 1 | Korean-substrate (모델 1순위 + 한글 RAG + Korean QA evals) | 2', 4.5', 5' | 4 |
| 2 | Wrap-not-replace 게이트웨이 + per-webapp scoped key | 3' | 6' |
| 3 | Portable workspace fingerprint repair | 3' | — |
| 4 | 매니페스트 installer + 2-tier + hardware-aware | 2' | 1A.3 (구현 완료) |
| 5 | 워크벤치 = quantize+LoRA+Korean validation+GGUF→Ollama 1-click | 5' | — |
| 6 | 자가스캔 deterministic + opt-in 로컬 LLM 요약 | 6' | 1' 잔여 (scanner crate) |
| 7 | Pipelines 패턴 (gateway-side, NOT UI-side) | 6' | 3' |

### 8 로드맵 갭 (G1~G8)

| # | 갭 | 노출 경쟁자 | 반영 페이즈 | 신규 ADR 제안 |
|---|---|---|---|---|
| G1 | Knowledge-Stack zero-config RAG | Msty / GPT4All LocalDocs / Page Assist / AnythingLLM | **Phase 4.5' 신설** | ADR-0023 (RAG 아키텍처) |
| G2 | Korean preset 100+ 번들 | Cherry Studio 300+ / Jan / LM Studio Hub | Phase 4 | — (Phase 4 PRD 항목) |
| G3 | Agent/Skill manifest (app/model manifest 동형) | AnythingLLM Agent Skill Store / Open WebUI Tools / Ollama Agent Store | Phase 6' | ADR-0017 addendum |
| G4 | 하드웨어 벤치마크 (probe만 아님, 실측 token/sec) | LM Studio Hardware tab | Phase 2' | ADR-0014 확장 |
| G5 | 2-tier (Verified/Community) 카탈로그 거버넌스 | Pinokio / LM Studio Hub | Phase 2' | ADR-0014 확장 |
| G6 | per-app data segregation | AnythingLLM workspace / Msty workspace | Phase 3' | ADR-0009 확장 |
| G7 | MCP host capability | Jan 0.7+ / Page Assist beta / AI Toolkit | Phase 6' | 신규 ADR (MCP boundary) |
| G8 | multi-provider routing 정책 (per-webapp scope) | Cherry / Jan / Open WebUI 부분 | Phase 3' | **ADR-0022 (게이트웨이 라우팅 정책)** |

### 신규 ADR 후보 (Phase 진입 시 작성)

- **ADR-0022** — Gateway routing policy (Phase 3' 진입 시) — local-first, cloud-fallback, per-webapp scope, observability hooks. ✅ 확정.
- **ADR-0023** — Workbench v1 boundary policy (Phase 5'.a) — LoRA / Modelfile / 양자화 wrapper 정책. ✅ 확정 (2026-04-28).
- **ADR-0024** — Knowledge Stack RAG (Phase 4.5'.a) — 한글 NFC, chunker, per-workspace SQLite, MockEmbedder 384-dim. ✅ 확정 (2026-04-28).
- **ADR-0025** — Pipelines surface at gateway (Phase 6'.a) — gateway-side filter modules, Pipeline trait, 외부 통신 0. ✅ 확정 (2026-04-28).
- **ADR-0026** — Auto-Updater source (Phase 6'.a) — GitHub Releases 1순위 + 6h poll + 사용자 동의 후 다운로드. ✅ 확정 (2026-04-28).

### 페이즈별 위험 (Top-3, 2026-04-27)

1. **Ollama new app GUI 추격** — drag-drop 파일·multi-session 채팅을 이미 보유. Phase 4 v1 데모를 채팅 polish가 아닌 워크벤치 + 포터블 + Korean 큐레이션에 anchoring.
2. **LM Studio Hub Korean 카테고리 진입** — Phase 4 종료 전에 Korean preset 100+ + Phase 5' 워크벤치 QA evals를 출시해 선점.
3. **Foundry Local의 hardware-aware ONNX 카탈로그** — 우리는 Foundry와 달리 ONNX-only가 아니라 Ollama+LM Studio+llama.cpp 다중 런타임 hardware filter로 차별화. Phase 2' 설계 시 명시.

### 키 컴파스 (모든 페이즈에서 자문)

> "Ollama / LM Studio가 다음 분기에 같은 기능을 출시하면 우리 USP가 살아남는가?"

- 살아남으면 → 진행.
- 안 살아남으면 → Korean / Workbench / Portable / Gateway 4축 중 어디로 더 깊이 들어가야 하는지 재검토.

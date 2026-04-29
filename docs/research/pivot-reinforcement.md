# Pivot 보강 리서치 — 종합 (4영역)

> 2026-04-26. v1 포지셔닝 재정의를 위한 4영역 병렬 리서치 결과.
> 출처/세부는 각 영역 끝 링크 모음 참조.

## 1. Wrapper/Orchestrator 앱 landscape (2026)

| 앱 | 백엔드 | 라이선스 | 한국어 | LMmaster에 주는 시사 |
|---|---|---|---|---|
| **Pinokio** | 자체 venv spawn (모든 앱 별도) | MIT | 없음 | **설치 매니페스트 패턴**(JSON + install/start/update). 디스크 폭증·zero portability 안티패턴 회피. |
| **Open WebUI** | Ollama native + OpenAI-compat URL (LM Studio attach 가능) | BSD-3 + branding clause | ✅ ko-KR translation 존재 | 웹 우선 배포(Docker/pip)는 한국 데스크톱 사용자에 마찰. **i18n 키 베이스 참고**. |
| **Jan.ai** | 자체 cortex.cpp + remote | Apache-2.0 / AGPL 혼합 | 부분 | **"compete don't wrap"** 철학 — 우리와 정반대. 회피 대상. |
| **Msty** | Ollama / llama.cpp / MLX / LM Studio | proprietary freemium | 부분 | **freemium 게이팅 패턴 회피**. UX는 참고. |
| **Cherry Studio** | Ollama+LM Studio HTTP attach 자동 + 300+ 클라우드 프리셋 | AGPL-3.0 | 부분 | **가장 가까운 패턴** — HTTP attach 자동 감지 + 카테고리 프리셋. 코드 fork 금지(AGPL), **패턴만** 학습. |
| **AnythingLLM** | 30+ providers (Ollama, LM Studio attach, 내장 llama.cpp) | MIT | 부분 | RAG-heavy. 우리 워크스페이스 메타포와 충돌 회피. |
| **Foundry Local (MS)** | 자체 ONNX-Runtime | MIT (GA 2026-04) | OS만 | **hardware-aware curated catalog** ("RTX 3060/M2/NPU에서 검증" 뱃지). 큐레이션 bar. |

**채택 패턴 Top 3:**
1. **Cherry Studio**의 HTTP-attach 자동 감지 (`:11434` Ollama, `:1234` LM Studio) + assistant preset 라이브러리.
2. **Foundry Local**의 hardware-aware 모델 카드 (per-model "tested on" 배지).
3. **Pinokio**의 declarative manifest + Verified/Community 2-tier 레지스트리.

**회피:**
- Pinokio per-app venv → 디스크 폭증.
- Jan compete-don't-wrap.
- Cherry/Open WebUI AGPL/branding 코드를 그대로 fork하지 않는다 (패턴만).
- 300+ 모델 한 번에 노출(Cherry) → 점진 공개(progressive disclosure).

---

## 2. 자동 설치 + 자동 업데이트 패턴

### 2.1 Pinokio installer 구조

- **포맷**: JSON 또는 `.js`가 JSON 반환. 각 앱이 `install.js`, `start.js`, `update.js`, `pinokio.js`(메뉴) 보유.
- **핵심 구문**: `run` 배열 = `{method, params}` ops. 메소드: `shell.run` (conda/venv 컨텍스트), `fs.download`, `fs.link`, `script.start`, `input` (env-var 프롬프트), `notify`.
- **템플릿**: `{{platform}}`, `{{gpu}}`, `{{args.x}}` Mustache. 동일 스크립트가 Win/mac/Linux × CUDA/ROCm/MPS/CPU 분기.
- **파일 시스템**: `~/pinokio/{api,bin,cache,drive,logs}`. `bin/`은 공유 toolchain (conda, ffmpeg, cuda) → 앱 간 재다운로드 회피. `drive/`는 모델 dir 심볼릭 링크 dedupe.
- **업데이트**: `update.js` = `git pull` + 의존성 재설치(트랜잭션). semver 없음.

**LMmaster 채택**: 우리 `manifests/apps/{ollama,lm-studio}.json`을 Pinokio-style declarative로 작성. `install`, `detect`, `update` 액션 + 공유 cache dir.

### 2.2 Tauri 2 외부 installer 호출

- `tauri-plugin-shell`의 `Command::new`로 설치된 바이너리 spawn.
- 권한은 `src-tauri/capabilities/*.json`에 명시: `shell:allow-execute` + `cmd` argument validator. 와일드카드 exec는 default-deny.
- **패턴**: `tauri-plugin-http`로 installer를 `app_cache_dir`에 다운로드 → SHA256 + (가능하면) 코드 사인 검증 → silent flag로 spawn:
  - LM Studio NSIS: `LM-Studio-Installer.exe /S`
  - Ollama Win: `OllamaSetup.exe /SILENT`
  - MSI: `msiexec /i ... /passive /norestart`
  - macOS: `installer -pkg ...`
- **코드 사인/SmartScreen**: 우리 앱은 사인(EV cert 권장). 다른 vendor 사인 installer는 그대로 호출 — UAC는 그쪽 cert로 뜸. 절대 재사인하지 말 것.
- **No "run-as-admin"** in Tauri 2 (issue #7173). Per-machine installer는 그쪽 manifest에 위임, 또는 per-user 설치 우선.

### 2.3 Tauri 2 updater 플러그인

- `tauri-plugin-updater` v2: 정적 JSON (GitHub Releases / S3) + OS-ARCH 키 + signature.
- **사인 mandatory**. `tauri signer generate`. 비활성 옵션 없음.
- Windows installModes: `passive` (default) / `basicUi` / `quiet`. 설치 시 앱 종료 필요 → "재시작 안내" UX.
- **delta update 미지원** in v2. 풀 번들 교체. 한국 사용자는 KR-가까운 CDN 권장.

### 2.4 외부 앱 버전 detect (cross-platform)

| 앱 | Win | macOS | Linux |
|---|---|---|---|
| LM Studio | `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\<GUID>\DisplayVersion` 또는 `lms.exe version` | `lms version`, `Info.plist CFBundleShortVersionString` | `lms version`, AppImage 파일명 |
| Ollama | `GET http://127.0.0.1:11434/api/version` (모든 OS, daemon 동작 시) → fallback `ollama --version` | 동일 + `Info.plist` | 동일 + `dpkg -s ollama` |

**원칙**: HTTP/CLI probe 우선, 레지스트리/plist는 fallback.

### 2.5 다중 앱 업데이트 조율

- **소스 (6~24h 폴링)**:
  - LM Studio: `https://lmstudio.ai/api/latest-version` + changelog. GitHub release 없음(데스크톱 앱). `lmstudio-ai/lms` (CLI)는 GitHub.
  - Ollama: `api.github.com/repos/ollama/ollama/releases/latest`.
- **트리거**: 두 앱 모두 자체 updater 보유. 최선 UX = "앱을 열어 자체 업데이트를 받게" 또는 silent re-install. 절대 사용자 동의 없이 자동 적용 금지.
- **모델 카탈로그**: Ollama `/api/tags` mirror + 우리 `models.json`의 `digest`(sha256) 비교.

---

## 3. LM Studio + Ollama orchestrability (검증된 사실)

### 3.1 LM Studio

- **`lms` CLI** (lmstudio.ai/docs/cli): `ls`, `get`, `load`, `unload`, `ps`, `server start|stop|status`, `daemon up|down|status|update` (= llmster), `log stream`, `import`, `runtime`.
- **REST API** (`http://localhost:1234`):
  - 자체: `GET/POST /api/v1/models`, `/models/load`, `/models/unload`, `/models/download`, `/models/download/status`, `/api/v1/chat`.
  - OpenAI-compat: `/v1/chat/completions`, `/v1/embeddings`, `/v1/models`, `/v1/responses`.
  - Anthropic-compat: `/v1/messages`.
  - **JIT 로딩** 토글: 활성 시 `/v1/models`가 모든 다운로드된 모델 반환 + 추론 호출 시 자동 로드. **idle TTL auto-evict** 토글 별도.
  - Bearer auth.
- **파일시스템**: `~/.lmstudio/models/<publisher>/<model>/*.gguf` (설정 가능). 읽기-열거 안전, 쓰기 금지.
- **헤드리스**: `lms daemon up` (= llmster). `:1234` listen.
- **라이선스 (lmstudio.ai/app-terms, free-for-work since 2025-07)**:
  - 개인/상업/사내 사용 자유.
  - **금지**: installer 재배포, sublicense, SaaS resale, 공식 인터페이스 외 통합.
  - **LMmaster 처리**: 사용자가 직접 설치하도록 **공식 사이트로 안내**. **번들/silent 자동 설치 금지**. `lms` CLI + REST API로만 통합.

### 3.2 Ollama

- **HTTP API** (`:11434`, github.com/ollama/ollama/blob/main/docs/api.md): `/api/{generate,chat,create,tags,show,copy,delete,pull,push,embed,ps,version,blobs/:digest}`. 완전 통제 가능.
- **Modelfile**: `FROM/PARAMETER/TEMPLATE/SYSTEM/ADAPTER/LICENSE/MESSAGE`. `POST /api/create`로 JSON 인라인 가능 (0.5+).
- **파일시스템**: `~/.ollama/models/{blobs,manifests}` (또는 `OLLAMA_MODELS`).
- **데몬 detect**: `/api/version` probe → 응답 시 attach. 실패 시 `ollama serve` 우리가 spawn. 이중 spawn 금지(포트 충돌).
- **라이선스 MIT**. 제약 없음. **silent install 가능**.

### 3.3 Orchestration 함정

- **포트 충돌**: 기본은 안 부딪힘(1234 vs 11434). 멀티 인스턴스 시 `lms server start --port N` / `OLLAMA_HOST=127.0.0.1:N ollama serve`. 항상 probe 우선, 기본값 가정 금지.
- **GPU contention**: 동시 로드 → OOM. **직렬화** — 한 백엔드 active, 다른 쪽 evict (`lms unload --all` / Ollama `keep_alive: 0`). 사용자에게 `keep_alive` / idle TTL 노출.
- **저장소 중복**: 같은 GGUF가 양쪽 path에 → SHA256으로 detect + 경고. **자동 symlink 금지** (Ollama blob-store invariant 깨짐).
- **업데이트 racing**: `lms` 바이너리가 mid-session에 교체될 수 있음. retry-with-detect 패턴.

---

## 4. SLM 양자화 + 도메인 파인튜닝 UX

### 4.1 파인튜닝 OSS

| 도구 | UI | 모델 커버리지 | 메소드 | 라이선스 | 플랫폼 | 비고 |
|---|---|---|---|---|---|---|
| **Unsloth** | 없음(Colab notebooks) | Llama 3.x, Qwen 2/2.5/3, Gemma, Mistral, Phi | LoRA, QLoRA, DPO, GRPO | Apache-2.0 | Linux+WSL/Win 부분, **Mac 없음** | 1.6~2x 빠름, VRAM 70% 절감 |
| **LLaMA-Factory** | **Gradio "LLaMA Board"** ★ | 100+ | Full SFT, LoRA, QLoRA, DPO, ORPO, KTO, PPO, RM | Apache-2.0 | Linux/WSL/Win, Mac CPU-only | **ko 로케일 이미 존재** (locales/) — v1 채택 |
| Axolotl | YAML CLI | Llama, Qwen, Gemma, Mistral, Mixtral | Full, LoRA, QLoRA, DPO, ORPO, multipack | Apache-2.0 | Linux | 고급 사용자용 |
| **MLX-LM** | CLI | Llama, Qwen, Gemma, Mistral, Phi | LoRA, QLoRA, DoRA, full | MIT | **Apple Silicon only** | mac path 채택 |
| NVIDIA AI Workbench | 데스크톱 GUI | Bring-your-own | 프로젝트 의존 | closed | Win/Mac/Linux + Docker | NVIDIA 락인 |
| HF AutoTrain | CLI + Spaces UI | Llama, Mistral, Mixtral 등 | LoRA SFT, DPO, ORPO, full | Apache-2.0 | Linux | 로컬보단 클라우드 |

### 4.2 양자화 포맷

| 포맷 | 런타임 | 사용자 가치 | 라이선스 |
|---|---|---|---|
| **GGUF** | llama.cpp / Ollama / LM Studio / Jan / koboldcpp | 단일 파일 portable, CPU/CUDA/Metal/Vulkan/ROCm 모두 | MIT |
| AWQ | vLLM / TGI / Transformers / MLC | 4-bit 정확도 ↑ (GPU-first) | MIT |
| GPTQ / GPTQModel | vLLM / ExLlamaV2 / Transformers | 4-bit GPU 추론 | Apache-2.0 |
| **MLX** | mlx_lm | Apple Silicon native | MIT |
| ExecuTorch | iOS/Android | mobile | BSD-3 |
| ONNX Runtime GenAI | Win/Linux/Mac/mobile | cross-platform | MIT |

### 4.3 GGUF quant 가이드
- **Q4_K_M**: 기본 sweet spot (~4.8 bpw, perplexity loss <1%).
- Q5_K_M: 12GB GPU에서 7B 품질.
- Q8_0: 거의 무손실, 2× 크기.
- Q3_K_S/IQ3_XXS: 8GB squeeze.

### 4.4 RTX 4070 Super (12GB) QLoRA 예산
- 3B (Llama-3.2-3B, Qwen2.5-3B): batch 2, seq 2048, 6~8GB, **15~30분 / epoch / 5k 샘플**.
- 7B (Llama-3.1-8B, Qwen2.5-7B): batch 1 + grad-accum, seq 2048, 10~11GB, **1.5~3시간**.
- 13B QLoRA: 11.5GB tight, Unsloth 필수.

### 4.5 LMmaster v1 워크벤치 권장 구성

**1차 통합**:
- **양자화: llama-quantize** (llama.cpp 자식 프로세스, Python 불필요) + `convert_hf_to_gguf.py`. 우리 Rust supervisor가 spawn.
- **파인튜닝: LLaMA-Factory CLI** (Python sidecar). Apache-2.0 + ko 로케일 + 가장 넓은 모델 커버리지. `llamafactory-cli train config.yaml`로 우리가 driver. **Unsloth는 옵션 가속기**(Win/Linux/NVIDIA, `--use_unsloth`).
- **mac path**: MLX-LM (`mlx_lm.lora` + `mlx_lm.convert -q`).

**MVP 5-화면 플로우**:
1. **베이스 모델 선택** — 큐레이션 (Llama-3.2-3B, Qwen2.5-3B/7B, Gemma-2-2B). HF 자동 다운로드.
2. **데이터 드롭** — JSONL/CSV/마크다운 폴더 → chat JSONL 자동 변환 + 토큰 수·예상 시간 표시.
3. **프리셋** — 빠름(QLoRA 3B 1ep) / 균형 / 품질. 고급은 접힘.
4. **학습** — live loss 차트, VRAM 게이지, N steps마다 샘플 미리보기, 1-click 취소.
5. **내보내기** — LoRA merge, GGUF Q4_K_M 양자화, **Ollama/LM Studio 한 번에 등록**.

**기존 클릭-투-파인튜닝 데스크톱 앱**:
- Kiln (`Kiln-AI/Kiln`) — MIT, Unsloth/Together/Fireworks 호출.
- H2O LLM Studio — Apache-2.0, Wave-based, Linux 우선.
- 그 외엔 NVIDIA AI Workbench(Docker-heavy + NVIDIA-only).
- **결론**: 한국어 + 로컬 + 풀 cross-platform 데스크톱 앱은 빈자리.

---

## 출처 모음 (검증됨)

- pinokiocomputer/pinokio · docs.pinokio.computer · pinokio data structure · pinokiofactory recipes
- open-webui/open-webui · openwebui.com docs · open-webui ko-KR translation
- menloresearch/jan · jan.ai/docs
- msty.ai · msty.ai/changelog
- CherryHQ/cherry-studio · cherry-ai.com docs (Ollama integration)
- Mintplex-Labs/anything-llm
- microsoft/Foundry-Local · devblogs.microsoft.com/foundry/foundry-local-ga
- v2.tauri.app/plugin/{shell,updater,http} · v2.tauri.app/distribute/sign/windows
- tauri-apps/tauri issue #7173 (admin)
- lmstudio.ai/docs/{cli,app/api/endpoints/rest,developer/core/headless,app-terms,blog/free-for-work}
- lmstudio-ai/lmstudio-bug-tracker issue #1255 (registry version)
- github.com/ollama/ollama/blob/main/{docs/api.md,LICENSE} · docs.ollama.com
- unslothai/unsloth · hiyouga/LLaMA-Factory · axolotl-ai-cloud/axolotl
- ml-explore/mlx-lm · pytorch/executorch · microsoft/onnxruntime-genai
- Kiln-AI/Kiln · h2oai/h2o-llmstudio · oumi-ai/oumi

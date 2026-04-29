# Phase 1' 보강 리서치 — 종합

> 2026-04-26. 4영역 병렬 리서치 결과 종합. v1 정체성에 추가된 (a) zero-knowledge 부팅 (b) 자가스캔+자동 업그레이드 두 요구를 반영해 Phase 1' 설계를 확정한다.
> 출처/세부는 각 영역 끝 링크 모음.

## 0. 종합 결정 (적용 결과)

| 영역 | 결정 | 근거 ADR / 출처 |
|---|---|---|
| 첫 실행 부팅 패턴 | **Hybrid (Pattern C)** — 빌드 시 bundled snapshot + 첫 실행 시 async 원격 fetch + cache 폭포 (cache≤TTL → remote → cache stale → bundled) | VS Code, GitHub Desktop, Cursor, JetBrains Toolbox 패턴. Korean 기업망(KT/SKB 간헐, 사내 proxy) 대비 필수. ADR-0019. |
| 멀티-소스 버전 lookup | 4-tier 병렬 폴백: vendor API(2s) ‖ GitHub releases(3s) → jsdelivr CDN(3s) → bundled snapshot. 2s soft deadline | ADR-0019. |
| GitHub Releases 폴링 | anonymous + `ETag`/`If-None-Match`. 60/h limit는 6h 폴링이면 충분. 304는 cap에 안 들어감 | ADR-0019. |
| 폴링 cadence | **on-launch + 6h interval**. JetBrains Toolbox-style "background, low-noise" | ADR-0019. |
| 자동 업그레이드 UX | JetBrains Toolbox 모델 — auto-check (toggle), background download, "다음 실행 때 적용" 토스트 + "지금 재시작" 보조 | ADR-0019. Toss agency-first 원칙과 일치. |
| Trending 발견 소스 | HF `/api/models?sort=trending_score` (anon 500/5min) + OpenRouter `/api/v1/models` + Ollama library scrape. 1h cache + ETag | ADR-0019. |
| 자가스캔 모델 | **EXAONE-3.5-2.4B-Instruct** (LG, ko native) Q4_K_M ~1.5GB 1순위; HyperCLOVA-X-SEED-3B 2순위; Qwen2.5-3B-Instruct fallback | ADR-0020. |
| 로컬 LLM 사용 범위 | **요약(블러브)만**. version 분류·카탈로그 fit 판단은 deterministic. ADR-0013 Gemini boundary와 동일 논리 | ADR-0020. |
| 모델 라이프사이클 | Ollama `keep_alive: "30s"`. JIT 로드 + 30초 idle eviction. CPU-only는 백그라운드만 허용 | ADR-0020. |
| 백그라운드 스케줄러 | `tokio-cron-scheduler` 인-앱. 데몬화는 v2 옵션 (`tauri-plugin-autostart`) | ADR-0020. |
| 모델 미설치 fallback | deterministic 한국어 템플릿. LLM 호출 0회. 모든 핵심 기능 동작 | ADR-0020. |
| 외부 앱 detect | HTTP probe 우선 (Ollama `/api/version`, LM Studio `/v1/models`) → 레지스트리/plist fallback. SHA-256 검증된 download → atomic rename → execute | ADR-0017 (기 작성). |
| Tauri shell ACL | `command` 필드 사용(v2), 인자 정규식 validator 강제. 와일드카드 0 | ADR-0017 (기 작성). |
| LM Studio EULA | installer 재배포 금지, **published interfaces**(lms CLI, REST)로만 통합 가능, free-for-work 2025-07-08부 | ADR-0017 verified. |
| Ollama Win 설치 | `OllamaSetup.exe /SILENT` (Inno Setup), per-user만 (`%LOCALAPPDATA%\Programs\Ollama`) | 검증됨. |
| WebView2 detect | `HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}\pv` | 검증됨. |
| VC++ 2022 detect | `HKLM\SOFTWARE\Wow6432Node\Microsoft\VisualStudio\14.0\VC\Runtimes\x64\Installed=1` | 검증됨. |
| 한국어 톤 | **해요체** consumer-facing 전면. 합쇼체는 license/legal 화면만 | ADR-0010 보강 (이 리포트 §4 참조). |
| 첫실행 우선 질의 | 언어 선택 + 첫 모델 프리셋(small/medium/skip) **단 2가지만**. 저장 경로·GPU 모드·텔레메트리 등 모두 default + 변경 가능 안내 | Last Responsible Moment 원칙. ADR-0019. |
| 첫 모델 자동 큐 | 1~2GB Korean-tuned (예: EXAONE-2.4B Q4) **명시 동의 화면** 포함. "건너뛰기" 옵션 제공 | ADR-0019. |
| 진행률 UX | **stepper + step-name + size/ETA + 접힘식 "자세히 보기"** (Pinokio scroll log 안티패턴 회피) | 검증됨. |
| 다운로드 손상 복구 | `.partial` + sha256 + atomic rename + Squirrel "blue/green" 보전. 1회 silent retry → 한국어 actionable error | 검증됨. |

ADR-0019(Always-latest hybrid)와 ADR-0020(자가스캔+로컬-LLM augmentation) 신설.

---

## 1. 외부 앱·환경 detect/install (요약)

### 1.1 LM Studio EULA 정확 인용 (lmstudio.ai/app-terms, 2025-07-01)
- 금지: "sublicense, distribute, sell, use for service bureau use, as an application service provider, or a software-as-a-service, lease, rent, loan, or otherwise transfer the Software."
- 허용 면허: "non-exclusive, non-transferable, license to use the Software solely for Your personal and / or internal business purposes."
- 통합 제한: "integrate the Software with other software other than through Element Labs published interfaces made available with the Software."
- Free-for-work blog (2025-07-08): "LM Studio is free to use both at home and at work."

→ LMmaster **CAN**: lms CLI + REST(`http://localhost:1234`)로 제어, internal business 사용. **CANNOT**: installer 재배포, SaaS 형태로 노출, 비공식 internal API 호출.

### 1.2 Ollama silent install (검증)
- Inno Setup. `/SILENT` 또는 `/VERYSILENT`, `/SUPPRESSMSGBOXES`, `/NORESTART`, `/DIR=`. **per-user만** (`%LOCALAPPDATA%\Programs\Ollama`). MSI `/quiet` NOT 지원. 이슈 ollama#7969.

### 1.3 환경 prereq detect (Windows)
- NVIDIA 드라이버: `nvidia-smi --query-gpu=driver_version --format=csv,noheader` 또는 디스플레이 클래스 레지스트리.
- CUDA toolkit: `nvcc --version` 또는 `HKLM\SOFTWARE\NVIDIA Corporation\GPU Computing Toolkit\CUDA\v<X.Y>` 키 존재.
- WebView2 evergreen: `HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}` 의 `pv`.
- VC++ 2022 redist x64: `HKLM\SOFTWARE\Wow6432Node\Microsoft\VisualStudio\14.0\VC\Runtimes\x64\Installed=1` + `Version`.

### 1.4 macOS / Linux
- mac: `uname -m`(arm64/x86_64), Rosetta `sysctl -n sysctl.proc_translated`, Metal `system_profiler SPDisplaysDataType | grep "Metal Support"`.
- Linux: glibc≥2.27 (`ldd --version`), libstdc++ GLIBCXX_3.4.25, `libcuda.so.1` resolvable, NVIDIA compute_cap≥5.0.

### 1.5 tauri-plugin-shell strict ACL (v2 schema)
- `tauri-plugin-shell/src/scope.rs` 인용: `ScopeAllowedCommand { name, command, args, sidecar }`. v2의 정식 필드는 `command`(v1 `cmd`이 일부 docs에 잔존). 인자 validator regex는 전체 일치.
- 예: `OllamaSetup.exe /SILENT` 한 가지 + `lms version|server (start|stop|status)|ls --(json|llm|embedding)` 정규식 제한.

### 1.6 Mid-flight 실패 복구
- 다운로드 → `*.partial` + sha256 검증 → atomic rename → execute.
- 비-zero exit 시 한 번 silent retry → 두 번째 실패엔 Inno setup log(`%TEMP%\Setup Log *.txt`) 노출 + 한국어 actionable.
- Squirrel "blue/green": 새 버전 검증 전엔 이전 버전 절대 삭제 금지.

---

## 2. Always-latest 부팅 + 자동 업그레이드

### 2.1 Hybrid bootstrap (Pattern C) 폴백 cascade
```
firstRun:
  cache <= TTL?  → use cache, async refresh in background
  remote 4-tier? → P1(vendor API, 2s) || P2(GitHub releases, 3s) → write cache + ETag → use
  cache (stale)?  → use stale, surface "확인 중…" badge
  bundled snapshot → always succeeds, surface "오프라인 표시 중" badge
```

### 2.2 GitHub anonymous polling + ETag
- `GET .../releases/latest` + `If-None-Match: <stored-etag>` → 304 free. 6h cadence × per-IP 60/h = 4 hits/day. 충분.
- 403 rate-limit 시 jsdelivr `cdn.jsdelivr.net/gh/{owner}/{repo}@latest/...` 폴백.

### 2.3 자동 업그레이드 UX (JetBrains Toolbox 모델)
- **(a) 동의**: 자동 체크(default on, 설정에서 off 가능).
- **(b) 다운로드**: 백그라운드.
- **(c) 적용**: "업데이트 준비됨 · 다음 실행 때 적용돼요" 토스트.
- **(d) 보조**: "지금 재시작" 버튼.
- 한국어 카피: "새 버전 LM Studio 0.4.x 준비됨 · [지금 적용] [나중에]".

### 2.4 Trending 모델 소스 결정
- **HuggingFace `GET /api/models?sort=trending_score&limit=N`** — anonymous 500/5min, fields: `author, downloads, likes, trendingScore, pipeline_tag, tags, gguf, gated`. 카테고리 필터 (on-device/voice/coding) 가능. Apache-2.0 client SDK.
- **OpenRouter `GET /api/v1/models`** — `id, pricing, context_length, supported_parameters, output_modalities, category`. 개발자 인기 신호.
- **Ollama library scrape** — public JSON API 없음. `ollama.com/library/<name>/tags` 또는 GitHub registry mirror 폴백.
- 1h cache + ETag.

### 2.5 tauri-plugin-updater 보강
- ETag/`If-Modified-Since` natively NOT 지원 → `setRequestHeaders`로 수동 추가.
- 다중 endpoint failover 지원 (timeout 시 자동 폴백).
- Ed25519 sign verification, `tauri signer generate` 키페어.

---

## 3. 자가스캔 + 로컬 LLM augmentation

### 3.1 모델 비교 (한국어 fluency 우선)

| 모델 | 파라미터 | Q4 GGUF | ko fluency | 라이선스 | LMmaster 적합도 |
|---|---|---|---|---|---|
| **EXAONE-3.5-2.4B-Instruct** | 2.4B | ~1.5GB | **Native (LG ko 학습)** | EXAONE AI License (research+제한적 commercial) | **1순위** |
| **HyperCLOVA-X-SEED-3B-Instruct** | 3.0B | ~1.9GB | **Native (Naver ko-first 코퍼스)** | HCX-SEED (commercial w/ conditions) | 2순위 |
| Qwen2.5-3B-Instruct | 3.1B | ~1.9GB | Good (multilingual) | Qwen Research License | fallback |
| Gemma-2-2B-it | 2.6B | ~1.6GB | Mediocre ko | Gemma Terms | 비추 |
| Llama-3.2-3B-Instruct | 3.2B | ~2.0GB | OK | Llama 3.2 Community | fallback |

### 3.2 latency (200토큰 출력)

| 하드웨어 | cold load | tok/s | 200tok latency |
|---|---|---|---|
| RTX 3060 6GB | 2~4s | 60~90 | ~2.5~3.5s |
| RTX 4070 12GB | 1~2s | 110~160 | ~1.3~1.8s |
| Apple M2 8GB Metal | 2~3s | 35~55 | ~4~6s |
| Ryzen 7 / i7 CPU | 3~5s | 8~15 | ~14~25s |

CPU-only는 백그라운드 OK, interactive UX는 NG.

### 3.3 JIT load + idle 정책
- **Ollama**: `keep_alive: "30s"` per-request. default `5m`, 우리는 30s로.
- **LM Studio**: `lms load <model> --ttl 30` 또는 REST `ttl` field.
- **llama-server**: idle timeout 미지원 → spawn-per-scan + kill 패턴.
- **권장**: Ollama HTTP + `keep_alive: "30s"`가 가장 단순·idiomatic.

### 3.4 LLM 사용 범위 결정 표

| Use Case | 결정 | 근거 |
|---|---|---|
| A. 모델 카드 한국어 1줄 블러브 | **LLM 사용** | 짧은 생성, JSON 입력, 환각 위험 낮음. EXAONE/HCX-SEED reliable. |
| B. version major/minor/patch 분류 | **deterministic semver regex** | 3B는 "0.3.10"을 patch로 오판할 수 있음. 도박 금지. |
| C. trending HF 모델이 카탈로그 fit인가 | **deterministic 룰 + human review queue** | 라이선스/사이즈/editorial 판단 — LLM unreliable. |

### 3.5 아키텍처
- **schedule**: `tokio-cron-scheduler` 인-프로세스. on-launch (60s grace) + 6h cron + UI on-demand.
- **모델 선택 cascade**:
  1. 사용자 Ollama에 EXAONE/HCX-SEED/Qwen2.5-3B 존재 → 사용.
  2. LM Studio에 호환 모델 → 사용.
  3. 모두 없음 → deterministic only. 자동 다운로드 강제 금지 (사용자 동의 후 첫 모델 큐).
- **호출 경로**: Rust core → reqwest → `http://localhost:11434/api/generate` (Ollama) `keep_alive:30s` `stream:true`.
- **이벤트 emit**: `app.emit("scan:summary", { model_id, korean_blurb, confidence, source: "local-llm"|"deterministic" })`.

### 3.6 프라이버시 마케팅 ("on-device 우선" precedent)
- Apple Intelligence (~3B foundation, 2024-06)
- Microsoft Copilot+ Phi Silica (NPU on-device)
- Pixel 9 Gemini Nano
- Obsidian 커뮤니티 플러그인 — Ollama 로컬 노트 요약

LMmaster 카피: **"AI 모델 카탈로그가 LMmaster를 떠나지 않아요. 추천을 만드는 AI도 당신의 컴퓨터에서 동작합니다."**

---

## 4. Zero-knowledge 한국어 onboarding

### 4.1 어떤 패턴을 채택하는가
- **Cursor (auto-detect over ask)** + **Postman (single-question personalization)** + **GitHub Desktop (sign-in deferred)** 교집합.
- Korean voice는 **Toss 8 writing principles** 따름 — 해요체, 한 줄 한 정보, 능동태, 제안 vs 강요.

### 4.2 첫실행 마법사 (4 단계, 60초 이내)
1. **언어 확인** ("ko-KR로 진행할게요. 변경하려면 [영어]") — 1 클릭 가능.
2. **환경 점검** (auto-detect, 1~3초): GPU / RAM / 디스크 / WebView2 / VC++ — "준비하고 있어요"만 표시. 실패 항목은 한국어 풀어서 + 자동 fix 옵션.
3. **첫 모델 프리셋**: small (~1.5GB EXAONE-2.4B) / medium (~4GB Qwen2.5-7B Q4) / 건너뛰기. **단 1회 명시 동의**.
4. **준비 완료** — 채팅 가능 카드 + "환경 둘러보기" 보조 링크.

### 4.3 한국어 voice 표준
- 해요체 일관 사용 ("준비하고 있어요", "곧 받을게요", "다시 시도할게요").
- 합쇼체는 license/legal 화면만 ("동의합니다").
- 에러 메시지 = 공감 + 다음 액션 ("인터넷 연결을 확인해 주세요. 다시 시도할게요").
- 한자 기술 jargon 회피.

### 4.4 진행률 UX
- stepper + 단계명 + 크기/ETA + 접힘식 "자세히 보기".
- 예: "모델을 받고 있어요 · 230 MB / 1.2 GB · 약 2분 · [자세히 보기 ▾]".
- Pinokio scroll log는 안티패턴.

### 4.5 네트워크 실패 / 디스크 / 권한
- 네트워크: Steam-style chunked + sha256 verified resume. 다음 실행 시 ".partial 발견 → 이어서 받을까요?" 묻기.
- 디스크 부족: 차단 + "다른 위치에 저장하기" 제안 (default 변경).
- UAC 거부: per-user 설치로 graceful degrade.
- 지수 backoff + jitter (5회 cap 후 한국어 error + 재시도 버튼).

### 4.6 deferred prompt 적용
- DON'T 묻는다: 모델 저장 경로, GPU vs CPU, 텔레메트리, 토크나이저 옵션.
- DO default + "설정 → 변경" affordance.
- 단 2개만 묻는다: 언어, 첫 모델 프리셋.

---

## 5. ADR 작성 결정

이 리서치 결과로 신설:
- **ADR-0019 — Always-latest hybrid bootstrap + 자동 업그레이드 정책**
- **ADR-0020 — 자가스캔 + 로컬 LLM augmentation (요약 only, 판단은 deterministic)**

기존 ADR에 영향:
- ADR-0010 (한국어 우선) — voice 표준에 "해요체 / 합쇼체 분리" 추가 (이 리포트 §4 본문 참조, 별도 ADR 없이 가이드 갱신).
- ADR-0013 (Gemini boundary) — 동일 원칙을 로컬 LLM에도 확장 (judgement 위임 금지).
- ADR-0014 (curated registry) — trending 소스 매트릭스 (HF/OpenRouter/Ollama scrape) 추가.

---

## 출처 모음 (검증)

- Pinokio: github.com/pinokiofactory/comfy, github.com/cocktailpeanut/fluxgym, docs.pinokio.computer
- Tauri: v2.tauri.app/plugin/{shell,updater}, /security/scope, docs.rs/tauri-plugin-shell
- LM Studio: lmstudio.ai/app-terms, lmstudio.ai/blog/free-for-work, lmstudio.ai/docs/{cli,local-server}, github.com/lmstudio-ai/lms
- Ollama: docs.ollama.com/{windows,api}, github.com/ollama/ollama (issues #7969 silent install, #3312 args, #15038 NSIS migration)
- WebView2/VC++/NVIDIA: learn.microsoft.com/microsoft-edge/webview2/concepts/distribution, learn.microsoft.com/cpp/windows/redistributing-visual-cpp-files
- macOS: developer.apple.com/forums/thread/652667 (Apple Silicon detect)
- 모델: huggingface.co/{LGAI-EXAONE/EXAONE-3.5-2.4B-Instruct, naver-hyperclovax/HyperCLOVAX-SEED-Text-Instruct-3B, Qwen/Qwen2.5-3B-Instruct}
- llama.cpp benchmark: github.com/ggerganov/llama.cpp/discussions/4167
- Apple Intelligence: machinelearning.apple.com/research/introducing-apple-foundation-models
- MS Copilot+: blogs.windows.com/windowsexperience/2024/05/20/introducing-copilot-pcs
- VS Code update: code.visualstudio.com/docs/{supporting/faq,enterprise/updates}
- GitHub REST: docs.github.com/rest/using-the-rest-api/{rate-limits,best-practices}
- HF API: huggingface.co/docs/hub/{rate-limits,api}
- OpenRouter: openrouter.ai/docs/api/{api-reference/models,reference/limits}
- Toss UX: toss.tech/article/{8-writing-principles-of-toss,introducing-toss-error-message-system}, developers-apps-in-toss.toss.im/design/ux-writing
- Squirrel.Windows: github.com/Squirrel/Squirrel.Windows/blob/develop/docs/using/install-process.md
- JetBrains Toolbox 2.0: blog.jetbrains.com/toolbox-app/2023/08/toolbox-app-2-0-overhauls-installations-and-updates
- AWS retry-with-backoff: docs.aws.amazon.com/prescriptive-guidance/latest/cloud-design-patterns/retry-backoff
- tokio-cron-scheduler: github.com/mvniekerk/tokio-cron-scheduler

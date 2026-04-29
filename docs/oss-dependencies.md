# 7. OSS Dependency Matrix

> 산출물 #7. 검토/채택 OSS의 라이선스, 성숙도, 통합 방식, 우선순위.
> "공식 문서 우선 / adapter로 붙일 수 있으면 adapter로 / fork보다 composition / 가짜 플래그 금지" 원칙을 따른다.

범례:
- **통합 방식**: `internal-dep`(우리 코드 내부 의존), `subprocess`(자식 프로세스로 띄움), `attach`(이미 떠있는 프로세스에 HTTP 붙음), `embed-py`(Python sidecar 안에서 import), `reference`(영감/참고만).
- **v1 채택**: ✅ 핵심 / 🟡 옵션·향후 / ⚪ 참고 only.

## 7.1 데스크톱 셸 / 프런트

| OSS | 라이선스 | 통합 방식 | v1 | 비고 |
|---|---|---|---|---|
| Tauri 2 | MIT/Apache-2.0 | internal-dep | ✅ | ADR-0002 |
| React | MIT | internal-dep | ✅ | |
| TypeScript | Apache-2.0 | internal-dep | ✅ | |
| Vite | MIT | internal-dep | ✅ | |
| i18next | MIT | internal-dep | ✅ | 한국어 키 베이스 |
| Zustand or Jotai | MIT | internal-dep | ✅ | 작은 상태 관리 |
| TanStack Query | MIT | internal-dep | ✅ | gateway/IPC 폴링 |
| Playwright | Apache-2.0 | dev-dep | ✅ | 시각 회귀 |

## 7.2 Native core (Rust)

| OSS | 라이선스 | 통합 방식 | v1 | 비고 |
|---|---|---|---|---|
| tokio | MIT | internal-dep | ✅ | async runtime |
| Axum | MIT | internal-dep | ✅ | ADR-0003 |
| tower / tower-http | MIT | internal-dep | ✅ | 미들웨어 |
| serde / serde_json | MIT/Apache-2.0 | internal-dep | ✅ | |
| sqlx 또는 rusqlite | MIT/Apache-2.0 | internal-dep | ✅ | ADR-0008 (선택은 스캐폴딩 시) |
| rusqlite + sqlcipher feature | MIT | internal-dep | ✅ | secrets DB |
| reqwest | MIT/Apache-2.0 | internal-dep | ✅ | 모델 다운로드 |
| sha2 | MIT/Apache-2.0 | internal-dep | ✅ | checksum |
| sysinfo | MIT | internal-dep | ✅ | hardware probe baseline |
| nvml-wrapper | MIT | internal-dep | ✅ | NVIDIA GPU |
| wgpu / wgpu-hal | MIT/Apache-2.0 | internal-dep | 🟡 | Vulkan/Metal/DirectML capability 탐지 보강 |
| libloading | MIT/Apache-2.0 | internal-dep | 🟡 | dynamic plugin v2 |
| tracing / tracing-subscriber | MIT | internal-dep | ✅ | 로깅 |
| keyring | Apache-2.0 / MIT | internal-dep | ✅ | OS keychain |
| refinery 또는 sqlx-migrate | MIT | internal-dep | ✅ | DB 마이그레이션 |

## 7.3 추론 런타임 (어댑터로 통합)

| OSS | 라이선스 | 통합 방식 | v1 | 비고 |
|---|---|---|---|---|
| **llama.cpp (server)** | MIT | subprocess | ✅ | ADR-0005 — primary portable |
| **KoboldCpp** | AGPLv3 | subprocess | ✅ | RP/캐릭터 카테고리 — 라이선스 영향 검토(별도 프로세스 호출은 GPL 결합 회피 통념, 법무 확인 필요) |
| **Ollama** | MIT | attach | ✅ | 외부 설치형 어댑터, 우리가 임베드/재배포 안 함 |
| **LM Studio (llmster)** | proprietary (외부 앱) | attach | ✅ | 사용자가 별도 설치, 우리는 HTTP attach만 |
| **vLLM** | Apache-2.0 | subprocess | 🟡 | Linux+CUDA 우선, Win 후순위 |
| TensorRT-LLM | Apache-2.0 (NVIDIA SLA 요건) | subprocess | ⚪ | 고성능 옵션, 라이선스 까다로움 |
| ExecuTorch | BSD | embed-py 또는 subprocess | 🟡 | 온디바이스 패키징 (워크벤치) |
| ONNX Runtime GenAI | MIT | subprocess | 🟡 | 일부 SLM, 워크벤치 |
| MediaPipe LLM Inference | Apache-2.0 | reference | ⚪ | 모바일/온디바이스 영감 |

## 7.4 모델 카탈로그 / 메타

| OSS | 라이선스 | 통합 방식 | v1 | 비고 |
|---|---|---|---|---|
| Hugging Face Hub API | (서비스, 라이선스는 client SDK 별도) | reference + http call | ✅ | manifest 보강용. 핵심 신뢰 소스는 우리 manifest |
| huggingface_hub (py) | Apache-2.0 | embed-py | 🟡 | 워크벤치 |
| GGUF tooling (gguf-py) | MIT | embed-py | 🟡 | 양자화/메타 추출 |

## 7.5 Gateway / 멀티-프로바이더 (옵션)

| OSS | 라이선스 | 통합 방식 | v1 | 비고 |
|---|---|---|---|---|
| **LiteLLM** | MIT | reference / future remote-mode subprocess | 🟡 | ADR-0007 — v1 핵심 의존성 아님. v2 remote/team mode 옵션 |
| Open WebUI | MIT | reference | ⚪ | UX 영감 |
| Continue | Apache-2.0 | reference | ⚪ | 모델 role 영감 |
| Qwen Code | Apache-2.0 | reference | ⚪ | provider/IDE 패턴 영감 |

## 7.6 ML Workbench (Python sidecar, v1 placeholder)

| OSS | 라이선스 | 통합 방식 | v1 | 비고 |
|---|---|---|---|---|
| transformers | Apache-2.0 | embed-py | 🟡 | placeholder, 활성 시 |
| PEFT | Apache-2.0 | embed-py | 🟡 | LoRA |
| TRL | Apache-2.0 | embed-py | 🟡 | preference tuning |
| Axolotl | Apache-2.0 | embed-py | 🟡 | SFT 워크플로 |
| LlamaFactory | Apache-2.0 | embed-py | 🟡 | SFT 워크플로 |
| Unsloth | Apache-2.0 | embed-py | 🟡 | 빠른 SFT |
| bitsandbytes | MIT | embed-py | 🟡 | 양자화 |

## 7.7 외부 API (SaaS)

| 서비스 | 사용 범위 | v1 | 비고 |
|---|---|---|---|
| Google Gemini API | 한국어 설명 멘트만 (ADR-0013) | ✅ (opt-in) | 추천/판정 사용 금지 |

## 7.8 라이선스 매트릭스 요약

- **MIT / Apache-2.0** 위주 — 데스크톱 본체 결합 가능, 재배포 자유.
- **AGPLv3 (KoboldCpp)** — 별도 프로세스 attach 형태로 결합도를 낮춘다. 법무 검토를 통해 어댑터-only 결합이 GPL 영향권 밖임을 확인하고, 그래도 의문이 남으면 **사용자가 별도 설치하는 attach 모드**로 우선 출시(우리가 KoboldCpp 바이너리를 자동 다운로드/배포하지 않음).
- **Proprietary (LM Studio)** — 우리는 HTTP attach만, 사용자가 직접 설치. 재배포 0.
- **Hugging Face TOS** — 다운로드 시 사용자 동의 게이트(라이선스 표시).

## 7.9 dependency 추가 정책

새 OSS 추가 시:
1. 라이선스 확인 → 본체 결합 가능 여부.
2. 성숙도 확인(최근 12개월 활동, 메인테이너 수, security advisory).
3. 우리가 trait 뒤로 숨길 수 있는지(어댑터 가능?).
4. 통합 방식 결정 후 ADR(필요 시) 작성.
5. CI에 회귀 테스트 추가.

## 7.10 의도적으로 배제

- 자체 추론 엔진 작성 → 거부 (성숙 OSS 활용).
- 데스크톱 본체에 Python 임베드 강제 → 거부 (워크벤치 활성 시 옵션).
- LiteLLM v1 필수 의존 → 거부 (ADR-0007).
- Ollama 단일 종속 → 거부 (사용자 명시).

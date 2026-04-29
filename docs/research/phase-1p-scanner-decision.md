# Phase 1' — `crates/scanner` 결정 노트

> 보강 리서치 (2026-04-27) 종합. ADR-0020 (Self-scan local LLM) + ADR-0013 (Gemini boundary) 구현체.

## 1. 핵심 결정

| 항목 | 결정 | 근거 |
|---|---|---|
| 스케줄러 | `tokio-cron-scheduler ^0.15.1` (default-features = false) | Stable, no PG/Nats deps, in-memory OK |
| Cron 표현식 | `"0 0 */6 * * *"` (6-field UTC) | 6시간 주기 |
| 트리거 | (1) 6h cron (2) on-launch grace 5분 후 + 마지막 점검 12h+ (3) UI on-demand `scan_now` | 이중/삼중 안전망 |
| 동시 실행 | `tokio::sync::Mutex<()>::try_lock()` — 중복 시 `AlreadyRunning` 에러 | semaphore 대신 단순 mutex |
| Ollama API | `POST /api/generate { stream: false, keep_alive: "30s" }` | ADR-0020 명시 |
| Model cascade | EXAONE3.5 2.4B → 1.2B → 7.8B → HCX-SEED 8B → Qwen2.5 3B → Llama3.2 3B | 한국어 1순위 |
| Cascade 캐시 | `GET /api/tags` 결과 1h TTL — 매 scan에서 재query 안 함 | 비용 절감 |
| Auto-pull | **금지** (v1) — ADR-0020 §2 | 디스크 폭발 + 오프라인 정책 |
| Deterministic 우선 | LLM 미사용/실패 시 항상 한국어 템플릿으로 fallback (`tracing::info`만, 사용자에겐 정상) | ADR-0013 보장 |
| LLM 검증 | hangul char 비율 ≥30% + 길이 <800 + chat-template 누수 없음 | 안전 그물 |
| 결과 채널 | `tokio::sync::broadcast` (capacity 8) — Tauri emit + future log + UI 다중 구독 | callback보다 유연 |
| EnvironmentProbe | `Arc<dyn EnvironmentProbe>` async trait — runtime-detector 기본 구현 + 테스트는 mock | 테스트 용이성 |
| `last_scan_at` 영속화 | JSON 단일 파일 (옵션) — None이면 영속화 비활성 (테스트/dev) | 단순 |
| 한국어 카피 | 해요체. 사용자에게 LLM 실패 노출 안 함 (graceful degradation) | UX |

## 2. Deterministic 체크 (RAM/Disk/GPU/Driver/WebView2/VC++/CUDA/Vulkan/Runtime)

각 체크 = `Option<CheckResult>`. 적용 안 되는 체크 (예: Win 외 NVIDIA driver) 자동 skip.

| Check | Severity 기준 |
|---|---|
| RAM | <8GB Warn / <16GB Info "7B 가능" / <32GB Info "13B 가능" / ≥32GB Info "30B+ 가능" |
| Disk free | <20GB Warn / <50GB Info |
| GPU | none/iGPU Info / NV ≥8GB Info / NV 4-8GB Warn / NV <4GB Warn |
| NVIDIA driver | Win <530.0 Warn / 누락 Info |
| WebView2 (Win) | 누락 Error "필수" |
| VC Redist 2022 (Win) | 누락 Error |
| CUDA | 미설치 Info |
| Vulkan | 미설치 Info |
| Ollama | running Info OK / installed Info / not-installed Info |
| LM Studio | 동일 |

## 3. 파일 구조 (~700 LOC)

```
crates/scanner/
├── Cargo.toml                     ~30
├── src/
│   ├── lib.rs                     ~150
│   ├── checks.rs                  ~200
│   ├── llm_summary.rs             ~200
│   ├── templates.rs               ~80
│   ├── scheduler.rs               ~70
│   └── error.rs                   ~50
└── tests/
    └── integration_test.rs        ~200
```

## 4. 테스트 케이스 (목표 ~10)

1. LLM happy path → `summary_source = "llm"`, model 매칭.
2. Ollama unreachable → deterministic fallback.
3. /api/tags 200 + 빈 모델 → deterministic fallback.
4. LLM 응답 hangul 30% 미만 → deterministic fallback.
5. RAM <8GB → Warn 체크 산출.
6. Disk <20GB → Warn 체크 산출.
7. WebView2 누락 (Win) → Error 체크.
8. `scan_now` 두 번 동시 → 두 번째 `AlreadyRunning`.
9. Cascade 캐시 1h TTL — 첫 호출만 /api/tags hit.
10. on-launch grace — 마지막 점검 12h 내면 skip.

## 5. 비목표

- 자동 모델 pull — Phase 2' opt-in.
- 시스템 idle 감지 (Win Power Idle Detection 등) — 후순위.
- 점검 결과 외부 전송 — ADR-0020 §3 금지.
- LLM streaming — 단일 응답.

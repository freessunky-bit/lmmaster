# Phase 1A.4.b — Step 2 환경 점검 결정 노트

> 보강 리서치 (2026-04-27) 인라인 종합. 새 보안/성능 트레이드오프 없음 — 기존 패턴 조립.

## 1. 핵심 결정

| 항목 | 결정 | 근거 |
|---|---|---|
| 결과 타입 위치 | `crates/shared-types::EnvironmentReport { hardware: HardwareReport, runtimes: Vec<DetectResult> }` | shared-types가 본래 그 목적. Tauri command/web app 모두에서 재사용 |
| 합성 함수 위치 | `crates/runtime-detector::probe_environment()` async — 두 crate를 병렬 호출 | runtime-detector가 이미 hardware-probe consumer 자격 + 의존성 방향 자연스러움 |
| Tauri command | `detect_environment(app: AppHandle) -> Result<EnvironmentReport, EnvApiError>` (async) | hardware-probe + runtime-detector 모두 async + 1.5~2초 |
| 권한 ACL | `permissions/probe.toml` 신설 — `allow-detect-environment` | install.toml과 분리해 영역 명확 |
| capability | `capabilities/main.json`에 `allow-detect-environment` 추가 | |
| xstate scan state | `invoke: { src: 'scan', input: () => undefined, onDone: { actions: 'setEnv' }, onError: { actions: 'setError' } }`. **자동 진입** (별도 trigger 이벤트 없음) | 사용자가 Step 2로 들어오자마자 즉시 시작 — UX 표준 |
| scan actor | `fromPromise(({ signal }) => detectEnvironment().catch(...))` | xstate v5 표준. signal은 detect_environment가 abort 미지원이라 무시 (~2초로 짧음) |
| 결과 표시 카드 | 4 그룹: OS / 메모리 / GPU / 런타임. 각 카드는 status pill + 핵심 fact 1~2줄 | Foundry Local / VS Code Welcome 패턴 |
| 이미 설치 감지 | runtimes 중 LM Studio 또는 Ollama가 Running이면 Step 3에서 자동 SKIP 옵션 노출 | "이미 사용 중이에요" 안내 후 NEXT/SKIP 둘 다 보여줌 |
| RAM/디스크 임계 | 8GB RAM 이하 / 20GB 디스크 이하 → 경고 (warn 톤). **차단 안 함** | LM Studio/Ollama 최소 사양 기반. 사용자에게 결정권 |
| 한국어 카피 | 해요체 — "GPU를 찾았어요" / "메모리는 16.0GB 정도 보여요" / "Ollama가 이미 사용 중이에요" / "잠시만 기다려 주세요" | Toss 8원칙 일관 |
| 비활성 NEXT | scan 중 또는 실패 상태에서 NEXT 비활성. 성공 시 활성 + "계속할게요" | 데이터 없이 다음 단계 진입 방지 |
| 로딩 표시 | 카드별 skeleton + 전체 "환경을 살펴보고 있어요…" 캡션 | 한 번에 표시 — 점진 노출은 깜빡임 |

## 2. 새 타입 (`crates/shared-types`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentReport {
    pub hardware: hardware_probe::HardwareReport,
    pub runtimes: Vec<runtime_detector::DetectResult>,
}
```

shared-types는 type만 정의 — 비-async crate. probe_environment 함수는 runtime-detector에 위치.

## 3. probe_environment (runtime-detector)

```rust
pub async fn probe_environment() -> EnvironmentReport {
    let detector = Detector::with_default_config().expect("detector init");
    let (hardware, runtimes) = tokio::join!(
        hardware_probe::probe(),
        detector.detect_all(),
    );
    EnvironmentReport { hardware, runtimes }
}
```

## 4. Tauri command (`apps/desktop/src-tauri/src/commands.rs`)

```rust
#[tauri::command]
pub async fn detect_environment() -> Result<EnvironmentReport, EnvApiError> {
    Ok(runtime_detector::probe_environment().await)
}
```

`EnvApiError`는 현재 점검 함수가 항상 성공 (graceful fail 내장)하므로 `Infallible` 비슷하게 두지만, 미래 확장성을 위해 enum으로.

## 5. xstate 머신 보강

```ts
actors: {
  scan: fromPromise<EnvironmentReport>(async () => detectEnvironment()),
}
// state.scan:
{
  invoke: {
    src: 'scan',
    onDone:  { actions: 'setEnv' },
    onError: { actions: 'setScanError' },
  },
  on: { BACK: 'language', NEXT: { target: 'install', guard: 'hasEnv' } },
}
```

`hasEnv` guard로 env가 없으면 NEXT 차단.

## 6. UI 카드 디자인

```
┌──────────────────────────────────────────────────┐
│ 환경을 살펴봤어요              잠시만 기다려요…  │ ← 헤더
├──────────────────────────────────────────────────┤
│ 운영체제                          OK             │
│ Windows 11 (10.0.26200) · x86_64                 │
├──────────────────────────────────────────────────┤
│ 메모리                            OK             │
│ 16.0GB 사용 가능 / 32.0GB 전체                   │
├──────────────────────────────────────────────────┤
│ GPU 가속                          NVIDIA         │
│ NVIDIA GeForce RTX 4080 · 16.0GB VRAM            │
├──────────────────────────────────────────────────┤
│ 런타임                                           │
│ • Ollama        사용 중       v0.3.x             │
│ • LM Studio     설치 안 됨                       │
└──────────────────────────────────────────────────┘
```

핵심 토큰:
- status pill 색: OK = `--primary` / 경고 = `--warn` / NotInstalled = `--text-muted` / Running = `--primary` glow
- 카드 구분: `--border` divider + `--space-4` padding

## 7. 비목표 (1A.4.b 외)

- 결과 cache (TTL) — 후순위 (현재 매번 새로 점검)
- 사용자 트리거 재점검 버튼 — 후순위 (자동 1회)
- 권장 모델 추천 (RAM/GPU 기반) — Phase 2'
- 자동 skip ("이미 LM Studio가 모델까지 갖춘 경우 Step 3 자동 SKIP") — 1A.4.c에서 옵션 노출

## 8. 추가 의존성

- 없음. 모두 기존 crate 재조합.

## 9. 검증 체크리스트

- `cargo clippy --workspace --all-targets -- -D warnings` 통과
- `cargo test --workspace` 통과 + shared-types에 Serialize round-trip 1건 (선택)
- `pnpm exec tsc -b` 통과
- `pnpm run build` Vite production 통과
- (사용자 측) `pnpm tauri:dev`로 Step 2 진입 시 자동 점검 시작 + 결과 카드 노출 + Korean copy 검증

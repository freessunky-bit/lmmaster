# ADR-0041: GlitchTip self-hosted telemetry endpoint + opt-in DSN env var

- Status: Accepted
- Date: 2026-04-28
- Related: ADR-0013 (외부 통신 0 — opt-in 단일 도메인 예외 허용), ADR-0027 §5 (telemetry 정책 — opt-in only + crash report 1차), Phase 7'.a (TelemetryConfig + UUID 영속), Phase 7'.b (본 ADR)
- 결정 노트: `docs/research/phase-7p-release-prep-reinforcement.md` §5

## Context

Phase 7'.a에서 TelemetryConfig + UUID 발급 + 영속까지 구현했어요. v1 launch를 앞두고 *실 endpoint*를 결정해야 해요.

핵심 질문 4개:

1. **endpoint를 자체 운영할까, SaaS로 위탁할까?**
2. **opt-in 사용자에게 endpoint를 어떻게 주입할까?**
3. **endpoint 미설정 시 이벤트 큐의 동작은?**
4. **panic / crash report와 수동 호출 이벤트의 분기는?**

## Decision

### 1. GlitchTip self-hosted 단일 채택

Sentry envelope JSON shape 호환 + 모든 데이터를 우리 서버에만 보관. ADR-0013 외부 통신 0 정책의 *opt-in 단일 도메인 예외*로 간신히 통과해요.

### 2. DSN 환경변수 `LMMASTER_GLITCHTIP_DSN` 주입

- 형식: `https://<key>@<host>[:port]/<project_id>` (Sentry 표준).
- 주입 시점: 런타임. 빌드 시점에 컴파일하지 않음 — 사용자가 자체 GlitchTip 서버를 운영하는 경우에도 우회 가능.
- v1: env var이 설정돼 있지 않으면 *완전히 비활성*. 큐에만 적재 + 외부 통신 0.
- v1.x: 사용자 향 dialog ("자체 telemetry 서버를 쓰고 싶나요?")로 in-app 설정 검토.

### 3. EventQueue cap=200 + oldest drop + 24h retention

- 큐 cap 200 — 사용자 PC 메모리 부담 최소화.
- 초과 시 oldest drop (FIFO).
- 24h 초과 이벤트는 `evict_expired`로 청소.
- DSN 미설정 시 retention only (drop 없음, 단 24h 초과는 expire).
- DSN 설정 시 backon 3회 retry (즉시 / 1s / 5s). 실패한 이벤트는 큐에 보존되어 다음 cycle 재시도 여지.

### 4. panic_hook이 자동 submit, frontend는 수동 호출 가능

- `panic_hook::handle_panic`은 TelemetryState가 attach돼 있고 opt-in 상태면 `submit_event(level: error, ...)` 호출.
- Frontend `submit_telemetry_event(level, message)` IPC도 노출 — 보통은 미사용. v1.x 디버그 도구가 활용 여지.
- opt-out 상태면 `NotEnabled` 에러로 조용히 거절 — 통신 시도 자체가 발생하지 않음.

### 5. 프롬프트 / 모델 출력은 절대 전송 X

- TelemetryEvent는 `level / message / event_id / timestamp / platform / anon_id`만 포함.
- panic message는 *Rust panic payload + location*만. 사용자 프롬프트 / 모델 응답은 panic info에 도달하지 않음.
- v1.x에서 추가 메트릭(예: 모델 호출 빈도)은 별도 ADR로 평가 + 사용자 향 별도 토글.

## Consequences

### Positive

- **사용자 데이터 통제권** — 모든 telemetry 데이터를 우리(또는 사용자가 운영하는) 서버에만 저장.
- **opt-in + DSN 미설정 시 비활성** — 사용자가 토글을 켜도 endpoint를 명시 주입하지 않은 한 외부 통신 0.
- **panic recovery 자동화** — opt-in 사용자가 별도 작업 없이 crash를 운영팀에 전달 가능.
- **Sentry SDK 호환** — GlitchTip 외에도 self-hosted Sentry / 미래 호환 도구로 전환 가능.
- **외부 의존 최소** — `reqwest` 1개 + JSON serialize. Sentry SDK crate 의존 X (compile time / binary size 절약).

### Negative

- **GlitchTip 서버 운영 부담** — VPS ~$10/월 + 보안 패치 + DB 백업 사내 SOC 일관성 유지.
- **재시도 후 실패 이벤트 누적 위험** — 큐가 200까지 채워지면 oldest drop. 운영팀이 v1.x에서 startup flush 추가하면 회복 가능.
- **DSN 형식 검증 약함** — v1은 `parse_dsn`이 host/port/project_id만 추출. URL 인코딩 / IPv6 / 특수문자는 v1.x에서 강화.
- **panic hook 안에서 async spawn** — Tauri runtime이 살아 있을 때만 동작. shutdown 도중 panic은 큐 적재가 안 될 수 있음 (best-effort).

## Alternatives considered

### A. Sentry SaaS

**거부**. ADR-0013 외부 통신 0 위반. 첫 외부 의존(sentry.io) + 사용자 데이터가 3rd party에 도달 → 신뢰 thesis 정면 충돌. 무료 5K/월 quota는 매력적이지만 채택 조건이 thesis와 맞지 않아요.

### B. Firebase Crashlytics

**거부**. Google에 데이터 송신 + Tauri 통합 부담 + iOS/Android 우선 SDK라 데스크톱 부적합. ADR-0013 위반.

### C. 자체 endpoint (직접 구현 — POST /telemetry)

**거부 (v1, v1.x 재검토)**. 우리가 stack trace 분석 / aggregation UI / search를 만들어야 함 → 운영 부담 크고 Sentry SDK 호환 도구의 가치를 잃어요. v1은 GlitchTip 한 단계로 시작, v1.x에서 사용량이 늘면 자체 endpoint 검토.

### D. plausible.io / posthog 같은 product analytics

**거부**. 우리는 *사용 통계*가 아닌 *crash report*가 1차. plausible은 page view 중심, posthog는 이벤트 분석 중심. v1은 crash만, v1.x에서 product analytics 별도 ADR.

### E. DSN을 빌드 시점에 컴파일

**거부**. 사용자가 자체 운영 GlitchTip을 가리키지 못함. 다중 deployment(개발 / 스테이징 / 프로덕션)에 별도 빌드 필요. 환경변수가 더 유연해요.

### F. 사용자 PC에 endpoint를 캐시 (config.json)

**거부 (v1, v1.x 재검토)**. config.json이 disk에 평문 저장되면 사용자가 실수로 noise(URL typo)를 commit할 위험. v1은 env var만, v1.x에서 사용자 향 설정 dialog + workspace config 분리.

## 검증 invariant (Phase 7'.b §10.4)

- **DSN 미설정 + opt-out** → submit 호출 자체 거절 (NotEnabled).
- **DSN 미설정 + opt-in** → queue 적재 (Queued).
- **DSN 설정 + opt-in + endpoint 응답 정상** → Sent.
- **DSN 설정 + opt-in + endpoint 도달 실패** → Retained + 큐 보존.
- **큐 cap 200 초과** → oldest drop (FIFO).
- **24h 초과 이벤트** → `evict_expired`로 제거.
- **panic hook 통합** → opt-in 상태에서 panic 발생 시 submit_event 자동 호출.
- **opt-out → opt-in 전환** → UUID 보존 (Phase 7'.a invariant 유지).

## References

- ADR-0013 (외부 통신 0 — opt-in 단일 도메인 예외).
- ADR-0027 §5 (telemetry 정책 1차).
- Phase 7' 보강 리서치 §5 (`docs/research/phase-7p-release-prep-reinforcement.md`).
- GlitchTip self-hosted: <https://glitchtip.com/>
- Sentry envelope spec: <https://develop.sentry.dev/sdk/envelopes/>
- backon retry crate: <https://docs.rs/backon/>

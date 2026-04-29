# ADR-0015: 타입 공유는 specta + tauri-specta + 빌드타임 codegen

- Status: Accepted
- Date: 2026-04-26

## Context
LMmaster는 (1) `crates/shared-types`(Rust)와 (2) `packages/js-sdk`/(`apps/desktop` 프런트) 양쪽에서 동일한 도메인 타입(ModelRef, RuntimeKind, ApiKeyScope 등)을 사용한다. 손으로 두 정의를 동기화하면 drift가 보장된다. Phase 0 보강 리서치에서 specta v2 + tauri-specta v2가 (a) 타입 + (b) 타입화된 command/event 래퍼를 동시에 생성하며, ts-rs 대비 의존 그래프 전체를 따르고 namespace/branded newtype/이벤트 타입을 지원함을 확인.

## Decision
- `crates/shared-types`의 모든 공개 타입에 `specta::Type` derive를 추가한다.
- `apps/desktop/src-tauri`는 **tauri-specta v2** 통합. `#[tauri::command]` + `#[specta::specta]` 페어 사용.
- 빌드 시점에 `apps/desktop/src/generated/`(프런트)와 `packages/js-sdk/src/generated/`(SDK)로 코드를 emit한다.
- **`packages/js-sdk`의 공개 API**는 `generated/`를 직접 export하지 않는다 — `src/index.ts`에서 수작업으로 re-export하면서 외부 API 경계를 안정화한다.
- CI 가드: `cargo run -p lmmaster-bindings-export && git diff --exit-code <generated paths>`. drift 시 fail.

생성기 위치:
- 옵션 A: `apps/desktop/src-tauri/build.rs`에서 emit (Tauri 빌드 시 자동).
- 옵션 B: 전용 small crate `crates/bindings-export`의 `bin`. CI에서 명시 호출.
- 두 방식 병행: 빌드 시 자동(개발 편의) + CI 가드(브랜치 보호).

## Consequences
- shared-types가 single source of truth.
- `js-sdk` 외부 API는 generated 변경에 영향 받지 않음(re-export 레이어가 흡수).
- specta는 추가 의존성 + macro. 컴파일 시간 약간 증가 — 측정값을 phase별 verify에 기록.
- 일부 Rust 타입(serde tag, untagged enum)은 specta가 표현 못 할 수 있음 — 등장 시 ADR 보강.

## Alternatives considered
- **ts-rs**: 구조체 단위 emit, 의존 그래프 follow 약함. 거부.
- **수작업 mirror + lint**: 휴먼 에러 누적. 거부.
- **OpenAPI/JSON Schema 기반 codegen**: gateway HTTP 스펙엔 적합하지만 IPC 타입엔 무거움. v2에 OpenAPI 도입 고려.

## References
- https://github.com/specta-rs/specta
- https://github.com/specta-rs/tauri-specta
- spacedriveapp/spacedrive (실사용 사례)
- 보강 리서치: docs/research/phase-0-reinforcement.md (§3)

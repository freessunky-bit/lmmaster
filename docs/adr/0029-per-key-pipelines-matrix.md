# ADR-0029: Per-key Pipelines override (enabled_pipelines)

- Status: Accepted
- Date: 2026-04-28
- Related: ADR-0022 (Gateway routing), ADR-0025 (Pipelines architecture), ADR-0028 (hot-reload)
- Phase: 8'.c.3

## Context

ADR-0025 §3에서 *per-key activation matrix*를 약속했지만 Phase 6'.b/6'.c에서는 **전역 토글**만 구현했어요. 시나리오:

- 블로그 키는 PII redact + observability만 — token-quota 불필요 (사용자가 자기 토큰 안에서 도는 web app).
- 내부 dev 키는 모든 Pipeline 비활성 — debug용 raw passthrough.
- 외부 위탁업체 키는 모든 Pipeline 강제 — 사용자가 토글 못 끄게 (per-key가 전역을 override).

전역 토글만으로는 위 3 use-case를 동시에 만족할 수 없어요. 키 단위 화이트리스트 필요.

## Decision

### Schema

`Scope` (key-manager)에 `enabled_pipelines: Option<Vec<String>>` 필드 추가:

- `None` (default) → 전역 토글을 그대로 따름.
- `Some(Vec)` → 명시 화이트리스트로 글로벌 토글 override (그 ID에 포함된 Pipeline만 활성).
- `Some(빈 Vec)` → 모든 Pipeline 비활성 (raw passthrough).

JSON column(`scope_json`)에 직렬화. `serde(default, skip_serializing_if = "Option::is_none")`로 기존 키와 호환 — 누락된 필드는 None으로 deserialize.

### Migration

DB schema는 변경 없음 (scope는 JSON column에 통째로 보관). 기존 키는 deserialize 시 자동으로 `enabled_pipelines: None`으로 채워짐 → 전역 정책 유지.

별도 `PRAGMA user_version` 변경 불필요. `serde(default)`가 마이그레이션의 모든 부담을 흡수.

### Wire

1. `KeyManager::verify` → `AuthOutcome::Allowed { id, alias, enabled_pipelines }`.
2. `auth::require_api_key` 미들웨어 → `req.extensions.insert(KeyPipelinesOverride(enabled_pipelines))`.
3. `PipelineMiddleware::call` → `parts.extensions.get::<KeyPipelinesOverride>()` → `chain.filter_by_ids(...)` → effective sub-chain.
4. effective_chain은 request + response + SSE 모두 동일 — 키 정책 일관성.

### UI

`ApiKeyIssueModal`에 fieldset "이 키에만 적용할 필터":

- "전역 설정 따르기" 체크박스 (default ON, `enabled_pipelines = null`).
- 해제 시 4종 v1 시드 체크박스 그룹 활성화 — 사용자가 명시 화이트리스트 작성.
- 모두 해제 시 경고 ("이 키 호출에는 어떤 필터도 적용되지 않아요").

`updateApiKeyPipelines(id, enabled_pipelines)` IPC로 회수/재발급 없이 부분 갱신 가능.

## Consequences

### 긍정

- 키 단위 정책 — 위탁업체 / 내부 / 블로그 시나리오 동시 지원.
- 전역 + per-key 두 axis가 *명시적*으로 분리 → UX 친화 (사용자는 키별 설정이 필요 없으면 default ON).
- DB 마이그레이션 없음 — `serde(default)` + JSON column 조합.
- ADR-0028 hot-reload와 직교 — 전역 토글 변경도 키별 override도 즉시 반영.

### 부정

- 사용자 멘탈 모델: "전역 토글 vs 키별 toggle"이라는 2-axis가 됨. UI에서 명시적 안내 필요.
- 빈 vec 케이스 — UI에서 경고하지만 사용자가 의도적으로 만들 수 있음 (raw passthrough 시나리오).
- per-key Pipeline 정책의 audit는 전역 audit log에 같이 기록 — 키별 audit 분리는 v1.x.

## Alternatives considered + rejected

### 1. 별도 매트릭스 테이블 (`api_key_pipelines (key_id, pipeline_id)`)

- 정규화는 깔끔하지만 매 verify마다 JOIN 추가.
- DB schema migration (테이블 신설 + index) — `PRAGMA user_version` 관리.
- JSON column 패턴이 단순/효율적 — Pipeline ID 4종이라 정규화 이득 미미.

### 2. Hard-coded per-key ID 매핑 (코드 내 if-else)

- 유연성 0 — 새 키 추가마다 코드 수정.
- 사용자가 직접 GUI에서 조작 불가.

### 3. 외부 정책 엔진 (OPA / Casbin)

- LMmaster의 외부 통신 0 정책 위반.
- 의존성 무거움 (RBAC 엔진 ~10MB).
- 4종 화이트리스트는 코드 50줄로 충분.

### 4. 전역만 + per-route activation

- ADR-0025에서 약속한 per-key 폐기 → 약속 위반.
- 키 별 분리 시나리오 (위탁/내부/블로그) 충족 불가.

## Open follow-ups (v1.x)

- 키별 audit log 분리 (현재 ring buffer는 전역).
- per-route × per-key 매트릭스 (현재는 per-key만, route는 전역 토글).
- ApiKeyEditPanel — 발급 후 alias / origins / models 변경. v1은 회수 후 재발급만 지원.

## References

- `crates/key-manager/src/scope.rs::Scope::enabled_pipelines` — 데이터 모델.
- `crates/pipelines/src/chain.rs::PipelineChain::filter_by_ids` — sub-chain 빌드.
- `crates/core-gateway/src/pipeline_layer.rs::KeyPipelinesOverride` — extension 마커.
- `apps/desktop/src/components/keys/ApiKeyIssueModal.tsx` — UI 토글.
- `apps/desktop/src-tauri/src/keys/commands.rs::update_api_key_pipelines` — 부분 업데이트 IPC.

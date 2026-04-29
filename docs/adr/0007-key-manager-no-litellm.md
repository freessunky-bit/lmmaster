# ADR-0007: v1 로컬 키 매니저 자체 구현 (LiteLLM 비고정)

- Status: Accepted
- Date: 2026-04-26

## Context
다중 클라이언트(기존 웹앱, 다른 웹앱)가 한 LMmaster를 호출하므로 API 키 발급/취소/scope/사용 로그가 필요하다. LiteLLM은 키 매니저/프록시로 강력하지만 (1) Python 의존성 (2) 데스크톱 portable 단일 PC 환경에 무거움 (3) v1 핵심 결합도가 너무 높음.

## Decision
v1은 **자체 경량 키 매니저**(Rust + SQLite)로 시작한다. 기능:
- `issue(scope, models, project_id?, expires_at?) -> ApiKey`
- `revoke(key_id)`
- `list()`
- `verify(key_string) -> Scope` (gateway middleware)
- 사용 로그(요청/토큰/모델/시각)를 SQLite에 append.

LiteLLM은 향후 **remote/team mode** 옵션으로 별도 ADR에서 검토. v1 기본 경로에는 들어가지 않는다.

## Consequences
- 데스크톱 portable에 적합한 경량 코드.
- 키 권한 모델: scope = {`models: glob[]`, `endpoints: glob[]`, `rate_limit?`, `quota?`, `project_id?`}.
- LiteLLM이 가진 다중 provider 라우팅이 필요해질 때 별도 모듈로 도입 가능.

## Alternatives considered
- **LiteLLM 임베드**: Python 사이드카 필수, v1 부담. 거부.
- **무인증 localhost**: 동일 PC의 다른 사용자/악성 프로세스 위협. 거부.

## References
- ADR-0008 (SQLite)

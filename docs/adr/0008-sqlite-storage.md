# ADR-0008: 메타데이터 저장소로 SQLite + 옵션 SQLCipher

- Status: Accepted
- Date: 2026-04-26

## Context
모델/런타임/키/사용 로그/프로젝트 바인딩/설정을 단일 PC에서 안정적으로 저장해야 한다. 멀티-프로세스 동시 접근(메인 + 워크벤치 worker) 가능성. 시크릿(API 키, 외부 토큰)은 평문 저장 회피 필요.

## Decision
- 일반 메타데이터는 **SQLite**(WAL 모드).
- 시크릿/민감 설정은 **SQLCipher**(또는 동등한 pluggable layer)로 분리.
- Rust 클라이언트는 우선 **rusqlite + r2d2** 또는 **sqlx**(컴파일 타임 쿼리 검증). 결정은 v1 스캐폴딩 시 미세 조정.
- 마이그레이션은 **refinery** 또는 **sqlx migrate** 중 채택.

논리적 DB 파일 분리:
- `data/metadata.sqlite` — 모델/런타임/사용 로그/프로젝트 바인딩
- `data/secrets.sqlcipher` — API 키, 외부 토큰, gemini key 등
- `data/cache.sqlite` — registry cache, hardware probe 결과 캐시

## Consequences
- 단일 파일 portable.
- 멀티 프로세스 접근은 WAL로 처리. 워크벤치 worker가 secrets DB에 접근할 일은 거의 없도록 설계.
- 백업/복원은 파일 복사로 충분.
- SQLCipher 키 자체의 저장: OS keychain(Win Credential Manager / macOS Keychain / Linux libsecret)에 위임. portable 모드 시 사용자 입력 패스프레이즈로 derive.

## Alternatives considered
- **JSON 파일들**: 동시성/일관성 부족. 거부.
- **embedded RDBMS (sled, redb)**: 가능하지만 SQL 쿼리/마이그레이션 도구 생태계가 SQLite보다 약함. 거부.
- **Postgres embedded**: 오버킬. 거부.

## References
- ADR-0009 (Portable workspace)

# 어댑터 작성 가이드 (개발자)

새 런타임을 추가하려면 `RuntimeAdapter` trait을 구현한 새 crate를 만든다.

## 절차

1. `crates/adapter-<name>/Cargo.toml` 생성.
2. `crates/adapter-<name>/src/lib.rs`에 `pub struct <Name>Adapter;` + `impl RuntimeAdapter`.
3. `Cargo.toml` workspace members에 추가.
4. `crates/runtime-manager`의 등록 헬퍼에 새 어댑터 등록 (M2 도입 후).
5. CI 회귀 테스트 추가 (`tests/<name>_smoke.rs`).
6. capability_matrix를 정확히 채울 것 (vision/tools/structured/embeddings).

## 어댑터 카테고리

- **subprocess**: 우리가 자식 프로세스로 spawn (예: llama.cpp, vLLM).
- **attach**: 사용자가 별도 설치, 우리는 HTTP 연결만 (예: Ollama, LM Studio).

## 라이선스 검토

- AGPLv3, GPL 등 강결합 시 결합 형태(attach 우선)와 법무 의견을 ADR로 기록.
- 자동 다운로드/재배포 전 라이선스 매트릭스(`docs/oss-dependencies.md`) 갱신.

## 금지

- gateway 또는 GUI 코드를 어댑터 안에서 직접 import 금지 (의존 방향 규칙).
- raw runtime port를 외부에 노출 금지.
- if-else로 다른 어댑터 동작에 끼어들지 말 것.

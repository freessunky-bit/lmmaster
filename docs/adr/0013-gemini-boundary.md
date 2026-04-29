# ADR-0013: Gemini API는 한국어 설명용으로만, 판정/추천은 deterministic

- Status: Accepted
- Date: 2026-04-26

## Context
한국어 친절한 설치 도우미 멘트, FAQ 자동 응답 등에 LLM이 유용. 그러나 외부 LLM에 deterministic 결정(하드웨어 적합도, 모델 추천, 설치 성공 판정)을 위임하면 (1) 오프라인 시 작동 불능 (2) 응답 변동성으로 같은 PC에 다른 추천 (3) 규제/과금/프라이버시 (4) 디버깅 어려움.

## Decision
Gemini API의 책임은 다음으로 한정한다:
- 사용자에게 **이미 결정된 결과**를 한국어로 친절하게 풀어서 설명 (예: "당신의 GPU는 RTX 4060 Ti, VRAM 16GB라서 7B Q4 모델이 권장됩니다. 사유는 …")
- FAQ/도움말 보강
- onboarding 멘트 변주

Gemini가 **하지 않는 것**:
- 하드웨어 적합도 판정
- 모델 추천 결정
- 설치 성공/실패 판정
- 헬스체크 결과 해석
- "이 모델이 너의 PC에 맞다"는 결정

판정/추천 로직은 모두 우리 Rust 코드(deterministic). 그 결과 객체를 Gemini에게 한국어 설명 요청. Gemini 응답이 실패하거나 비활성 상태여도 앱 동작에 지장 없도록 fallback 한국어 템플릿 항상 보유.

## Consequences
- 오프라인에서도 모든 핵심 기능 동작.
- 같은 PC + 같은 모델 카탈로그 = 같은 추천.
- Gemini 사용은 전적으로 사용자 동의(설정 → "한국어 설명 강화 사용") 후에만.
- API 키는 시크릿 DB(SQLCipher)에 보관.

## Alternatives considered
- **Gemini가 추천도 함께**: 거부. 비결정성과 오프라인 의존.
- **Gemini 미사용**: 가능. v1에서 사용자 토글로 둔다.

## References
- ADR-0010 (Korean-first)

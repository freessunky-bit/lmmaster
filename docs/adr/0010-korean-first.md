# ADR-0010: UI/문서 한국어 우선 (ko-KR 기본)

- Status: Accepted
- Date: 2026-04-26

## Context
1차 사용자는 한국어권. 영어 우선 설계 후 번역하면 톤이 어색해지고 정보 누락이 잦다. 사용자 명시 요구.

## Decision
- 기본 locale은 `ko-KR`. 모든 UI 문자열, 오류 메시지, onboarding, 도움말, README, 가이드 문서를 한국어로 작성한다.
- 영어(en-US)는 i18n 키 자리만 비워두고 v2에서 채운다.
- 기술 오류는 사용자용 한국어 설명을 1차로, 원문 메시지를 expandable 부가 정보로 제공.
- i18n 라이브러리: 프런트는 **i18next**(또는 동등) + 키 베이스. 백엔드 오류 코드는 enum + 한국어/영어 message catalog.

## Consequences
- 디자인 시 한국어 폰트(특히 mono 영역의 한국어 fallback)·줄바꿈 정책을 우선 검토.
- 한국어 카피의 톤은 "친절하지만 실무형, 마케팅 톤 금지". 디자인 시스템에 voice & tone 가이드 포함.
- 영어 사용자 대응은 v2 별도 마일스톤.

## Alternatives considered
- **영어 우선 + 자동 번역**: 톤 품질 저하. 거부.
- **양 언어 동시**: v1 부담. 거부, v2로 미룸.

## References
- ADR-0013 (Gemini는 한국어 설명용)

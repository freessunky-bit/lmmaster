# ADR-0001: Companion 데스크톱 + localhost gateway 구조 채택

- Status: Accepted
- Date: 2026-04-26

## Context
기존 HTML 웹앱과 다른 웹앱들이 로컬 LLM/STT/TTS 추론을 사용해야 한다. 사용자 요구사항은 (1) 기존 웹앱은 호출만, (2) 직접 런타임을 붙들지 않음, (3) provider abstraction에 1개 추가하는 수준의 변경, (4) install/health/recommend 책임은 새 프로그램이 진다.

## Decision
별도 데스크톱 프로그램(LMmaster)을 만들고 그 내부에 localhost-only HTTP gateway를 띄운다. 클라이언트(기존 웹앱 포함)는 OpenAI-compatible REST 또는 JS/TS SDK를 통해 이 gateway만 호출한다. 런타임은 LMmaster 내부 Runtime Adapter가 관리한다.

## Consequences
- 기존 웹앱 변경 면적이 SDK 1개 + provider 1개 + 헬스 체크 모달 1개로 제한된다.
- 런타임 교체/추가가 LMmaster 내부 변경으로 끝난다.
- 사용자가 별도 설치라는 단계를 거쳐야 함 — 설치 마찰을 OS 표준 installer + custom URL scheme + 한국어 onboarding으로 완화.
- localhost 포트 충돌 가능성 — 자동 회피 + 헬스 probe로 SDK가 자동 탐지.
- 두 코드베이스 동기화 부담 — semver + capability discovery로 graceful degrade.

## Alternatives considered
- **직접 통합**: 웹앱이 Ollama/llama.cpp 클라이언트를 직접 import. 결합도 폭증, 런타임 교체 시 웹앱 PR. 거부.
- **브라우저 확장 + native messaging**: 권한 모델 좁음, OS별 native host 매니페스트 필요, 사용자에게 확장 설치 강요. 거부.
- **클라우드 프록시**: "로컬 AI"라는 본 제품 정체성과 충돌. 거부.

## References
- `docs/architecture/01-rationale-companion.md`

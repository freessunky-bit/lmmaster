# LMmaster

> 기존 HTML 웹앱이 **호출만 해서 사용할 수 있는** 로컬/하이브리드 AI 오케스트레이터 데스크톱 프로그램.
> 단순 런처가 아닙니다. **Local AI Companion**입니다.

LMmaster는 사용자 PC에서 실행되어, 기존 웹앱과 다른 웹앱들이 **로컬 HTTP API** 또는 **JS/TS SDK**만 호출해 LLM/STT/TTS 추론을 사용할 수 있게 만드는 데스크톱 프로그램입니다. 런타임의 설치·업데이트·헬스체크·라우팅·폴백·키 관리·하드웨어 적합도 추천을 LMmaster가 모두 책임집니다.

## 핵심 특징

- **호출만 하면 끝** — 기존 웹앱은 `@lmmaster/sdk` 의존성 1개와 provider 1개만 추가하면 됩니다.
- **하드웨어 점검 + deterministic 추천** — 사용자의 PC를 점검해 best/balanced/lightweight/fallback 4종을 결정적으로 추천합니다.
- **다중 런타임** — llama.cpp · KoboldCpp · Ollama · LM Studio · vLLM을 어댑터로 통합. 어디에도 종속되지 않습니다.
- **Portable workspace** — 같은 OS·아키텍처 계열에서 폴더 이동 가능. 환경 변경은 자동 감지·복구.
- **OpenAI-compatible 로컬 게이트웨이** — 기존 OpenAI SDK가 base URL만 바꾸면 동작.
- **API 키 + scope 모델** — 다른 웹앱도 키만 받아서 붙일 수 있습니다.
- **한국어 우선** — UI · 문서 · 오류 메시지 전부 한국어 기본.
- **Dark + 네온 그린** — 기존 웹앱과 동일 디자인 시스템 공유.

## 디렉터리 구조

```
LMmaster/
├─ apps/desktop/              # Tauri 2 + React 데스크톱 앱
├─ crates/                    # Rust 코어 (gateway, runtime, hardware probe, registry, key, ...)
├─ packages/
│  ├─ design-system/          # 공유 토큰/컴포넌트 (데스크톱 + 웹앱)
│  └─ js-sdk/                 # @lmmaster/sdk
├─ workers/ml/                # Python ML 워크벤치 (v1 placeholder)
├─ manifests/models/          # curated 모델 manifest seed
├─ examples/webapp-local-provider/  # 기존 웹앱 통합 데모
└─ docs/                      # ADR, 아키텍처, 한국어 가이드, 개발자 가이드
```

상세는 `docs/architecture/03-repo-tree.md` 참조.

## 빠른 시작 (개발자)

> 사전 요구: Rust(stable), Node 20+, pnpm 9+, Tauri 사전요건(OS별 webview/system 라이브러리 — `https://tauri.app/start/prerequisites/`).

```bash
# 의존성 설치
pnpm install
cargo build --workspace

# 데스크톱 앱 개발 모드
pnpm --filter @lmmaster/desktop tauri dev

# Rust 테스트
cargo test --workspace
```

## 문서

핵심 산출물:

- 아키텍처 요약 — `docs/architecture/00-overview.md`
- Companion 구조 채택 근거 — `docs/architecture/01-rationale-companion.md`
- 단계별 구현 로드맵 (M0~M6) — `docs/architecture/02-roadmap.md`
- Repo 구조 — `docs/architecture/03-repo-tree.md`
- ADR 인덱스 — `docs/adr/README.md`
- 위험요소와 대응책 — `docs/risks.md`
- OSS dependency matrix — `docs/oss-dependencies.md`
- 디자인 시스템 적용 계획 — `docs/design-system-plan.md`

한국어 사용자/개발자 가이드:

- 시작 가이드 — `docs/guides-ko/getting-started.md` *(작성 예정)*
- 모델 설치 가이드 — `docs/guides-ko/install-models.md` *(작성 예정)*
- 기존 웹앱 연동 가이드 — `docs/guides-ko/webapp-integration.md` *(작성 예정)*
- 로컬 API 키 발급 가이드 — `docs/guides-ko/api-keys.md` *(작성 예정)*
- 문제 해결 가이드 — `docs/guides-ko/troubleshooting.md` *(작성 예정)*
- UI 정보 구조 — `docs/guides-ko/ui-ia.md`
- SDK 연동 (개발자) — `docs/guides-dev/sdk-integration.md` *(작성 예정)*
- 어댑터 작성 (개발자) — `docs/guides-dev/adapter-authoring.md` *(작성 예정)*

## 의도적으로 하지 않는 것

- 자체 추론 엔진을 새로 만들지 않습니다. 성숙한 OSS를 어댑터로 얹습니다.
- 브라우저-only 웹앱이 아닙니다. 데스크톱 프로그램입니다.
- 기존 웹앱을 뜯어고치지 않습니다. companion provider만 추가합니다.
- training을 v1 핵심으로 만들지 않습니다. 워크벤치는 placeholder.
- Ollama 단일 종속으로 만들지 않습니다. 어댑터 중 하나일 뿐입니다.
- 라이트 테마를 만들지 않습니다.

## 라이선스

추후 결정. (자체 코드는 MIT/Apache-2.0 dual 검토 중. 외부 OSS 라이선스 매트릭스는 `docs/oss-dependencies.md` 참조.)

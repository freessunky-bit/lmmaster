[한국어](./README.md) · [English](./README.en.md)

# LMmaster

[![CI](https://github.com/freessunky-bit/lmmaster/actions/workflows/ci.yml/badge.svg)](https://github.com/freessunky-bit/lmmaster/actions/workflows/ci.yml)
[![Release](https://github.com/freessunky-bit/lmmaster/actions/workflows/release.yml/badge.svg)](https://github.com/freessunky-bit/lmmaster/actions/workflows/release.yml)
[![License](https://img.shields.io/badge/license-MIT%20%7C%20Apache--2.0-blue.svg)](#라이선스)
[![GitHub Releases](https://img.shields.io/github/v/release/freessunky-bit/lmmaster?include_prerelease&label=release)](https://github.com/freessunky-bit/lmmaster/releases)

> 기존 HTML 웹앱이 **호출만 해서 사용할 수 있는** 로컬/하이브리드 AI 오케스트레이터 데스크톱 프로그램.
> 단순 런처가 아니에요. **Local AI Companion**이에요.

LMmaster는 사용자 PC에서 실행돼서, 기존 웹앱과 다른 웹앱들이 **로컬 HTTP API** 또는 **JS/TS SDK**만 호출해 LLM/STT/TTS 추론을 사용할 수 있게 만드는 데스크톱 프로그램이에요. 런타임의 설치·업데이트·헬스체크·라우팅·폴백·키 관리·하드웨어 적합도 추천을 LMmaster가 모두 책임져요.

## 6 pillar 약속

LMmaster는 v1에서 다음 6가지를 약속해요:

1. **자동 설치** — LM Studio / Ollama를 한국어 마법사로 한 번에 설치하고, 환경 변수까지 정리해요.
2. **한국어 1차** — UI · 문서 · 오류 메시지 전부 한국어 해요체. 영어가 필요하면 토글 한 번이면 돼요.
3. **포터블** — 같은 OS·아키텍처 계열에서 폴더 이동이 가능해요. 환경이 바뀌어도 자동 감지·복구.
4. **큐레이션 카탈로그** — `manifests/snapshot/models/`에 검수된 모델 매니페스트 + 한국어 설명.
5. **워크벤치** — Python sidecar로 LLM 평가 / 미세조정 placeholder. v1.x에서 본격 확장.
6. **자가 점검 + 자동 갱신** — 6시간 cron으로 환경/카탈로그 자동 점검. 외부 통신은 GitHub Releases만(opt-in 텔레메트리는 따로 토글).

## 핵심 특징

- **호출만 하면 끝** — 기존 웹앱은 `@lmmaster/sdk` 의존성 1개와 provider 1개만 추가하면 돼요.
- **하드웨어 점검 + deterministic 추천** — 사용자의 PC를 점검해 best/balanced/lightweight/fallback 4종을 결정적으로 추천해요.
- **다중 런타임** — llama.cpp · KoboldCpp · Ollama · LM Studio · vLLM을 어댑터로 통합. 어디에도 종속되지 않아요.
- **OpenAI-compatible 로컬 게이트웨이** — 기존 OpenAI SDK가 base URL만 바꾸면 동작해요.
- **API 키 + scope 모델** — 다른 웹앱도 키만 받아서 붙일 수 있어요.
- **Dark + 네온 그린** — 기존 웹앱과 동일 디자인 시스템 공유.

## 빠른 시작

### 사용자 (베타 다운로드)

> v1 정식 release 전이에요. [GitHub Releases](https://github.com/freessunky-bit/lmmaster/releases)에서 베타 빌드를 받아 보세요.

- **Windows**: `.exe`를 받아 실행해 주세요. SmartScreen 경고가 나오면 "추가 정보" → "실행"을 눌러 주세요. *(인증서 미발급 v1 단계 — 평판이 쌓이면 경고가 사라져요.)*
- **macOS**: `.dmg`를 받아 Applications로 옮겨 주세요. 첫 실행은 우클릭 → "열기"로 Gatekeeper를 통과해 주세요.
- **Linux**: `.AppImage`에 실행 권한을 준 뒤 실행해 주세요.
  ```bash
  chmod +x LMmaster_*.AppImage
  ./LMmaster_*.AppImage
  ```

### 개발자

> 사전 요구: Rust(stable), Node 20+, pnpm 9+, Tauri 사전요건(OS별 webview/system 라이브러리 — `https://tauri.app/start/prerequisites/`).

```bash
# 의존성 설치
pnpm install
cargo build --workspace

# 데스크톱 앱 개발 모드
pnpm --filter @lmmaster/desktop tauri dev

# 검증 (CLAUDE.md §3 정식 형식)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude lmmaster-desktop
pnpm exec tsc -b
```

## 사용자 결정 6건 (v1)

LMmaster는 사용자 동의 없이 외부 통신을 하지 않아요. 다음 6건은 모두 *사용자 명시 토글*로만 활성돼요.

1. **자가 점검 주기** — 끔 / 15분 / 1시간 (기본 1시간).
2. **자동 갱신** — GitHub Releases 6시간 cron (기본 OFF). 베타 채널 토글로 pre-release 받기.
3. **익명 사용 통계** — 기본 OFF. 활성 시 익명 UUID 발급, GlitchTip self-hosted endpoint(env var 미설정 시 비활성).
4. **Gemini 한국어 도우미** — 기본 OFF. 설치 가이드의 한국어 자연어 생성에만 사용 (판정/추천은 deterministic).
5. **포터블 export/import** — 명시 클릭으로만 trigger.
6. **로컬 API 키 발급** — 다른 웹앱이 LMmaster 게이트웨이를 호출할 때 사용하는 scope 키.

## 디렉터리 구조

```text
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

- 자체 추론 엔진을 새로 만들지 않아요. 성숙한 OSS를 어댑터로 얹어요.
- 브라우저-only 웹앱이 아니에요. 데스크톱 프로그램이에요.
- 기존 웹앱을 뜯어고치지 않아요. companion provider만 추가해요.
- training을 v1 핵심으로 만들지 않아요. 워크벤치는 placeholder.
- Ollama 단일 종속으로 만들지 않아요. 어댑터 중 하나일 뿐이에요.
- 라이트 테마를 만들지 않아요.
- Sentry SaaS 같은 외부 SaaS에 텔레메트리를 보내지 않아요. (ADR-0041 — GlitchTip self-hosted opt-in만)

## 기여하기

이슈와 PR을 환영해요. 자세한 가이드:

- [버그 신고 템플릿](.github/ISSUE_TEMPLATE/bug_report.md)
- [기능 제안 템플릿](.github/ISSUE_TEMPLATE/feature_request.md)
- [PR 체크리스트](.github/PULL_REQUEST_TEMPLATE.md)
- 릴리스 자동화 가이드 — [.github/SECRETS_SETUP.md](.github/SECRETS_SETUP.md)

## 라이선스

추후 결정. (자체 코드는 MIT/Apache-2.0 dual 검토 중. 외부 OSS 라이선스 매트릭스는 `docs/oss-dependencies.md` 참조.)

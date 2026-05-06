# Phase 22' — Trend Report 결정 노트 (큐레이션 콘텐츠 데이터셋 + 라이브 갱신)

> **작성일**: 2026-05-07
> **선행 문서**: `docs/adr/0060-trend-report.md`, `docs/research/phase-22p-trend-report-reinforcement.md`
> **트리거**: 사용자 요청 — AI 트렌드 메뉴 (모델 + 유튜브 + 뉴스 + 거물 SNS).
> **상태**: v2.0 진입 standby. 코드 0 — 사용자 명시 진입 신호 시 v2.0 sub-phase 5단계 진입.

---

## 1. 결정 요약

- **A1**: B 안 (큐레이션 트렌드 데이터셋 + 라이브 갱신) 채택. 사용자 PC 직접 scrape X.
- **A2**: trends-bundle 단일 합본 JSON, ADR-0044 catalog.json 패턴 정합. minisign 서명 (Phase 13'.g.3 sign-manifests 확장).
- **A3**: 데이터 소스 8종 (HF Daily Papers + arXiv RSS + AI 회사 RSS + 한국 미디어 RSS + YouTube Data API + Bluesky public + Mastodon/블로그 RSS + GitHub trending 자체 scrape). X(Twitter) / OpenRouter 거부.
- **A4**: 큐레이터 운영 — GHA 매일 cron → review queue Issue → 큐레이터 매주 월요일 PR. 부담 30~60분/주.
- **A5**: 사용자 측 — 4B+ 모델 게이트 + SQLite 캐시 (diff 요약) + tags 화이트리스트 + 해요체 톤 ≤ 80자.
- **A6**: 호스팅 — 별도 repo `lmmaster-trends-bundle` (정공). v1.x prototype은 `crates/trends-bundle-curator/` 가능.
- **A7**: registry-fetcher 코드 변경 0 — generic 인프라 그대로 활용.

ADR-0060 §1~7에 모두 명시. 본 결정 노트는 cross-reference + sub-phase 분할 + 위험 매트릭스 + 결정 포인트 7건.

---

## 2. 채택안 cross-reference

| 영역 | 결정 위치 |
|---|---|
| trends-bundle JSON schema | ADR-0060 §1, reinforcement §7 |
| 데이터 소스 매트릭스 | ADR-0060 §2, reinforcement §1~6 |
| 큐레이터 운영 흐름 | ADR-0060 §3, reinforcement §9 |
| 모델 게이트 + 요약 정책 | ADR-0060 §4, reinforcement §8 |
| 호스팅 (별도 repo) | ADR-0060 §5, reinforcement §9 |
| registry-fetcher generic | ADR-0060 §6 |
| EULA 갱신 + 인용 윤리 | ADR-0060 §7, reinforcement §3, §5 |

---

## 3. 기각안 + 이유 (negative space — 다음 세션 보호)

ADR-0060 "거부된 대안" 15건 중 핵심 6건만 결정 노트에 명시 (전체는 ADR 참조):

| 기각안 | 거부 이유 |
|---|---|
| **사용자 PC 직접 scrape (B 안 X)** | 외부 통신 0 정체성 위반 + ToS 위반 위험 |
| **X (Twitter) API / scrape** | $200/월 + ToS 위반. Bluesky + 본인 블로그 RSS로 대체 |
| **OpenRouter `openrouter.ai` 화이트리스트 추가** | 가치 대비 정체성 훼손 큼. `hugging_face_id` 신호 가치는 인정, v2.x 후순위 |
| **자동 LLM 요약 PR** | 큐레이션 정체성 와해 + 사실 오류 위험. 큐레이터 매주 1회 의도적 게이트 |
| **OpenAI/Anthropic API 텍스트 요약** | cloud-zero 정체성 위반. 로컬 LLM만 |
| **검색 도구 호출 (E 안)** | v2.x 옵트인 토글 검토. v2.0 진입 시점은 B만 |

---

## 4. 미정 / 후순위 이월

- **YouTube Data API key 정책** — 큐레이터 GHA secret으로. v2.0 진입 시 등록 결정.
- **Bluesky / Mastodon 큐레이션 대상 인사 화이트리스트** — Karpathy / LeCun / Hassabis / Lilian Weng / Sebastian Raschka / Chip Huyen / Simon Willison + 한국 인사 5~10명. v2.0 진입 시 큐레이터 결정.
- **한국 AI 채널 YouTube 5~10개 큐레이션** — v2.0 진입 시점 1주 모니터링 후.
- **모델 게이트 정확한 임계** — 4B+ 결정. 단 사용자 PC 사양 (RAM 8GB 미만) 시 *2B* 폴백 검토. v2.x.
- **사용자 관심사 필터 UI** — tags 화이트리스트 토글 디자인. v2.0 진입 시.
- **만료된 bundle 처리** — `expires_at` 지나면 "갱신 N일 전" 표시 vs 자동 hide. 정책 v2.0 진입 시.
- **C 안 (사용자 RSS 추가) v2.x** — v2.0 운영 1~2개월 후 사용자 신호 누적 후 검토.
- **E 안 (검색 도구 호출) v2.x+** — Brave Search / DuckDuckGo Lite 옵트인. EULA 갱신 별개.

---

## 5. 테스트 invariant

ADR-0060 §테스트 invariant 8건:

1. trends-bundle 무결성 (schema_version 1, expires_at 7일, kind enum 6종).
2. minisign verify (registry-fetcher 호출 측).
3. 모델 게이트 (4B+ 미설치 시 메뉴 disabled + 한국어 hint).
4. 캐시 diff (bundle 갱신 시 신규 item만 LLM 요약).
5. 사용자 관심사 필터 (tags 화이트리스트 deterministic).
6. 만료 표시 (`expires_at` 후 "갱신 N일 전" UI).
7. 로컬 LLM 요약 톤 (해요체 + ≤ 80자 + 출처 링크).
8. fair use 본문 길이 (item summary_ko ≤ 200자).

추가 invariant (curator 측, 별도 repo):

9. **Issue dedupe** — 동일 source_url 재발견 시 issue 1개 (JasonEtco/create-an-issue update_existing).
10. **kind 분류 정확** — RSS feed parse → kind 자동 매핑 (paper/blog/news/video/sns/github).

---

## 6. 다음 페이즈 인계 — sub-phase 분할

### 진입 조건 (v2.0 메이저 분기)
- v0.0.x ship + v1.x 안정화 종료.
- Phase 21' Trending Watcher 운영 1~2개월 경험 (큐레이션 흐름 검증).
- 사용자 명시 진입 신호 (v2.0 분기).
- ADR-0060 + 본 결정 노트 사용자 명시 승인.
- ADR-0060 §결정 포인트 7건 답변.

### sub-phase 5단계

| Phase | 제목 | 의존성 | DoD |
|---|---|---|---|
| **22'.a** | trends-bundle schema + minisign 통합 | ADR-0060 | `manifests/trends/bundle.json` + sign-manifests workflow에 추가 + tagged enum 6종 + 8 invariant |
| **22'.b** | 큐레이터 GHA — fetch + review queue | 22'.a | RSS / HF Daily Papers / arXiv / YouTube / Bluesky / GitHub trending fetch + JasonEtco/create-an-issue dedupe |
| **22'.c** | registry-fetcher 호출 측 + cache | 22'.b | desktop의 `fetch_trends_bundle()` IPC + SQLite diff cache + 7 invariant |
| **22'.d** | 모델 게이트 + UI | 22'.c | `apps/desktop/src/pages/Trends.tsx` + 4B+ 모델 게이트 + 관심사 필터 + 해요체 LLM 요약 + i18n ko/en |
| **22'.e** | 운영 모니터링 + 큐레이터 가이드 | 22'.d | `docs/CURATION_GUIDE.md` 갱신 + 1주 운영 + 큐레이터 부담 측정 |

### 위험 매트릭스

| 위험 | 영향 | 완화 |
|---|---|---|
| 큐레이터 운영 부담 (매주 30~60분) | 콘텐츠 stale 위험 | 자동 GHA review queue + LLM-assist draft (v2.x 후속 옵션) |
| RSS feed 부재 매체 (Anthropic 등) | 데이터 누락 | HTML diff (CSS selector) 보조 + ETag 폴링 |
| 거물 SNS 인용 윤리 | ToS 위반 위험 | X 거부 + Bluesky/Mastodon/블로그 RSS 1순위 + fair use 1~2문장 |
| YouTube Data API key 노출 | quota 무단 사용 | GHA secrets 분리 + 별도 repo |
| jsdelivr propagate 24h | 사용자 갱신 지연 | GitHub Releases tier (2순위) 즉시 fetch + ETag |
| 한국 AI 미디어 콘텐츠 부족 | 한국어 사용자 가치 ↓ | 영문 매체의 한국어 요약 + 큐레이터 손길 |
| 로컬 LLM 4B 한국어 자연스러움 | 사용자 첫 인상 망가짐 | 모델 게이트 + EXAONE/HCX 1순위 권장 |
| SQLite 캐시 무한 증식 | 디스크 사용 ↑ | 만료 bundle 자동 GC (TTL 30일) |

### 다음 standby (v2.0 진입 후)
- Phase 22'.a 진입 — trends-bundle schema 정의 + sign-manifests workflow 확장.

### 검증 명령 (v2.0 진입 시)
```powershell
.\.claude\scripts\verify.ps1
# 추가:
# - cargo test --workspace --package desktop-trend-cache
# - pnpm exec vitest run apps/desktop/src/pages/Trends.test.tsx
```

---

## 7. 결정 포인트 7건 (사용자 결정 필요 — v2.0 진입 시)

reinforcement 노트 §10에 정리된 7건:

| # | 포인트 | 권장 (현재) |
|---|---|---|
| 1 | rss.arxiv.org 화이트리스트 확장 | GHA 측만 fetch, 사용자 PC는 trends-bundle만 (현행 정체성 보존) |
| 2 | X (Twitter) API/scrape | **거부 확정** |
| 3 | 별도 repo vs 본 repo prototype | 별도 repo 정공 (v1.x prototype 후 분리) |
| 4 | 큐레이터 운영 부담 (30~60분/주) | 합의 가능 → 진행, 불가 시 LLM-assist draft 옵션 검토 |
| 5 | trends-bundle schema kind enum 6종 | 채택 (paper/blog/news/video/github/sns) |
| 6 | 모델 게이트 — 4B+ 모델 4종 | 채택 (Gemma 3 4B, Nemotron 3 Nano 4B, EXAONE 3.5 7.8B, HCX-SEED 8B) |
| 7 | 한국 매체 1순위 | THE AI + AI타임스 (RSS 안정) |

---

## 출처 (보강 리서치 노트 §출처 참조)

`docs/research/phase-22p-trend-report-reinforcement.md` §출처에 19개 링크 보존.

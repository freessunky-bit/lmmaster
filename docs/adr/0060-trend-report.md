# ADR-0060 — Trend Report (큐레이션 콘텐츠 데이터셋 + 라이브 갱신)

* **상태**: Proposed (2026-05-07). v2.0 standby — Phase 22' 풀 설계 단계.
* **선행**:
  - ADR-0014 (Curated Model Registry) — 큐레이션 정체성 thesis.
  - ADR-0026 (외부 통신 화이트리스트 — `huggingface.co` + `github.com`).
  - ADR-0044 (Live Catalog Refresh — bundle JSON + jsdelivr + hot-swap).
  - ADR-0047 (Catalog minisign Ed25519) + Phase 13'.g.3 (manifest signature 확장).
  - ADR-0059 (Phase 21' Trending Watcher — 별도 repo + 큐레이터 review queue).
* **결정 노트**: `docs/research/phase-22p-trend-report-decision.md`
* **보강 리서치**: `docs/research/phase-22p-trend-report-reinforcement.md` (9 영역 엘리트 사례 + 7 결정 포인트, commit 03225c4).

## 컨텍스트

사용자 요청 (2026-05-06): AI 모델/유튜브/뉴스/거물 SNS 동향 메뉴. 4B+ 모델 설치 시 활성화 + 메뉴 진입 시 로컬 LLM이 fresh 뉴스페이퍼 형태로 한국어 요약. *목적은 LMmaster 안에서 최신 AI 트렌드/활용 예시/업계 동향 파악*.

핵심 충돌:
- 외부 통신 0 (ADR-0013) ↔ 트렌드 = *외부 콘텐츠 fetch* 필요.
- cloud-zero ↔ "fresh"하려면 어떤 형태든 외부 데이터.

화해 (사용자 결정 옵션 A 채택, 2026-05-06):
- **B 안 (큐레이션 데이터셋 + 라이브 갱신)** — 사용자 PC 직접 scrape X. 큐레이터(우리) GHA가 RSS+arXiv+YouTube fetch → 큐레이터 review queue → 큐레이터 검토 후 *trends-bundle* push → jsdelivr propagate → 사용자 fetch (registry-fetcher 4-tier fallback) → 로컬 LLM 요약 정렬.
- 사용자 PC 외부 통신 화이트리스트 변경 0 — `huggingface.co` + `github.com` 보존.

## 결정

### 1. trends-bundle JSON 단일 합본 (ADR-0044 패턴 정합)

```
manifests/trends/bundle.json (LMmaster 본 repo 또는 별도 repo)
  → minisign 서명 (Phase 13'.g.3 확장)
  → jsdelivr / GitHub Releases / Bundled 4-tier fallback
  → registry-fetcher::fetch("trends-bundle")
  → 사용자 PC 검증 + cache
```

schema (보강 리서치 §7):

```json
{
  "schema_version": 1,
  "generated_at": "2026-05-07T00:00:00Z",
  "expires_at": "2026-05-14T00:00:00Z",
  "curator_note_ko": "이번 주 핵심 흐름은 ...",
  "items": [
    {
      "id": "hf-paper-2024-arxiv-2405.12345",
      "kind": "paper",
      "title": "Title (원문)",
      "summary_ko": "한 줄 한국어 요약 (해요체).",
      "source": "huggingface-daily-papers",
      "source_url": "https://arxiv.org/abs/2405.12345",
      "attribution": "AK on HuggingFace",
      "published_at": "2026-05-05T12:00:00Z",
      "tags": ["llm", "korean", "reasoning"],
      "score": 0.84
    }
  ]
}
```

`kind`: tagged enum `#[serde(tag = "kind", rename_all = "kebab-case")]` 6종 — `paper` / `blog` / `news` / `video` / `github` / `sns`.

### 2. 데이터 소스 매트릭스

| Source | 용도 | 호출 측 | 라이선스 |
|---|---|---|---|
| HF Daily Papers API | 학술 트렌드 (BEST) | 큐레이터 GHA | 인증 0, 5분 윈도우 |
| arXiv RSS (cs.LG/CL/AI/CV) | 학술 보조 | 큐레이터 GHA | 3 req/sec, UA 필수 |
| OpenAI / TechCrunch / The Verge / VentureBeat / NVIDIA Blog RSS | AI 회사·미디어 | 큐레이터 GHA | RSS 표준 |
| THE AI / AI타임스 RSS | 한국 미디어 | 큐레이터 GHA | RSS 표준 |
| YouTube Data API v3 | AI 채널 영상 | 큐레이터 GHA (API key) | 무료 10k unit/day |
| Bluesky `public.api.bsky.app` | 거물 SNS (대체) | 큐레이터 GHA | 인증 0, 관대 limit |
| Mastodon RSS / 본인 블로그 RSS | 거물 SNS (1순위) | 큐레이터 GHA | RSS 표준 |
| GitHub Trending (자체 scrape) | OSS 도구 트렌드 | 큐레이터 GHA | github.com 화이트리스트 |

**X (Twitter) 거부** — API $200/월 + ToS 위반 위험. Bluesky + Mastodon + 본인 블로그로 충분.
**OpenRouter 거부** — `openrouter.ai` 화이트리스트 추가 부담. 가치 대비 ROI 낮음.

### 3. 큐레이터 운영 흐름

```
[매일 00:00 UTC]
GHA cron → fetch 30+ source → review queue Issue 자동 (JasonEtco/create-an-issue)
  ↓
[매주 월요일 09:00 KST]
큐레이터 → queue 100~200 item 중 12~30 선별 → 한국어 1~2문장 요약 작성
  ↓
PR `manifests/trends/bundle.json`
  ↓
sign-manifests workflow 자동 minisign 서명
  ↓
jsdelivr propagate (1~24h)
  ↓
[사용자 PC] 메뉴 진입 시 registry-fetcher::fetch("trends-bundle") + 로컬 LLM 요약 정렬
```

큐레이터 운영 부담: 매주 월요일 30~60분 (선별 + 요약).

### 4. 사용자 측 — 모델 게이팅 + 요약 정렬

**모델 게이트** (4B+ instruction-tuned + 한국어 출력):
- 활성 조건: Gemma 3 4B / Nemotron 3 Nano 4B / EXAONE 3.5 7.8B / HCX-SEED 8B 중 *1개 이상* 설치 + 로드 가능.
- 미충족 시: 메뉴 disabled + hint chip "동향 메뉴는 4B+ 모델이 필요해요. EXAONE 3.5 등 설치 후 다시 와요."

**요약 정책**:
- bundle 전체 매번 요약 X. SQLite 캐시 (bundle.id + bundle.generated_at 기준).
- bundle 갱신 시 신규 item만 요약 (diff).
- 사용자 관심사 필터: `tags` 화이트리스트 (예: `[korean, agent, llm]`).
- system prompt 톤 제약: "해요체. 1~2문장. 출처 링크 보존. 영어 단어는 첫 등장 시 한국어 풀이." 출력 ≤ 80자/item, 화면당 ≤ 12 item.

### 5. 호스팅 — 별도 repo `lmmaster-trends-bundle`

Phase 21' Trending Watcher와 동일 패턴. LMmaster 본 repo는 PR + minisign 받는 측. secrets 분리 (`YOUTUBE_API_KEY` 등).

**v1.x prototype 예외 (Phase 21' 정책 준용)**:
- v1.x 검증 단계는 본 repo 안 prototype 가능 (`crates/trends-bundle-curator/` 또는 별도). 검증 후 별도 repo 분리 = v2.x.

### 6. registry-fetcher generic 활용 — 코드 변경 0

ADR-0044 + Phase 13'.g.3 인프라가 *이미 generic*:
- `signature_url_for("trends-bundle", tier)` — manifest_id 기반.
- `fetch_signature_text(url, timeout)` — generic .minisig fetch.
- `SignatureVerifier::verify` — 매니페스트 종류 무관.
- `mark_signature_verified(source, manifest_id)` — generic 마킹.

→ trends-bundle 통합은 *호출 측 추가*만 필요. registry-fetcher 코드 변경 0.

### 7. EULA 갱신 + 인용 윤리

EULA에 명시 추가 (v2.0 진입 시):
- *큐레이션 트렌드 데이터*가 사용자 PC에서 fetch됨.
- 출처 표기 (attribution) 보존 — fair use 원칙.
- 거물 SNS 인용은 1~2문장 + 원문 링크 (fair use 한도). X(Twitter) 거부.

## 근거

- **ADR-0044 인프라 재활용**: bundle JSON + jsdelivr + hot-swap + minisign이 이미 production. 추가 개발 비용 ↓.
- **HF Daily Papers + arXiv RSS BEST**: 학술 트렌드 자동화 100% 가능. AK 큐레이션이 골든 시그널.
- **YouTube Data API v3 무료 한도**: 10 채널 × 매일 폴링 = 20 units / 10k 한도 = 500배 마진.
- **X 거부 + Bluesky 대체**: ToS + 비용 위험 회피 + 거물 인사 일부가 Bluesky 마이그레이션 중.
- **별도 repo 정공**: secrets 분리 + GHA 빈도 차이 + 본 repo CI 부담 0.
- **deterministic 큐레이터 + 사람 검토**: ADR-0014 thesis 그대로 — chat template 깨짐 / 라이선스 함정 / 한국어 자연스러움 보장.

## 거부된 대안

1. **사용자 PC 직접 scrape (B 안 X)** — 외부 통신 0 정체성 위반 + ToS 위반 위험.
2. **OpenRouter `openrouter.ai` 화이트리스트 추가** — 가치 대비 정체성 훼손. `hugging_face_id` 신호 가치는 인정, v2.x 후순위.
3. **X (Twitter) API / scrape** — $200/월 + ToS 위반.
4. **LinkedIn scrape** — ToS 위반.
5. **SerpAPI / Tavily / Brave Search SaaS** — cloud-zero 정체성 위반.
6. **검색 도구 호출 (E 안)** — v2.x 옵트인 토글로 검토. v2.0 진입 시점은 B만.
7. **사용자 RSS 추가 (C 안)** — v2.x 후순위. 우선 큐레이션 데이터셋이 운영 안정화 후 옵션 추가.
8. **자동 LLM 요약 PR** — 큐레이션 정체성 와해 + 사실 오류 위험. 큐레이터 손길 매주 1회 의도적 게이트.
9. **OpenAI/Anthropic API 텍스트 요약** — cloud-zero 정체성 위반. 로컬 LLM만.
10. **메뉴 *모델 미설치 시 LLM 다운로드 권유*** — 사용자 자율성 침범. *menu disabled + hint chip*이 정합 (NSFW 토글 패턴 같이).
11. **Bundle 만료 시 stale 데이터 강제 표시** — 사용자가 *언제 갱신됐는지* 혼란. `expires_at` 7일 + UI에 "갱신 N일 전" 명시.
12. **기존 카탈로그 entries에 trend score 직접 통합** — schema 오염. trends-bundle은 *별도 자료* (모델 카탈로그 ≠ 콘텐츠 트렌드).
13. **본 repo curator GHA에서 직접 trigger** — secrets 노출 + 본 repo CI 부담. 별도 repo 정합.
14. **Bluesky `searchPosts` 기능 활용** — 인증 필요 + 공격 표면 ↑. `getAuthorFeed` (인증 0) + 큐레이터 handle 화이트리스트가 정합.
15. **GitHub trending 자동 PR** — OSS 라이선스 자동 검증 미흡 + 큐레이션 가치 낮음. 큐레이터 review가 정합.

## 결과 / 영향

### 신규 산출물 (별도 repo 또는 v1.x prototype)
- `lmmaster-trends-bundle/` (별도 repo, public, MIT) 또는 `crates/trends-bundle-curator/` (v1.x prototype):
  - `src/` — Rust binary. fetch (RSS / HF Daily Papers / arXiv / YouTube / Bluesky / GitHub trending) + Issue 생성.
  - `.github/workflows/curator.yml` — 매일 cron + JasonEtco/create-an-issue.
  - `tests/` — deterministic invariant 테스트.
  - `manifests/trends/bundle.json` — 큐레이터 PR 머지 후 자동 minisign 서명.

### LMmaster 본 repo 영향 (사용자 측)
- `crates/desktop-trend-menu/` (또는 `apps/desktop/src/pages/Trends.tsx` 신규) — UI.
- `crates/desktop-trend-cache/` — SQLite 캐시 + 로컬 LLM 요약.
- `apps/desktop/src/i18n/{ko,en}.json` — 메뉴 라벨 + 모델 게이트 카피.
- `crates/registry-fetcher` — 호출 측 추가만 (manifest_id "trends-bundle" 등록).

### EULA 갱신
- "큐레이션 트렌드 데이터" 명시 — fetch 흐름 + fair use + 거물 인용 윤리.

### 라이선스
- 모든 신규 코드: MIT 또는 Apache-2.0 dual.
- trends-bundle 콘텐츠: CC BY 4.0 (출처 표기 필수, 사용자 자유 활용).

## 테스트 invariant (v2.0 진입 시)

1. **trends-bundle 무결성** — schema_version 1, expires_at 7일, kind enum 6종.
2. **minisign verify** — registry-fetcher 호출 측 verify 통과.
3. **모델 게이트** — 4B+ 모델 미설치 시 메뉴 disabled + 한국어 hint.
4. **캐시 diff** — bundle 갱신 시 신규 item만 LLM 요약.
5. **사용자 관심사 필터** — tags 화이트리스트 deterministic.
6. **만료 표시** — `expires_at` 지나면 "갱신 N일 전" UI.
7. **로컬 LLM 요약 톤** — 해요체 + ≤ 80자 + 출처 링크 보존.
8. **fair use 본문 길이** — item summary_ko ≤ 200자 (full article 저장 X).

## 다음 단계

1. **결정 노트 6-section** + 본 ADR과 짝 + sub-phase 분할 (22'.a~e).
2. **v0.0.x ship 후 v1.x 안정화 종료 + 사용자 명시 진입 신호 시 진입**.
3. **v2.0 진입 시점 사용자 결정 7건 재확인** (보강 리서치 §결정 포인트).
4. **별도 repo 신설 vs v1.x prototype** — Phase 21' 운영 1~2개월 경험 보고 결정.

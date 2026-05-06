# Phase 22' — Trend Report 보강 리서치 (엘리트 사례 종합)

> **목적**: AI 모델/유튜브/뉴스/거물 SNS 동향 메뉴. 4B+ 모델 설치 시 활성화 + 메뉴 진입 시 로컬 LLM이 fresh 뉴스페이퍼 생성.
> **작성일**: 2026-05-06
> **선행**: ADR-0044 라이브 갱신 + Phase 13'.g.3 minisign 확장 + Phase 21' Trending Watcher 큐레이션 운영 패턴.
> **상태**: **v2.0 standby** — 본 페이즈는 v0.0.x ship + v1.x 안정화 후 진입. ADR-0060 + 결정 노트는 진입 시점에 작성.
> **정책**: B 안 (큐레이션 + jsdelivr propagate). 사용자 PC 직접 scrape X. 외부 통신 화이트리스트 보존.

---

## 1. AI 회사 공식 블로그 RSS — 1순위 신호 (확실히 가능)

**확정 가용**:
- **OpenAI**: `https://openai.com/news/rss.xml` (공식, 갱신 1~3건/주).
- **TechCrunch AI**: `https://techcrunch.com/category/artificial-intelligence/feed/`.
- **The Verge**: `https://www.theverge.com/rss/index.xml` (전체).
- **VentureBeat AI**: `https://venturebeat.com/category/ai/feed/`.
- **NVIDIA Blog**: `https://blogs.nvidia.com/feed/` (관행 패턴, GTC 시즌 트래픽 폭증).

**부분 가용 (피드 검증 필요)**:
- **Anthropic**: `anthropic.com/news` 페이지만 확인. RSS 직접 노출 X — 큐레이터가 페이지 diff scrape (GHA runner 측, 사용자 PC X) 또는 RSS-Bridge 사용.
- **Google DeepMind**: `deepmind.google/discover/blog` — RSS 명시 부재. Bridge 또는 sitemap 폴링.
- **Meta AI / Microsoft Research / Mistral / Cohere / xAI / Stability**: RSS 직접 URL 미확인. 큐레이터 GHA에서 페이지 ETag 폴링이 현실적.

**한국어 — 결론적 빈자리**:
- Naver Cloud / Kakao / LG AI Research / SKT / KT — *공식 한국어 RSS 명시 미확인*. 실무 권장: **THE AI** (`newstheai.com/rssIndex.html`) + **AI타임스** (`aitimes.com`) 일반 뉴스 피드.

**LMmaster 적용**: 큐레이터(GHA) 측에서 `feed-rs` (Rust) 또는 `feedparser` (Python)로 ETag 기반 폴링. RSS 직접 부재 매체는 **HTML diff (CSS selector)** 보조 — 모두 GHA runner 측 수행, 사용자 PC는 trends-bundle만 fetch.

---

## 2. 학술 / arXiv — 1순위 자동화 (BEST)

**arXiv 공식 RSS** (재구현 2024-01-31 이후 안정):
- `https://rss.arxiv.org/rss/cs.LG` + `/atom/cs.LG` — 매일 EST 자정 갱신, 다중 카테고리 결합 `cs.LG+cs.CL+cs.AI+cs.CV` (최대 2000 결과). User-Agent 필수, **3 req/sec rate limit**.
- API 검색: `http://export.arxiv.org/api/query?search_query=cat:cs.LG&sortBy=submittedDate&sortOrder=descending`.

**HuggingFace Daily Papers**:
- 공식 JSON API: `https://huggingface.co/api/daily_papers?date=YYYY-MM-DD&page=1&limit=100` (인증 불필요, CDN 캐시). AK 큐레이션 — *학술 핫리스트의 골든 시그널*.
- 비공식 RSS 미러: `papers.takara.ai/api/feed` 또는 `huangboming/huggingface-daily-paper-feed` (GHA + raw github URL).

**LMmaster 적용**: 큐레이터 GHA가 매일 `huggingface.co/api/daily_papers` (이미 화이트리스트) + `rss.arxiv.org/atom/cs.LG+cs.CL` fetch → 인용수/댓글수 정렬 → top 10 + 한국어 1줄 요약 + 출처 URL을 trends-bundle에 push.

---

## 3. AI 뉴스 미디어 — fair use 패턴

**해외 BEST**: TechCrunch / The Verge / VentureBeat / Ars Technica — RSS 안정. *제목 + 1~2문장 요약 + 출처 URL*은 fair use 표준 패턴.

**한국 BEST 1~2**:
- **THE AI** (`newstheai.com/rssIndex.html`) — 조선미디어그룹, RSS 명시.
- **AI타임스** (`aitimes.com`) — RSS 직접 URL 미확인이나 게시판 형식이라 ETag 폴링 가능.

**LMmaster 적용**: trends-bundle item = `{title, source, excerpt_ko, url, published_at}`. **본문 저장 X / 큐레이터 손길로 한국어 요약 1~2문장만**. 매체 표기 + 직접 링크는 fair use 안전.

---

## 4. YouTube AI 채널 — Data API v3 무료 한도 BEST

**할당량 산수** (10k unit/day 기준):
- `playlistItems.list` (uploads playlist) = 1 unit/req.
- `videos.list` (batch up to 50 IDs) = 1 unit/req.
- 10 채널 × `playlistItems.list` (1) + 10개 batched `videos.list` (1) = **20 units/일**. 여유 500배.
- *주의*: `search.list` = 100 unit. **search 사용 금지** — 채널 ID 직접 보유 + uploads playlist만 폴링.

**확정 채널 ID** (영문):
- `@3blue1brown` / `@YannicKilcher` / `@TwoMinutePapers` / `@LexFridman` / `@AIExplained` / Andrej Karpathy / Sebastian Raschka / Computerphile.

**한국 채널**: 큐레이터가 운영 1개월 내 5~10개 한국 AI 채널 직접 큐레이션 권장 (모두의연구소, 노가다코딩 등 후보).

**LMmaster 적용**: 큐레이터 GHA secrets에 `YOUTUBE_API_KEY` 1개. 매일 1회 cron으로 channel uploads playlist에서 24h 신규 영상 추출 → 제목/설명/published_at + 한국어 1줄 요약을 trends-bundle에 push. **사용자 PC는 YouTube API 호출 0**.

---

## 5. 거물 SNS — 인용 윤리 + 대체 채널 BEST

**X (Twitter)** — *매우 부정적*:
- 2026-02-06부터 신규 가입 pay-per-use. 기존 Basic은 $200/월. ToS scrape 금지.
- **결정 권장**: X scrape / API 모두 **거부**.

**Bluesky AT Protocol — BEST 대체**:
- `https://public.api.bsky.app` — 인증 0, rate limit 관대, 캐시 적용. `app.bsky.feed.getAuthorFeed` read-only.
- 단, `searchPosts`는 인증 필요. **큐레이터가 핵심 인사 handle 30~50개 직접 보유 + getAuthorFeed로만 폴링**.

**Mastodon** — BEST 보조:
- 사용자별 RSS 자동 — `https://{instance}/users/{user}.rss` (Karpathy / LeCun 일부 인사).

**개인 블로그 RSS** (가장 안전):
- **Karpathy**: `karpathy.bearblog.dev` (2025-03 시작) + `karpathy.github.io` (구).
- **Sebastian Raschka**: `sebastianraschka.com/blog`.
- **Lilian Weng / Chip Huyen / Simon Willison**: 모두 개인 블로그 RSS.

**LMmaster 적용**: trends-bundle SNS 항목 우선순위 = (a) 본인 블로그 RSS → (b) Bluesky public.api.bsky.app authorFeed → (c) Mastodon RSS. **X 인용 거부**. 큐레이터 검토 후 *1~2문장 인용 + 출처 URL*만.

---

## 6. GitHub Trending — AI 분야

**공식 API 부재** 확정. 현실 옵션:
- `huchenme/github-trending-api` — Node.js, 무료, 호스팅 종료 위험.
- `antonkomarev/github-trending-api` — Rust 자체 호스팅 (LMmaster 친화).
- **자체 scrape**: 큐레이터 GHA에서 `https://github.com/trending/python?since=daily` HTML 직접 파싱.

**LMmaster 적용**: 큐레이터 GHA가 자체 scrape (Rust `scraper` crate) → 라이선스 화이트리스트 (Apache-2 / MIT / BSD) + 토픽 필터 (`llm`, `ai`, `agent`, `ml`) → 상위 10 → `{repo, stars_today, summary_ko, license, url}` bundle. **외부 의존 0**.

---

## 7. trends-bundle JSON Schema — Phase 13'.g.3 minisign 호환 안

```json
{
  "$schema_hint": "lmmaster trends-bundle schema_version=1. Auto-generated by curator GHA + manual review.",
  "schema_version": 1,
  "generated_at": "2026-05-06T00:00:00Z",
  "expires_at": "2026-05-13T00:00:00Z",
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

- `kind`: tagged enum `#[serde(tag = "kind", rename_all = "kebab-case")]` — `paper` / `blog` / `news` / `video` / `github` / `sns`.
- **minisign 서명**: 기존 Phase 13'.g.3 sign-manifests workflow에 `manifests/trends/bundle.json` 추가 → `bundle.json.minisig` 자동 생성. 동일 keypair 재사용.
- **registry-fetcher generic 활용**: `signature_url_for("trends-bundle", tier)` 호출만 추가하면 4-tier fallback (jsdelivr → github → bundled) + minisign verify 자동 적용. **registry-fetcher 코드 변경 0** — 호출 측만.
- **expires_at**: 7일 — 그 후 사용자 UI는 stale 표시 + 큐레이터 push가 늦어진 신호.

---

## 8. 로컬 LLM 요약 정렬 — 모델 게이팅 + 캐시 전략

**모델 가용성 게이트**:
- 활성 조건: Gemma 3 4B / Nemotron 3 Nano 4B / EXAONE 3.5 7.8B / HCX-SEED 8B 중 *1개 이상* 설치 + 로드 가능.
- 미충족 시: 메뉴 disabled + hint chip "동향 메뉴는 4B+ 모델이 필요해요. EXAONE 3.5 등 설치 후 다시 와요."

**요약 정책**:
- *bundle 전체 매번 요약 X*. 캐시: bundle.id + bundle.generated_at 기준 LLM 요약 결과 SQLite 캐시. bundle 갱신 시 신규 item만 요약 (diff).
- 사용자 관심사 필터: `tags` 화이트리스트 (예: `[korean, agent, llm]`만) → deterministic 필터 후 LLM은 요약 정렬만.
- **톤 제약 (system prompt)**: "해요체. 1~2문장. 출처 링크 보존. 영어 단어는 첫 등장 시 한국어 풀이."
- 출력 길이: item당 ≤ 80자 한국어 요약. 화면당 ≤ 12 item.

---

## 9. 자동화 호스팅 + 운영 모델

**별도 repo 권장** (Phase 21' Trending Watcher와 동일 패턴):
- `lmmaster-trends-bundle` (public, MIT). LMmaster 본 repo는 **PR 받는 측**.
- 별도 repo 이유: secrets 분리 (`YOUTUBE_API_KEY`, RSS 폴링용 User-Agent) + GHA 빈도 차이 + 본 repo CI 부담 0.

**GHA cron 스케줄**:
- **매일 00:00 UTC**: arXiv + HF Daily Papers + RSS 30 source fetch → review queue Issue 자동 생성 (`JasonEtco/create-an-issue`).
- **매주 월요일 09:00 KST**: 큐레이터가 review queue 검토 → 한국어 요약 작성 → `manifests/trends/bundle.json` PR + sign workflow 자동 minisign.
- *GHA cron drift 알려진 문제*: cron 실패 알림이 issue로 자동 생성되도록 헤일프루프 워크플로 추가 권장.

**큐레이터 운영 부담 추정**: 매주 월요일 30~60분 (queue 100~200 item 중 12~30 선별 + 한국어 요약). v1.x에 LLM-assist 큐레이터 도구 검토 가능.

---

## 결정 포인트 7개 (v2.0 진입 시 사용자 결정 필요)

1. **DECISION-1 (화이트리스트 확장)**: `rss.arxiv.org` + (필요 시) `huggingface.co/api/daily_papers`(이미 허용). *사용자 PC 측 추가 0*. 큐레이터 GHA만 RSS 외부 fetch 수행. **권장: GHA 측만, 사용자는 trends-bundle만 fetch**.

2. **DECISION-2 (X 거부 확정)**: API $200/월 + ToS 위반 위험 → **거부**. SNS 채널은 *Bluesky public.api.bsky.app + Mastodon RSS + 본인 블로그 RSS* 3가지로 충분.

3. **DECISION-3 (별도 repo vs 본 repo 안)**: Phase 21' Trending Watcher와 동일 패턴 *별도 repo `lmmaster-trends-bundle`* 권장. 본 repo는 PR + minisign 받는 측. **사용자 명시 승인 필요** (큰 아키텍처 분기, CLAUDE.md §1).

4. **DECISION-4 (큐레이터 운영 부담)**: 매주 30~60분 합의 가능 여부. 합의 불가 시 → **B+ 안 (자동 review queue + 큐레이터 LLM-assist draft)** 검토.

5. **DECISION-5 (trends-bundle schema_version 1 → tagged enum 6 kinds)**: `paper` / `blog` / `news` / `video` / `github` / `sns` — `BenchErrorReport` 패턴(serde tag) 그대로 적용.

6. **DECISION-6 (모델 게이트 정확한 모델 목록)**: Gemma 3 4B + Nemotron 3 Nano 4B + EXAONE 3.5 7.8B + HCX-SEED 8B 4개 명시. *3.x 이하 / 다국어 미지원 모델은 disable*.

7. **DECISION-7 (한국 매체 1순위)**: THE AI (`newstheai.com`) + AI타임스 (`aitimes.com`) 2종 1순위 한국어 미디어. 한국어 자체 콘텐츠 부족 시 영문 매체의 한국어 큐레이션 요약으로 충당.

---

## 핵심 참조 파일 (LMmaster 본 repo)

- `crates/registry-fetcher/src/fetcher.rs` — 4-tier fallback + minisign verify (generic, 호출 측만 추가하면 trends-bundle 즉시 호환).
- `crates/registry-fetcher/src/source.rs` — Vendor/Github/Jsdelivr/Bundled tier 정의.
- `docs/research/phase-21p-trending-watcher-decision.md` — 별도 repo + GHA cron + 큐레이터 흐름 청사진.
- `docs/research/phase-13pa-live-catalog-decision.md` — bundle JSON + jsdelivr + hot-swap 패턴.
- `docs/research/phase-13pg3-manifest-signature-expansion-decision.md` — sign-manifests workflow 확장 청사진.

---

## v2.0 진입 시 작성할 추가 문서 (현재 미작성)

- `docs/adr/0060-trend-report.md` — 정식 ADR.
- `docs/research/phase-22p-trend-report-decision.md` — 6-section 결정 노트 + 18+ 기각안 + sub-phase 분할.
- 사용자 결정 7건 (위 §결정 포인트) 답변 후 진입 가능.

---

## 출처

- [arXiv RSS Feeds](https://info.arxiv.org/help/rss.html)
- [arXiv API User Manual (rate limits)](https://info.arxiv.org/help/api/user-manual.html)
- [HuggingFace Daily Papers API](https://huggingface.co/api/daily_papers)
- [YouTube Data API quota cost](https://developers.google.com/youtube/v3/determine_quota_cost)
- [X API pricing 2026](https://docs.x.com/x-api/getting-started/pricing)
- [Bluesky API Hosts and Auth](https://docs.bsky.app/docs/advanced-guides/api-directory)
- [Bluesky rate limits](https://docs.bsky.app/docs/advanced-guides/rate-limits)
- [Mastodon RSS feeds](https://openrss.org/blog/mastodon-rss-feeds)
- [Karpathy bear blog (RSS)](https://karpathy.bearblog.dev/)
- [GitHub trending API community discussion](https://github.com/orgs/community/discussions/161519)
- [antonkomarev github-trending-api Rust](https://github.com/antonkomarev/github-trending-api)
- [TechCrunch AI category](https://techcrunch.com/category/artificial-intelligence/)
- [The Verge RSS](https://www.theverge.com/rss/index.xml)
- [VentureBeat AI feed](https://venturebeat.com/category/ai/feed/)
- [OpenAI News](https://openai.com/news/)
- [DeepMind blog](https://deepmind.google/blog/)
- [THE AI RSS index](http://www.newstheai.com/rssIndex.html)
- [AI타임스](https://www.aitimes.com/)
- [GitHub Actions cron reliability discussion](https://github.com/orgs/community/discussions/194300)

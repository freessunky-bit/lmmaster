# Phase 22' — Trend Report 보강 리서치 (엘리트 사례 종합 / 본격 풀 설계 ed.)

> **목적**: AI 모델/유튜브/뉴스/거물 SNS 동향 메뉴. 4B+ 모델 설치 시 활성화 + 메뉴 진입 시 로컬 LLM이 fresh 뉴스페이퍼를 한국어 해요체로 생성. 외부 통신 0 + 큐레이션 정체성 + cloud-zero 보존.
> **작성일 (v1)**: 2026-05-06 (commit `03225c4`, v2.0 standby 첫 스케치)
> **갱신 (v2)**: 2026-05-07 — Phase 22'.a 진입 직전 본격 풀 설계 보강. 9 영역으로 재구성 + 큐레이션 OSS 사례 + 법적 분석 + 화이트리스트 정합 + 출처 12+ 확장.
> **선행 ADR**: ADR-0014 (큐레이션 정체성) · ADR-0026 (외부 통신 화이트리스트) · ADR-0044 (라이브 갱신 패턴) · ADR-0047 (catalog minisign) · ADR-0059 (Phase 21' Trending Watcher)
> **결정 노트**: `docs/research/phase-22p-trend-report-decision.md`
> **ADR**: `docs/adr/0060-trend-report.md`
> **정책**: B 안 (큐레이션 + jsdelivr propagate). 사용자 PC 직접 scrape 거부. 외부 통신 화이트리스트 변경 0 (`huggingface.co` + `github.com` 보존).

---

## 0. 본 노트의 9 영역 매핑

사용자 요청 9 영역을 본 노트 §1~§14에 매핑:

| 사용자 요청 영역 | 본 노트 § |
|---|---|
| (1) 데이터 소스 매트릭스 | §1 (AI 회사 RSS) · §2 (학술 / arXiv) · §3 (뉴스 미디어) · §4 (YouTube) · §5 (거물 SNS) · §6 (GitHub Trending) |
| (2) 로컬 LLM 한국어 요약 | §7 |
| (3) 데이터셋 schema | §8 |
| (4) 라이브 갱신 빈도 | §9 |
| (5) 사용자 UX 흐름 | §10 |
| (6) 외부 통신 화이트리스트 | §11 |
| (7) 글로벌 베스트 프랙티스 | §12 |
| (8) 법적·라이선스 위험 | §13 |
| (9) 유사 OSS 인프라 | §14 |

§15는 결정 포인트 7건, §16은 핵심 참조 파일, §17은 출처 12개 모음.

---

## 1. AI 회사 공식 블로그 RSS — 1순위 신호 (확실히 가능)

### 1.1 확정 가용 RSS

| 매체 | 피드 URL | 갱신 빈도 | 비고 |
|---|---|---|---|
| **OpenAI News** | `https://openai.com/news/rss.xml` | 1~3건/주 | 공식 RSS, 모델 출시·연구 announcement 우선 |
| **TechCrunch AI** | `https://techcrunch.com/category/artificial-intelligence/feed/` | 시간당 다중 | RSS 2.0, WordPress generator |
| **The Verge** | `https://www.theverge.com/rss/index.xml` | 시간당 다중 | Atom/RSS 혼합, AI 카테고리 별도 필요 시 sub-path |
| **VentureBeat AI** | `https://venturebeat.com/category/ai/feed/` | 일 다중 | RSS 2.0 |
| **NVIDIA Blog** | `https://blogs.nvidia.com/feed/` | 일 1~2건 | GTC 시즌 트래픽 폭증, AI focus 카테고리 분리 가능 |
| **MIT Technology Review** | `https://www.technologyreview.com/feed/` | 일 다중 | AI 카테고리 별도 sub-feed |
| **Google AI Blog** | `https://blog.google/technology/ai/rss/` | 주 1~3건 | 공식 RSS |
| **HuggingFace Blog** | `https://huggingface.co/blog/feed.xml` | 주 2~5건 | 모델 출시·튜토리얼 |

### 1.2 부분 가용 (피드 검증 필요)

- **Anthropic**: `anthropic.com/news` 페이지만 확인. RSS 직접 노출 X — 큐레이터가 페이지 diff scrape (GHA runner 측, 사용자 PC X) 또는 RSS-Bridge 사용. 페이지 ETag 폴링이 현실적.
- **Google DeepMind**: `deepmind.google/discover/blog` — RSS 명시 부재. Bridge 또는 sitemap 폴링.
- **Meta AI / Microsoft Research / Mistral / Cohere / xAI / Stability**: RSS 직접 URL 미확인. 큐레이터 GHA에서 페이지 ETag 폴링 + selectolax CSS selector 추출이 현실적.

### 1.3 한국어 — 공식 RSS 빈자리 (현실)

- **Naver Cloud / Kakao / LG AI Research / SKT / KT** — *공식 한국어 RSS 명시 미확인*.
- 실무 권장: **THE AI** (`http://www.newstheai.com/rssIndex.html`, RSS index 명시) + **AI타임스** (`https://www.aitimes.kr/rss/allArticle.xml`, RSS 2.0 / `language=ko` / 일 5~10건 갱신 — 2026-05-07 검증).
- 한국 매체 콘텐츠 부족 시 — *영문 매체의 한국어 큐레이터 요약*으로 충당 (한국어 사용자 가치 손실 최소화).

### 1.4 LMmaster 적용

큐레이터 GHA 측에서 `feed-rs` 0.9 (Rust) 또는 `feedparser` (Python)로 If-Modified-Since/ETag 기반 폴링 — `feed-rs`는 Atom 1.0 / RSS 1.0 / RSS 2.0 / JSON Feed 모두 지원 + xml-rs 스트리밍 파서로 메모리 효율. RSS 직접 부재 매체는 **HTML diff (CSS selector + scraper crate)** 보조 — 모두 GHA runner 측 수행, 사용자 PC는 trends-bundle만 fetch.

> **CLAUDE.md 정합**: 사용자 PC 외부 통신 화이트리스트(`huggingface.co` + `github.com`) 변경 0. RSS 폴링은 *큐레이터 GHA*에서만 발생 → 외부 통신 0 정체성 보존.

---

## 2. 학술 / arXiv — 1순위 자동화 (BEST)

### 2.1 arXiv 공식 RSS (재구현 2024-01-31 이후 안정)

- 카테고리별: `https://rss.arxiv.org/rss/cs.LG` · `/atom/cs.LG`
- 다중 결합: `https://rss.arxiv.org/rss/cs.LG+cs.CL+cs.AI+cs.CV` (최대 2,000 결과)
- 갱신: **EST 자정 매일** 1회 (Friday 갱신 토요일 새벽).
- **Rate limit (확정)**: 단일 connection, **3 req/sec** (3초 간격), 초과 시 HTTP 503 + exponential backoff 의무.
- **User-Agent**: 명시 *권장* (강제 X). 책임감 있는 사용 + arXiv 측 사용 패턴 추적용. LMmaster는 `LMmaster-curator/0.1 (+https://github.com/lmmaster/lmmaster-trends-bundle)` 형태 권장.
- 검색 API: `http://export.arxiv.org/api/query?search_query=cat:cs.LG&sortBy=submittedDate&sortOrder=descending` — 검색 기반.

### 2.2 HuggingFace Daily Papers (BEST — 학술 핫리스트 골든 시그널)

- 공식 JSON API: `https://huggingface.co/api/daily_papers?date=YYYY-MM-DD&page=1&limit=N` — *인증 불필요*, CDN 캐시 (HF 측 fetch 부담 0).
- 응답 핵심 필드: `paper.id` (arxiv id) · `title` · `summary` (영어 abstract) · `ai_summary` (HF 자체 생성) · `ai_keywords` · `upvotes` · `numComments` · `submittedOnDailyAt` · `githubRepo` · `githubStars` · `organization`.
- **AK (`@akhaliq`) 큐레이션** — *학술 핫리스트의 골든 시그널*. AK가 매일 손으로 픽한 30~80개 논문 = 트위터/HF 화제성 + 실용성의 hybrid.
- 비공식 RSS 미러 (참고): `papers.takara.ai/api/feed` 또는 `huangboming/huggingface-daily-paper-feed` (GHA + raw github URL).
- 비공식 doc: `0x0is1/hf-papers-api-docs` — endpoint 3종 (`/api/papers/search?q=...` · `/api/papers/{arxiv_id}` · `/api/daily_papers?date=...`) 정리.

### 2.3 LMmaster 적용

큐레이터 GHA가 매일:
1. `huggingface.co/api/daily_papers?date={today}&limit=50` fetch (이미 화이트리스트, 인증 0)
2. arXiv RSS `cs.LG+cs.CL+cs.AI+cs.CV` fetch (User-Agent + 3 sec interval)
3. AK upvotes ≥ 10 OR githubStars ≥ 100 OR `cardData.language` 에 한국어 매칭 → 큐레이터 review queue
4. 큐레이터가 top 10~12 + 한국어 1~2문장 요약 + 출처 URL을 trends-bundle에 push.

> **CLAUDE.md 정합**: HF Daily Papers는 *deterministic source* (AK 손길은 *이미 큐레이션*) → LMmaster 큐레이션 정체성과 정합. LLM-as-judge 0.

---

## 3. AI 뉴스 미디어 — fair use 패턴

### 3.1 해외 BEST (RSS 안정)

| 매체 | 피드 | 응답 검증 |
|---|---|---|
| TechCrunch | `https://techcrunch.com/feed/` | RSS 2.0, WordPress 6.9.4, 시간당 다중 갱신. `<title>` + `<dc:creator>` + `<pubDate>` + `<category>` (AI/Startups/Security) + `<description>` |
| The Verge | `https://www.theverge.com/rss/index.xml` | (URL 차단 환경 검증 어려움) Atom/RSS 혼합, 시간당 다중 |
| VentureBeat | `https://venturebeat.com/category/ai/feed/` | RSS 2.0 |
| Ars Technica | `https://feeds.arstechnica.com/arstechnica/index` | RSS 2.0 |

### 3.2 한국 BEST 1~2

- **AI타임스** (`https://www.aitimes.kr/rss/allArticle.xml`) — 2026-05-07 검증: RSS 2.0, `language=ko`, 일 5~10건 갱신, `<title>` · `<link>` · `<description>` (CDATA) · `<author>` · `<pubDate>` 표준. 카테고리 (로봇·AI·금융 기술 등) 분류 명확.
- **THE AI** (`http://www.newstheai.com/rssIndex.html`) — 조선미디어그룹, RSS index 명시.
- **ZDNet Korea AI** (`https://feeds.zdnet.com/zdnet/topic/artificial-intelligence/`)는 영문 본사 피드 — 한국어 본문 직접 RSS는 별도 검증 필요.

### 3.3 fair use 패턴 (§13 법적 분석과 짝)

- trends-bundle item = `{title, source, summary_ko, source_url, published_at, kind: news}`.
- **본문 저장 X / 큐레이터 손길로 한국어 요약 1~2문장만**.
- 매체 표기 (`attribution: "AI타임스"`) + 직접 링크 (`source_url`) → fair use 안전.
- 출력 길이 제약: `summary_ko ≤ 200자` (테스트 invariant), 화면 표시 시 한국어 헤드라인 ≤ 80자.

---

## 4. YouTube AI 채널 — 두 경로 (Data API v3 + RSS Atom)

### 4.1 경로 A — YouTube Data API v3 (큐레이터 GHA 측)

**Quota 산수** (2026-05 기준 무료 한도 10,000 unit/day):

| Method | 비용 (unit) | LMmaster 사용 |
|---|---|---|
| `search.list` | 100 | ❌ 사용 금지 (산수 폭발) |
| `channels.list` | 1 | ✅ 채널 ID → uploads playlist 매핑 (1회 캐시) |
| `playlistItems.list` | 1 | ✅ uploads playlist에서 24h 신규 영상 추출 |
| `videos.list` (batch up to 50 IDs) | 1 | ✅ 영상 메타 일괄 fetch |
| `activities.list` | 1 | (대안) |

→ 10 채널 × `playlistItems.list` (1) + 1 batched `videos.list` (1) = **20 units/일**. 10,000 한도 대비 **500배 마진**.

### 4.2 경로 B — YouTube RSS Atom feed (인증 0, fallback)

- `https://www.youtube.com/feeds/videos.xml?channel_id={CHANNEL_ID}` — **공식 미공개지만 안정 동작 (2026-05 검증)**.
- Atom 1.0 + `yt:` / `media:` 네임스페이스: `<entry>` 마다 `<id>` (`yt:video:{id}`) · `<title>` · `<published>` · `<updated>` · `<media:group>` (title/description/thumbnail/community statistics).
- *playlist*: `?playlist_id=PL...` / *user*: `?user=username`.
- 인증 X, IP-based rate limit 관대 (수 req/min 수준 안전).
- **단점**: 최근 15개만 표시, history 깊지 않음 → daily cron이면 충분.

### 4.3 채널 큐레이션 (영문 + 한국어)

**영문 (확정 후보 — 큐레이터 진입 시 검토)**:
- Andrej Karpathy (`@AndrejKarpathy`)
- Yannic Kilcher (`@YannicKilcher`)
- Two Minute Papers (`@TwoMinutePapers`)
- Lex Fridman (`@LexFridman`)
- AI Explained (`@AIExplained-Official`)
- Sebastian Raschka (`@SebastianRaschka`)
- 3blue1brown (`@3blue1brown` — 수학/딥러닝)
- Computerphile (`@Computerphile`)
- Matthew Berman (`@matthew_berman`)

**한국어**:
- 조코딩 JoCoding (`@jocoding`) — 한국 #1 코딩 유튜버, AI focus 강화 (AX 컨설팅 운영 중).
- 노마드 코더 Nomad Coders — 한·콜 듀오, 일부 AI 컨텐츠.
- 모두의연구소 — 한국어 AI 강의·세미나 ([검토]).
- *추가 한국 AI 채널 5~10개*는 v2.0 진입 시점 큐레이터 1~2주 모니터링 후 화이트리스트 확정.

### 4.4 LMmaster 적용

큐레이터 GHA secrets에 `YOUTUBE_API_KEY` 1개 (별도 repo 분리). 매일 1회 cron으로 channels.list 캐시 + playlistItems.list 폴링 → 24h 신규 영상 추출 → `{title, channel, published_at, summary_ko, thumbnail_url, video_url, kind: video}` bundle. **사용자 PC는 YouTube API 호출 0**.

폴백 옵션: API key 발급 차단 시 RSS Atom feed 경로 → 같은 schema 산출. 현실적으로는 Data API가 통계(viewCount/likeCount)까지 한 번에 가능해 우월.

---

## 5. 거물 SNS — 인용 윤리 + 대체 채널 BEST

### 5.1 X (Twitter) — *매우 부정적*

- 2026-02-06부터 신규 가입 pay-per-use. 기존 Basic은 $200/월.
- ToS: scrape 금지 + 자동화 통신 endpoint 의무.
- **결정 권장**: X scrape / API 모두 **거부** (ADR-0060 §거부된 대안 #3 동일).

### 5.2 Bluesky AT Protocol — BEST 대체

- `https://public.api.bsky.app` — *인증 0*, AppView 자체 caching 적용, "관대한 rate-limits".
- `app.bsky.feed.getAuthorFeed` (read-only, 인증 0) — 특정 actor의 author feed (post + repost).
- Rate limit (정확치 검증):
  - **공식 doc**: 일반 IP당 *3,000 req/5분* (PDS 기준), 일부 endpoint는 더 엄격.
  - public AppView는 더 관대 (정확치 미공개, 429 응답 시 백오프).
  - 5,000 points/h 쓰기 제한은 *콘텐츠 쓰기*에만 적용 (LMmaster는 read-only이므로 무관).
- `searchPosts`는 **인증 필요** → 사용 금지. **큐레이터가 핵심 인사 handle 30~50개 직접 보유 + getAuthorFeed로만 폴링** (ADR-0060 §거부된 대안 #14 정합).

### 5.3 Mastodon RSS — BEST 보조

- 사용자별 RSS 자동: `https://{instance}/users/{user}.rss` 또는 `https://{instance}/@{user}.rss`.
- *posts only*, no replies, **public posts only** (private 자동 제외 → 프라이버시 안전).
- 갱신: 사용자별, 포스팅 빈도에 직결.
- Akkoma / Pleroma / Takahē / GoToSocial 등 호환 인스턴스도 동일 패턴.

### 5.4 개인 블로그 RSS (가장 안전)

| 인사 | 블로그 RSS |
|---|---|
| **Andrej Karpathy** | `https://karpathy.bearblog.dev/feed/` (Bear Blog, 2025-03 시작) + `https://karpathy.github.io/feed.xml` (구) |
| **Sebastian Raschka** | `https://sebastianraschka.com/feed.xml` (Substack 미러도) |
| **Lilian Weng (Lil'Log)** | `https://lilianweng.github.io/feed.xml` |
| **Chip Huyen** | `https://huyenchip.com/feed.xml` |
| **Simon Willison** | `https://simonwillison.net/atom/everything/` (포스트 + 마이크로블로그) |
| **Yann LeCun** | (Mastodon: `@ylecun@mastodon.social.rss`) |

### 5.5 한국 인사 (큐레이터 검토 — v2.0 진입 시)

- Naver Hyperscale 책임자 (Bluesky/블로그 RSS 검색 우선)
- LG AI Research (블로그 / Medium 등)
- 카카오브레인 / SKT AI / KT 융합기술원 — 공식 블로그 RSS 가용성은 v2.0 진입 시 1주 검증.

### 5.6 LMmaster 적용

trends-bundle SNS 항목 우선순위:
1. 본인 블로그 RSS (BEST — 1차 출처, 라이선스 명확).
2. Bluesky `public.api.bsky.app` authorFeed (조회 인증 0).
3. Mastodon RSS (사용자 RSS auto-discoverable).
4. **X 인용 거부** (ToS + 비용).

큐레이터 검토 후 *1~2문장 인용 + 출처 URL*만 → fair use 안전 (§13).

---

## 6. GitHub Trending — AI 분야

### 6.1 공식 API 부재 (확정)

GitHub `trending` 페이지의 공식 API 부재 — Community discussion #161519에서 다년간 미해결. 현실 옵션:

- **`huchenme/github-trending-api`** (Node.js) — 호스팅 종료 위험.
- **`antonkomarev/github-trending-api`** (Rust) — 자체 호스팅 (LMmaster 친화).
- **자체 scrape**: 큐레이터 GHA에서 `https://github.com/trending/python?since=daily` HTML 직접 파싱 — 가장 견고 + 의존 0.

### 6.2 LMmaster 적용

큐레이터 GHA가 자체 scrape:
1. Rust `scraper` crate로 `https://github.com/trending` (전체) + `https://github.com/trending/python?since=daily` + `https://github.com/trending/rust?since=weekly` 등 fetch.
2. 라이선스 화이트리스트 (Apache-2 / MIT / BSD / 0BSD / Unlicense / CC0).
3. 토픽 필터: `llm` / `ai` / `agent` / `ml` / `transformer` / `rag` / `mcp` / `tts` / `stt` 매칭.
4. 상위 10 → `{repo, stars_today, summary_ko, license, language, url, kind: github}` bundle.
5. 외부 의존 0 (자체 scrape, github.com 화이트리스트 정합).

---

## 7. 로컬 LLM 한국어 요약 — fresh newspaper 생성

### 7.1 모델 가용성 게이트 (4B+)

활성 조건 — 다음 중 *1개 이상* 설치 + 로드 가능:

| 모델 | params | 강점 | LMmaster 카탈로그 |
|---|---|---|---|
| **Gemma 3 4B** | 4B | 다국어 + 128K context, 한국어 자연스러움 우수 | ✅ |
| **Nemotron 3 Nano 4B** | 4B | NVIDIA, 한국어 우수, 라이선스 NVIDIA-Open | ✅ (2026-05-06 추가) |
| **EXAONE 3.5 7.8B** | 7.8B | LG AI, 한국어 1순위, KoMT-Bench 자체평가 | ✅ |
| **HCX-SEED 8B** | 8B | NAVER HyperCLOVA X, 한국어 BEST | ✅ |
| Llama-3-Korean-Bllossom-8B | 8B | Korean tabular SOTA (arXiv 2501.10487) | (검토) |

미충족 시 메뉴 disabled + hint chip:
- ko: "동향 메뉴는 4B+ 모델이 필요해요. EXAONE 3.5 등 설치 후 다시 와요."
- en: "The Trends menu needs a 4B+ model. Install EXAONE 3.5 or similar to continue."

### 7.2 입력 컨텍스트 길이 (현실적)

- Gemma 3 4B: 128K — bundle 전체 (12~30 item × 평균 200~300 토큰 = ~6~9K)을 1 prompt에 충분히 수용.
- Nemotron 3 Nano 4B: 128K — 동일.
- EXAONE 3.5 7.8B: 32K — 충분 (12~30 item × 300 토큰 = ~9K).
- HCX-SEED 8B: 16K — 마진 있음.

→ **bundle 전체를 1 prompt에 넣어도 되는 contexts**. 단 *추론 속도*를 위해 §7.4 캐시 + diff 전략으로 신규 item만 요약.

### 7.3 프롬프트 템플릿 (해요체 + 1~2문장 + 출처 보존)

```
[SYSTEM]
당신은 LMmaster의 한국어 AI 트렌드 뉴스페이퍼 작성 도우미예요.
- 톤: 해요체. "이래요" / "할게요" / "왔어요" 스타일.
- 분량: 항목당 1~2문장, 한국어 80자 이내.
- 영어 기술 단어 (LLM, RAG, agent 등)는 첫 등장 시 한국어 풀이 (예: "RAG(검색 보강 생성)").
- 출처 링크는 그대로 보존해요.
- 사실 추측 금지 — bundle 메타에 없는 내용은 만들지 않아요.

[USER]
오늘의 트렌드 묶음이에요. 사용자 관심사 필터: {tags_filter}.
관심사에 맞는 항목만 한국어 한 줄 요약으로 골라줘요. 최대 12개.

{bundle_items_json}
```

영문 매체 요약 시 — 큐레이터가 미리 작성한 `summary_ko` 그대로 노출하고, 로컬 LLM은 *정렬 (rank by relevance to user tags)* + *카테고리 그룹핑*만 수행 (사실 오류 위험 회피).

### 7.4 캐싱 전략 (SQLite)

```sql
CREATE TABLE IF NOT EXISTS trend_cache (
  bundle_id TEXT NOT NULL,
  bundle_generated_at TEXT NOT NULL,
  item_id TEXT NOT NULL,
  user_tags TEXT NOT NULL,         -- ['korean','agent','llm'] JSON sorted
  model_id TEXT NOT NULL,           -- 'gemma-3-4b' 등
  rendered_ko TEXT NOT NULL,
  generated_at_ms INTEGER NOT NULL, -- LLM 호출 시점
  PRIMARY KEY (bundle_id, item_id, user_tags, model_id)
);
CREATE INDEX idx_trend_cache_generated ON trend_cache(generated_at_ms);
```

- bundle 갱신 시 (`bundle_generated_at` 변경) → 신규 item만 LLM 호출 (diff).
- 사용자 관심사 변경 시 → 동일 bundle 안에서 캐시 미스만 호출.
- TTL 30일 (만료 bundle GC) — 디스크 사용 ↑ 차단.
- *동일 (bundle, item, tags, model) 결과는 deterministic*. seed 고정 시 재현 가능.

참고 라이브러리: `losfair/sqlite-cache` (TTL + per-key lock + thundering herd 방어), `jkelin/cache-sqlite-lru-ttl` (LRU + TTL eviction). LMmaster 자체 SQLCipher와 통합 시 별 schema로 가능.

### 7.5 모델별 호환성 noteworthy

- **Gemma 3 4B** — system role 직접 미지원 (Gemma 4부터 표준 system/assistant/user). vLLM Recipes의 chat template 적용 권장.
- **Nemotron 3 Nano 4B** — system role 표준 ChatML 패턴.
- **EXAONE 3.5 7.8B** — Tabular-TX 검증 — Korean 0.45 SOTA. system role + journalist persona prompting 권장.
- **HCX-SEED 8B** — HyperCLOVA X 계열, NAVER 내부 chat template (KoMT-Bench 호환).

LMmaster Workbench가 chat template metadata 자동 매칭 (Phase 13'.h 기존 인프라).

---

## 8. trends-bundle JSON Schema — Phase 13'.g.3 minisign 호환

### 8.1 schema (v1)

```json
{
  "$schema_hint": "lmmaster trends-bundle schema_version=1. Auto-generated by curator GHA + manual review.",
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

### 8.2 tagged enum (Rust serde 패턴)

```rust
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum TrendItem {
    Paper(PaperItem),
    Blog(BlogItem),
    News(NewsItem),
    Video(VideoItem),
    Github(GithubItem),
    Sns(SnsItem),
}
```

기존 `BenchErrorReport` / `ExclusionReason` / `ScanApiError`와 동일 표준 형태 (CLAUDE.md §4.2 정합).

### 8.3 minisign 서명

기존 Phase 13'.g.3 sign-manifests workflow에 `manifests/trends/bundle.json` 추가 → `bundle.json.minisig` 자동 생성. 동일 keypair 재사용. registry-fetcher generic 활용으로 호출 측 추가만 필요.

### 8.4 expires_at 정책

- **7일** — 그 후 사용자 UI는 stale 표시 + 큐레이터 push가 늦어진 신호.
- 만료된 bundle 처리: `expires_at` 지나면 "갱신 N일 전" 표시 (현재 정책) vs 자동 hide (대안). v2.0 진입 시 사용자 결정 (결정 노트 §4 미정).

### 8.5 시간 기반 partitioning vs 단일 합본

- **단일 합본 권장** (현재 결정): bundle 1개에 12~30 item, jsdelivr 1 fetch → 단순.
- 거부된 안:
  - 일일 dump (`bundle-2026-05-07.json`): 사용자 PC fetch 회수 ↑ + 합본 인덱스 추가 부담.
  - parquet partitioning: 사용자 측 Rust parquet 의존 추가 + 화면당 12 item이라 SQL 분석 불필요.

---

## 9. 라이브 갱신 빈도 — daily / 6h / weekly 균형

### 9.1 큐레이터 GHA cron 매트릭스

| 빈도 | 가치 | 비용 (큐레이터 부담) | 권장 |
|---|---|---|---|
| **6h** | 신선도 ↑↑ | 큐레이터가 매일 4회 review 불가 → 자동 review queue 누적만 가능 | ❌ |
| **daily 00:00 UTC** | 학술 (arXiv 자정 갱신) + 매체 일 다중 | 큐레이터 review queue 매일 누적 — 부담 적정 | ✅ |
| **weekly Mon 09:00 KST** | 큐레이터 push 의식 (v1 적정) | 매주 1회 의도적 게이트 | ✅ (push) |
| **on-demand** | 큰 announcement (예: GPT-5 공개) | manual workflow_dispatch | ✅ (옵션) |

### 9.2 권장 흐름

```
[매일 00:00 UTC] curator GHA cron
  ↓ fetch 30+ source (RSS + HF Daily Papers + arXiv + YouTube + Bluesky + Mastodon + GitHub trending)
  ↓ deterministic dedupe (URL fingerprint)
  ↓ JasonEtco/create-an-issue (제목 fingerprint 기반 update_existing: true)
  ↓ review queue Issue (open) — 큐레이터에게 알림
[매주 월요일 09:00 KST] 큐레이터 작업 30~60분
  ↓ queue 100~200 item 중 12~30 선별
  ↓ 한국어 1~2문장 요약 작성 (summary_ko)
  ↓ tags 태깅 (deterministic 화이트리스트)
  ↓ PR `manifests/trends/bundle.json`
  ↓ sign-manifests workflow 자동 minisign 서명
[bundle PR merge] jsdelivr propagate (1~24h)
[사용자 PC] 메뉴 진입 시 registry-fetcher::fetch("trends-bundle") + ETag 갱신 + 로컬 LLM 요약
```

### 9.3 GHA cron drift

- *알려진 문제*: 매시 정각 cron이 5~30분 drift, 부하 시 skip.
- 완화: cron 실패 알림이 issue로 자동 생성되도록 헤일프루프 워크플로 추가 (Phase 21' 패턴 재활용).
- workflow_dispatch 수동 트리거 옵션 (announcement 시 즉시 push).

### 9.4 사용자 측 fetch 빈도

- 메뉴 *진입 시* fetch (lazy) — *항상 latest*.
- ETag 캐시 (registry-fetcher 4-tier fallback에 ETag 헤더 동작 확인 — Phase 21' 검증).
- 24h 강제 폴백 X — 사용자가 메뉴 안 들어가면 fetch 안 함 (외부 통신 0 정체성 강화).

---

## 10. 사용자 UX 흐름 — 메뉴 진입 → fresh newspaper 정렬

### 10.1 진입 시퀀스

```
1. 사용자 메뉴 클릭 ("동향 보기")
2. [모델 게이트] 4B+ 모델 1개 이상 활성? 
   - No → disabled state + hint chip ("EXAONE 3.5 등 설치 후 다시 와요")
   - Yes → 진입 진행
3. registry-fetcher::fetch("trends-bundle") (4-tier fallback)
   - jsdelivr 우선 → github → vendor → bundled
   - minisign verify
   - SQLite cache 기록
4. UI [로딩] "오늘의 동향을 받아오고 있어요..."
5. [로컬 LLM] bundle 항목 + 사용자 tags filter → summary_ko 정렬
   - cache hit → 즉시 표시
   - cache miss → 신규 item만 LLM 호출 (diff)
6. UI [표시] 카테고리별 카드 (paper / blog / news / video / github / sns)
   - 각 카드 ≤ 80자 한국어 요약 + 출처 링크 + published 상대시간 ("3일 전")
7. [필터/카테고리 토글] 사용자 클릭 → cache + tags 매핑 → 즉시 재정렬
8. [새로고침 버튼] 강제 fetch + 재요약 (수동 트리거)
```

### 10.2 카테고리 필터 UI

- 화면 상단 chip rail: `전체 / 학술 / 블로그 / 뉴스 / 영상 / 깃허브 / SNS`
- 좌측 사이드바: `사용자 관심사` (체크박스 — `한국어 / Agent / RAG / 멀티모달 / 추론 / 코딩 등`)
- *deterministic*: 동일 (bundle, tags) → 동일 결과 100회 보장 (테스트 invariant §5.5).

### 10.3 빈 상태

- *bundle 만료*: "최근 갱신이 N일 전이에요. 곧 새 동향이 도착할 거예요." (해요체)
- *사용자 필터 결과 0*: "조건에 맞는 동향이 없어요. 필터를 조정해 볼래요?"
- *모델 미설치*: "동향 메뉴는 4B+ 모델이 필요해요. EXAONE 3.5 등 설치 후 다시 와요."

### 10.4 a11y / 키보드 (CLAUDE.md §4.3)

- chip rail은 `role="radiogroup"` + 각 chip `role="radio" aria-checked`.
- 사이드바 체크박스는 표준 `<input type="checkbox">`.
- Esc 키: 메뉴 닫기 (Drawer 패턴 재활용).
- 카드 focus-visible ring (`--primary-a-3`).
- `prefers-reduced-motion`: 카드 진입 fade는 토큰 차원에서 자동 비활성.

### 10.5 디자인 토큰 (LMmaster 디자인 시스템)

- 카드 배경: `--surface-2`
- 카테고리 chip: `--surface-3` + active `--primary-a-3`
- 출처 표기: `--text-muted` + monospace tabular-nums (날짜)
- 한국어 본문: `--text-primary` (기본 16px, 1.6 line-height)

---

## 11. 외부 통신 화이트리스트 — 갱신 필요성 분석

### 11.1 현재 화이트리스트 (ADR-0026)

- `huggingface.co` (모델 카탈로그 + Daily Papers API)
- `github.com` (raw URL + Releases)
- `cdn.jsdelivr.net` (manifest 합본 propagate)

### 11.2 사용자 PC 측 — 갱신 필요 0 ✅

trends-bundle은 본 합본을 *jsdelivr* / *github raw* / *bundled* 4-tier로 fetch → 화이트리스트 변경 X.

```
사용자 PC fetch chain:
  cdn.jsdelivr.net/gh/{owner}/lmmaster-trends-bundle@latest/manifests/trends/bundle.json
  → raw.githubusercontent.com/{owner}/lmmaster-trends-bundle/main/manifests/trends/bundle.json
  → 본 repo bundled fallback
```

### 11.3 큐레이터 GHA 측 — 다양한 도메인 fetch (분리 정합)

큐레이터 GHA *별도 repo* `lmmaster-trends-bundle`이 fetch하는 도메인 (사용자 PC 무관):

| 도메인 | 용도 |
|---|---|
| `huggingface.co` | Daily Papers API |
| `rss.arxiv.org` | arXiv RSS |
| `export.arxiv.org` | arXiv API (필요 시) |
| `openai.com` · `techcrunch.com` · `theverge.com` · `venturebeat.com` · `blogs.nvidia.com` | 매체 RSS |
| `aitimes.kr` · `newstheai.com` | 한국 매체 RSS |
| `youtube.com` · `googleapis.com` (YouTube Data API) | YouTube API + RSS Atom |
| `public.api.bsky.app` | Bluesky AppView |
| `mastodon.social` 등 다양 instance | Mastodon RSS |
| `karpathy.bearblog.dev` · `lilianweng.github.io` 등 | 거물 블로그 RSS |
| `github.com` (trending HTML scrape) | GitHub Trending |

→ 사용자 PC 측 화이트리스트는 *변경 0*. 큐레이터 GHA는 별도 repo + secrets 분리로 외부 통신 정체성 보존.

### 11.4 trends-bundle host 결정

**별도 repo `lmmaster-trends-bundle`** (Phase 21' 정공) 권장:
- repo: `github.com/{owner}/lmmaster-trends-bundle` (public, MIT).
- jsdelivr: `cdn.jsdelivr.net/gh/{owner}/lmmaster-trends-bundle@main/manifests/trends/bundle.json`.
- github raw: `raw.githubusercontent.com/{owner}/lmmaster-trends-bundle/main/manifests/trends/bundle.json`.
- secrets 분리 (`YOUTUBE_API_KEY` 등 본 repo 노출 0).

v1.x prototype 시점에는 본 repo `crates/trends-bundle-curator/` 단계적 가능 (Phase 21' 패턴 준용).

### 11.5 RSS feed별 도메인 다양성 처리 — curator side aggregator 패턴

큐레이터 GHA 자체에서 *모든 외부 fetch 끝낸 후 trends-bundle 단일 합본*으로 변환 → 사용자 PC fetch 도메인 1개 (`cdn.jsdelivr.net`)로 수렴 (ADR-0026 정합).

**aggregator 흐름**:
```
GHA matrix-strategy (병렬):
  - HF Daily Papers fetch
  - arXiv RSS fetch (cs.LG / cs.CL / cs.AI / cs.CV)
  - 매체 RSS 8종 fetch (영문 + 한국 2종)
  - YouTube API 10채널 fetch
  - Bluesky 30 handles getAuthorFeed
  - Mastodon 5 handles RSS
  - 거물 블로그 6 RSS
  - GitHub trending HTML scrape
  ↓
JasonEtco/create-an-issue (review queue)
  ↓ 큐레이터 review 후 PR
manifests/trends/bundle.json (단일 합본)
  ↓ minisign + jsdelivr propagate
사용자 PC: cdn.jsdelivr.net 1개 도메인만 fetch
```

---

## 12. 글로벌 베스트 프랙티스 — 비슷한 시도 OSS / 미디어

### 12.1 The Batch (deeplearning.ai) — 주간 큐레이션 뉴스레터

- 운영 주체: Andrew Ng + DeepLearning.AI 편집팀.
- 갱신: **주간** 1회 (수요일).
- 콘텐츠: AI 뉴스 + 인사이트 + Andrew Ng 개인 letter.
- 구조: business / research / culture / hardware / careers 카테고리.
- 큐레이션 정신: "AI 현실 — 무엇이 실제 일어나고 있고 무엇을 의미하는지"; *hype 차단*.
- **LMmaster 적용**: `curator_note_ko` 필드로 큐레이터 1단락 인사 — Andrew Ng letter 패턴 흡수.

### 12.2 Hacker News — 알고리즘 + community

- 시스템: 사용자 점수 (upvote/downvote) + 시간 decay 기반 알고리즘.
- 큐레이션 운영자: dang (Daniel Gackle) — manual 개입 (대형 사이트 1인 mod).
- *Show HN* 패턴: 작성자 self-submit, 커뮤니티 가시성 의존.
- **LMmaster 적용**: HN 자체를 source로 부분 검토 가능 (`https://hn.algolia.com/api` — 가능하면 `tag=story&query=AI`). 단 신호 잡음 비율 높아 v2.x 후순위.

### 12.3 Papers with Code — algorithmic + manual hybrid

- arXiv 논문 + GitHub 코드 자동 매칭.
- benchmark 표 + state-of-the-art ranking *알고리즘* + 사용자 contribute (manual hybrid).
- LMmaster 적용: HF Daily Papers `githubRepo`/`githubStars` 필드가 동일 신호 (이미 활용).

### 12.4 The Rundown AI / TLDR AI — 뉴스레터 형식

- 일일 emails + 핵심 헤드라인 3~5개 + 1줄 요약.
- 운영: 인하우스 편집팀 + AI-assisted draft.
- LMmaster 적용: `curator_note_ko` (1단락) + 카테고리별 12 item ≤ 80자 패턴이 여기서 흡수.

### 12.5 arxiv-sanity (Karpathy) — 알고리즘 추천

- arXiv API 폴링 + SVM over tfidf features (paper abstracts) + tag-based filter.
- 사용자 personal library + 추천 시스템.
- *Lite 버전* (2021-11) — 작아지고 안정.
- **LMmaster 차이점**: arxiv-sanity는 *알고리즘* 추천 (LLM 의존성 0), LMmaster trend report는 *큐레이션 + 로컬 LLM 정렬* hybrid.
- 흡수: 사용자 tags filter는 deterministic — 로컬 LLM은 톤·정렬만.

### 12.6 awesome-* (GitHub awesome lists) — 메인테이너 검토 PR

- 운영: 대형 awesome list (예: `Hannibal046/Awesome-LLM`)는 PR 검토 기반.
- 검토 기준: 활성 유지 (2~3년 미활동 자동 제거) + 명확한 doc + 관련성 명시.
- *불확실 PR*: 메인테이너가 open 상태로 두고 community reactions 투표.
- **LMmaster 적용**: 큐레이터 매주 1회 의도적 게이트 = awesome-list 메인테이너 패턴 흡수. 자동 LLM PR 거부 (ADR-0060 §거부된 대안 #8 정합).

### 12.7 한국 — GeekNews / AI 코리아 커뮤니티

- **GeekNews** (`news.hada.io`) — 1인 운영 (xguru), 한국어 IT 큐레이션.
- **AI 코리아 커뮤니티** (`aikoreacommunity.com`) — Generative AI 학습.
- 패턴: *작성자 self-submit + 메인테이너 검토 + community 소비*.
- LMmaster 흡수: 한국어 자체 콘텐츠 *부족* 시 영문 매체의 한국어 요약 전략 = GeekNews의 영문 article 한국어 한 줄 요약 패턴.

---

## 13. 법적·라이선스 위험 — fair use vs 저작권 + EU AI Act

### 13.1 EU 권역 — Article 15 (구 Article 11) "press publishers' right"

- 2019 EU Copyright Directive (DSM Directive) §15: 언론 출판사가 *online use*에 대해 권리 행사 가능.
- **예외**: "individual words or very short extracts" (단어 / 매우 짧은 발췌) + hyperlinking.
- 시행 ('19~'21년 회원국별 transposition): 독일 2020, 스페인 2021 등.
- 이전 사례: 독일·스페인 link tax 시도 → Google News Spain 종료 / Google 독일 무링크 snippet.
- **LMmaster 정합**:
  - 본문 저장 X (큐레이터 *자체 작성* 1~2문장 한국어 요약).
  - 매체 표기 (`attribution`) + 직접 링크 (`source_url`) 보존.
  - 원문 *재게시* 0.

### 13.2 EU AI Act 2026 + TDM 예외

- 2024-08 발효, 2026 단계적 적용 (GPAI 의무 2025-08, 일반 2026-08).
- *Recital 105*: TDM (Text and Data Mining) 기법은 GPAI 학습에 사용 가능, **단 Article 4(3) opt-out 존중 의무**.
- **Hamburg 법원 (2025-12)**: opt-out이 "machine-readable"이 되려면 *자동 프로세스가 해석 + 차단 가능*해야 함 (단순 ToS 문구 *natural language*도 일부 인정).
- 2026-06 Copyright Directive 검토 + AI Act GPAI Code of Practice가 추가 명확화 예정.
- **LMmaster 정합**: 본 페이즈는 *AI 학습이 아니라 사용자 향 큐레이션 콘텐츠*이므로 TDM 예외와 별개. *큐레이터 손길의 1~2문장 한국어 요약*은 *번역 + 매우 짧은 발췌*에 해당 → Article 15 예외 안에서 안전.

### 13.3 미국 — Fair Use (transformative)

- 17 USC §107 4 factor test:
  1. 사용 목적 (commercial vs nonprofit / educational): LMmaster는 무료 OSS — 우호.
  2. 원작 성격 (factual vs creative): 뉴스 기사는 factual — 우호.
  3. 사용 분량 (substantiality): 1~2문장 한국어 요약 — 우호.
  4. 시장 영향: 출처 링크가 *원작 트래픽 증가*에 기여 — 우호.
- transformative 정의: *변형적 사용* (한국어 번역 + 큐레이터 손길 = 변형성 있음).

### 13.4 한국 — 저작권법 §28 (공정이용)

- 2011년 ISP 면책 + §35-3 공정이용 도입.
- 한국 매체 (AI타임스 / THE AI 등) 인용 시:
  - 출처 명시 의무 (저작권법 §37).
  - 정당한 범위 내 인용 (저작권법 §28).
  - LMmaster — 한국어 매체 RSS는 *제목 + summary_ko 자체 작성 1~2문장 + 직접 링크* → §28 안전.

### 13.5 일본 — 저작권법 §30-4 (TDM 우호)

- 2018 개정 §30-4: AI 학습 / 정보 분석 목적 광범위 허용.
- LMmaster — *학습 무관*, 큐레이션 사용 → §30-4와 별개. 그러나 일본 매체 (Nikkei 등) RSS 인용 시 동일 fair use 패턴이 유효.

### 13.6 ToS 측 위험 분석

| 매체 / 플랫폼 | ToS 위험 | LMmaster 대응 |
|---|---|---|
| X (Twitter) | API $200/월 + scrape 금지 | **거부** ✅ |
| LinkedIn | scrape 금지 (HiQ Labs vs LinkedIn 케이스) | **거부** ✅ |
| Facebook | scrape 금지 | **거부** ✅ |
| YouTube | Data API ToS 준수 (10k unit/day, attribution 의무) | API key + attribution ✅ |
| Bluesky | public AppView open access 명시 | getAuthorFeed only ✅ |
| Mastodon | 인스턴스별 ToS — public posts only | RSS auto-discoverable ✅ |
| RSS 일반 | 표준 — fair use 안에서 자유 | 1~2문장 + 출처 ✅ |
| GitHub | Acceptable Use Policy — public scrape 허용 | trending HTML scrape ✅ |

### 13.7 LMmaster fair use 정책 (테스트 invariant 포함)

ADR-0060 §테스트 invariant 정합:
- `summary_ko ≤ 200자` (자체 작성 한국어, 원문 번역 X).
- `source_url` 보존 (원작 트래픽 기여).
- `attribution` 명시 (저자 / 매체 표기).
- 본문 저장 X (`description_ko`만, full article body X).

### 13.8 콘텐츠 라이선스 분류 (큐레이터 GHA 측 자동 라벨)

```rust
pub enum ContentLicense {
    /// CC BY 4.0 (큐레이터 자체 작성 한국어 요약)
    CcBy40,
    /// 매체 자유 인용 (제목 + 1~2문장 + 링크)
    FairUseCitation,
    /// MIT / Apache-2 / BSD (GitHub trending repos)
    OssPermissive,
    /// CC BY-SA / NC 등 (제한 인용)
    RestrictedCitation,
    /// 명시 없음 — 큐레이터 보수적 인용 (제목 + 링크만)
    Unspecified,
}
```

bundle item당 `license_kind` 필드 추가 옵션 (v2.x 검토).

### 13.9 EULA 갱신 (v2.0 진입 시)

- *큐레이션 트렌드 데이터*가 사용자 PC에서 fetch됨 (cdn.jsdelivr.net 1개 도메인).
- 출처 표기 (attribution) 보존 — fair use 원칙.
- 거물 SNS 인용은 1~2문장 + 원문 링크 (fair use 한도). X(Twitter) 거부.
- 사용자가 trends-bundle 콘텐츠 *재배포*하지 않음 (본 LMmaster 안 표시 only).
- 큐레이터 자체 한국어 요약 = CC BY 4.0 (출처 표기 시 자유 활용).

---

## 14. 유사 OSS의 trend curation 인프라 — GitHub repo + GHA cron + signed manifest

### 14.1 검증된 패턴 — Phase 21' Trending Watcher (LMmaster 자체)

- `lmmaster-trending-watcher` 별도 repo, public, MIT.
- GHA cron `0 */6 * * *` (6h) — 본 페이즈는 daily로 조정.
- `JasonEtco/create-an-issue@v2` — `update_existing: true` + `search_existing: open` 동일 fingerprint 자동 dedupe.
- `peter-evans/create-issue-from-file` 거부 (dedupe 없음).
- **검증 결과** (Phase 21'.e 운영 1주): cron drift 5~30분 내 안정, dedupe 정확, GHA 인증 rate limit 안전.

### 14.2 awesome-* lists — 메인테이너 검토 PR 패턴

- 대표: `Hannibal046/Awesome-LLM` (수만 stars, 일주일 PR 다수).
- 검토 운영: 메인테이너 1~3인, *active maintenance + 명확한 doc + relevance* 게이트.
- 자동 detection: 2~3년 미활동 repo 자동 식별 후 archive.
- **LMmaster 흡수**: 큐레이터 매주 월요일 30~60분 의도적 게이트 = awesome-list 메인테이너 패턴.

### 14.3 daily-arxiv-org-papers (huangboming) — GHA + raw URL host

- repo: `huangboming/huggingface-daily-paper-feed`.
- 패턴: GHA cron + HF Daily Papers fetch + RSS feed XML 생성 + `raw.githubusercontent.com` host.
- **LMmaster 차이점**: signed manifest (minisign) 없음, schema 없음. LMmaster는 trends-bundle JSON + minisign + tagged enum 6종 + tier fallback로 한 단계 위.

### 14.4 ai-news-aggregator-bot (hrnrxb) — Telegram + AI

- 패턴: Python + GHA cron 5h + SQLite dedupe + Telegram bot 발송.
- AI 출처: research blogs + GitHub trending + The Verge + HN.
- *공통 학습*: SQLite dedupe (URL fingerprint) + 다양 source aggregation.
- *차이점*: Telegram bot host (LMmaster는 데스크톱 native).

### 14.5 home-assistant / brew/cask — 합본 manifest + 서명 pattern

- Home Assistant `add-on` repo + 인증서 chain.
- Homebrew Cask — 수천 cask + community PR + maintainer 검토.
- **LMmaster 흡수**: minisign Ed25519 (Phase 13'.g.3 검증) + curated manifest + raw URL fallback chain (jsdelivr → github raw → bundled).

### 14.6 검증된 GHA / Action 매트릭스

| Action / Library | 검증 |
|---|---|
| `actions/checkout@v4` | 표준 |
| `JasonEtco/create-an-issue@v2` | dedupe 우월 (Phase 21' 검증) |
| `peter-evans/create-pull-request@v6` | bundle PR 자동화 (옵션) |
| `actions/setup-rust@v1` 또는 `dtolnay/rust-toolchain@stable` | Rust binary 빌드 |
| `actions/cache@v4` | RSS ETag / 폴링 결과 캐시 |
| `softprops/action-gh-release@v2` | bundle GitHub Release tier (옵션) |

### 14.7 차원별 비교 표

| 시스템 | 큐레이션 방식 | 갱신 빈도 | 서명 / 무결성 | 사용자 fetch chain |
|---|---|---|---|---|
| **The Batch** | 편집팀 manual | weekly | (이메일) | newsletter |
| **arxiv-sanity** | 알고리즘 SVM+tfidf | hourly | None | 웹 사이트 |
| **awesome-LLM** | 메인테이너 PR | on-PR | git history | GitHub 직접 |
| **GeekNews** | 1인 메인테이너 | daily | None | 웹 사이트 |
| **AI News Aggregator Bot** | 자동 + dedupe | 5h | None | Telegram |
| **Phase 21' Trending Watcher** | deterministic + 큐레이터 PR | 6h + 주 1회 | minisign Ed25519 | catalog manifest |
| **LMmaster Phase 22'** (본 페이즈) | curator + GHA + 매주 1회 | daily review + weekly push | minisign Ed25519 | 4-tier (jsdelivr/github/vendor/bundled) |

LMmaster trend report = **awesome-list 메인테이너 + The Batch 편집 + Phase 21' GHA 운영 + minisign 서명** = 4가지 베스트 프랙티스 합성.

---

## 15. 결정 포인트 7개 (v2.0 진입 시 사용자 결정 필요)

(ADR-0060 §결정 노트 §7과 동일 — 진입 시점 사용자 명시 결정)

| # | 포인트 | 권장 (현재) | 거부 / 후순위 |
|---|---|---|---|
| 1 | **rss.arxiv.org 화이트리스트 확장** | GHA 측만 fetch, 사용자 PC는 trends-bundle만 (현행 정체성 보존) | 사용자 PC 측 추가 (정체성 훼손) |
| 2 | **X (Twitter) API/scrape** | **거부 확정** | API 추가 (비용 + ToS) |
| 3 | **별도 repo vs 본 repo prototype** | 별도 repo 정공 (v1.x prototype 후 분리 가능) | 본 repo 영구 (secrets 노출 위험) |
| 4 | **큐레이터 운영 부담 (30~60분/주)** | 합의 가능 → 진행 | 합의 불가 시 LLM-assist draft 옵션 검토 |
| 5 | **trends-bundle schema kind enum 6종** | 채택 (paper/blog/news/video/github/sns) | 5종 / 7종 (검토) |
| 6 | **모델 게이트 — 4B+ 모델 4종** | 채택 (Gemma 3 4B, Nemotron 3 Nano 4B, EXAONE 3.5 7.8B, HCX-SEED 8B) | 2B 폴백 (사용자 PC 사양 약한 경우 — v2.x) |
| 7 | **한국 매체 1순위** | THE AI + AI타임스 (RSS 안정) | 더 많은 한국 매체 추가 (RSS 검증 필요) |

---

## 16. 핵심 참조 파일 (LMmaster 본 repo)

- `crates/registry-fetcher/src/fetcher.rs` — 4-tier fallback + minisign verify (generic, 호출 측만 추가하면 trends-bundle 즉시 호환).
- `crates/registry-fetcher/src/source.rs` — Vendor/Github/Jsdelivr/Bundled tier 정의.
- `docs/research/phase-21p-trending-watcher-decision.md` — 별도 repo + GHA cron + 큐레이터 흐름 청사진.
- `docs/research/phase-21p-trending-watcher-reinforcement.md` — HF API + JasonEtco/create-an-issue 검증 노트.
- `docs/research/phase-13pa-live-catalog-decision.md` — bundle JSON + jsdelivr + hot-swap 패턴.
- `docs/research/phase-13pg3-manifest-signature-expansion-decision.md` — sign-manifests workflow 확장 청사진.
- `docs/adr/0044-live-catalog-refresh.md` — 라이브 갱신 패턴 ADR.
- `docs/adr/0047-minisign-catalog-signature.md` — Ed25519 서명 ADR.
- `docs/adr/0059-trending-watcher.md` — Phase 21' 별도 repo 정공 ADR.
- `docs/adr/0060-trend-report.md` — 본 페이즈 ADR (Proposed).

---

## 17. 출처 (Sources) — 12+ 항목

1. [arXiv RSS Feeds](https://info.arxiv.org/help/rss.html) — RSS / Atom URL 패턴 + EST 자정 갱신 + 2,000 결과 제한.
2. [arXiv API User Manual (rate limits)](https://info.arxiv.org/help/api/user-manual.html) — 3 sec 간격 + User-Agent 권장 + 503 backoff 의무.
3. [HuggingFace Daily Papers API](https://huggingface.co/api/daily_papers) — 인증 0 + `date` query + `submittedOnDailyAt` 응답 필드.
4. [HuggingFace Papers API 비공식 문서 (0x0is1)](https://github.com/0x0is1/hf-papers-api-docs) — endpoint 3종 (`/api/papers/search` · `/api/papers/{id}` · `/api/daily_papers`) 정리.
5. [YouTube Data API v3 Quota Costs](https://developers.google.com/youtube/v3/determine_quota_cost) — search.list 100 / channels.list 1 / playlistItems.list 1 / videos.list 1.
6. [YouTube Data API getting started (10k unit/day)](https://developers.google.com/youtube/v3/getting-started) — 무료 한도 + API key 발급 절차.
7. [YouTube channel RSS Atom feed (Chuck Carroll)](https://chuck.is/yt-rss/) — `/feeds/videos.xml?channel_id=...` 공식 미공개 안정 endpoint.
8. [Bluesky API getAuthorFeed](https://docs.bsky.app/docs/api/app-bsky-feed-get-author-feed) — `https://public.api.bsky.app/xrpc/app.bsky.feed.getAuthorFeed`, 인증 0.
9. [Bluesky Rate Limits](https://docs.bsky.app/docs/advanced-guides/rate-limits) — IP당 3,000 req/5분 + AppView 관대.
10. [Mastodon RSS feeds (OpenRSS)](https://openrss.org/blog/mastodon-rss-feeds) — `https://{instance}/@{user}.rss` 자동 + public posts only.
11. [Karpathy Bear Blog (RSS)](https://karpathy.bearblog.dev/) — 거물 인사 블로그 RSS 사례 (2025-03 시작).
12. [Lilian Weng (Lil'Log)](https://lilianweng.github.io/) — OpenAI Applied AI Research Manager 블로그.
13. [Sebastian Raschka blog](https://sebastianraschka.com/) — ML Q&AI / Tabular-TX 한국어 SOTA 검증.
14. [TechCrunch RSS feed](https://techcrunch.com/feed/) — RSS 2.0 + WordPress + 시간당 다중 갱신.
15. [AI타임스 RSS feed (검증 2026-05-07)](https://www.aitimes.kr/rss/allArticle.xml) — 한국 매체 RSS 2.0 + 일 5~10건 갱신.
16. [THE AI RSS index](http://www.newstheai.com/rssIndex.html) — 조선미디어그룹 한국 AI 매체.
17. [JasonEtco/create-an-issue (GHA)](https://github.com/marketplace/actions/create-an-issue) — `update_existing: true` + `search_existing: open` dedupe 우월.
18. [feed-rs (Rust RSS/Atom parser)](https://crates.io/crates/feed-rs) — Atom 1.0 + RSS 1.0/2.0 + JSON Feed + xml-rs 스트리밍.
19. [losfair/sqlite-cache (Rust)](https://github.com/losfair/sqlite-cache) — TTL + per-key lock + thundering herd 방어.
20. [The Batch (DeepLearning.AI)](https://www.deeplearning.ai/the-batch/) — 주간 AI 큐레이션 뉴스레터 + Andrew Ng letter 패턴.
21. [arxiv-sanity-lite (Karpathy)](https://github.com/karpathy/arxiv-sanity-lite) — SVM+tfidf 추천 + tag-based 알고리즘.
22. [Awesome-LLM (Hannibal046)](https://github.com/Hannibal046/Awesome-LLM) — 메인테이너 PR 검토 + active maintenance 게이트.
23. [EU Copyright Directive Article 15 (Wikipedia)](https://en.wikipedia.org/wiki/Directive_on_Copyright_in_the_Digital_Single_Market) — press publishers' right + "very short extracts" 예외.
24. [EU AI Act 2026 + TDM exception (Norton Rose Fulbright)](https://www.insidetechlaw.com/blog/2025/12/machine-readable-opt-outs-and-ai-training-hamburg-court-clarifies-copyright-exceptions) — Hamburg 법원 2025-12 machine-readable opt-out 판결.
25. [Tabular-TX Korean LLM summarization (arXiv 2501.10487)](https://arxiv.org/html/2501.10487) — EXAONE 3.0 7.8B + Llama-3-Korean-Bllossom-8B Korean SOTA + journalist persona prompting.

---

## 18. v2.0 진입 시 추가로 작성/갱신할 문서

- `docs/adr/0060-trend-report.md` — 정식 ADR (현 Proposed → Accepted 갱신).
- `docs/research/phase-22p-trend-report-decision.md` — 6-section 결정 노트 (현존, 진입 시 cross-reference 갱신).
- `manifests/trends/bundle.json` 초기 빈 합본 + minisign keypair 사용 등록.
- 사용자 결정 7건 (위 §15) 답변 후 진입 가능.
- `docs/CURATION_GUIDE.md` 갱신 (Phase 21' 패턴 + trend report 큐레이터 매주 운영 매뉴얼).
- EULA v3 갱신 (§13.9 4 항목).

---

## 19. 다음 페이즈 인계

### 진입 조건
- v0.0.x ship + v1.x 안정화 종료.
- Phase 21' Trending Watcher 운영 1~2개월 경험 (큐레이션 흐름 검증).
- 사용자 명시 진입 신호 (v2.0 분기).
- ADR-0060 + 결정 노트 사용자 명시 승인.
- §15 결정 포인트 7건 답변.

### sub-phase 5단계 (결정 노트 §6)
- **22'.a** — trends-bundle schema + minisign 통합.
- **22'.b** — 큐레이터 GHA fetch + review queue.
- **22'.c** — registry-fetcher 호출 측 + cache.
- **22'.d** — 모델 게이트 + UI.
- **22'.e** — 운영 모니터링 + 큐레이터 가이드.

### 위험 — 결정 노트 §6 위험 매트릭스 8건 (큐레이터 부담 / RSS 부재 / SNS 윤리 / API key / propagate / 한국 콘텐츠 / 4B 한국어 / SQLite 증식).

---

**문서 버전**: v2 (2026-05-07 — 9 영역 풀 설계 보강 + 출처 25개로 확장 + 법적 분석 + OSS 사례 + 결정 포인트 7건). v0.0.x ship 후 v2.0 진입 시 본 노트 + ADR-0060 cross-reference 사용자 결정 후 진입.

# Phase 21' — Trending Watcher 보강 리서치 (엘리트 사례 종합)

> **목적**: 큐레이션 catalog 자동 갱신 — *발견 → deterministic 필터 → 큐레이터 GitHub Issue 알림 → 큐레이터 검토 PR → manifest 합류*. 외부 통신 0 + 큐레이션 정체성 + 한국어-우선 보존.
> **작성일**: 2026-05-06
> **결정 노트**: `phase-21p-trending-watcher-decision.md`
> **ADR**: `docs/adr/0059-trending-watcher.md`

---

## 1. HuggingFace Hub `/api/models` (BEST #1)

**확정 사실 (2026-05 OpenAPI spec)**:
- Endpoint: `GET https://huggingface.co/api/models?sort=trending&limit=N&pipeline_tag=<tag>` — `sort=trending` (alias `trending_score`).
- 응답 필드: `id`, `modelId`, `downloads`, `likes`, `trendingScore`, `pipeline_tag`, `tags`, `library_name`, `gguf` (object: 존재 시 GGUF metadata block), `gated` (`auto`/`manual`/`false`), `private`, `lastModified`, `cardData.license`, `safetensors.parameters` (사이즈 추정).
- **Rate limit (확정)**: 익명 IP당 **API 500 / Pages 100 / Resolvers 3,000 — 5분 fixed window**. Free token 1,000 / 5,000 / 200. `RateLimit-Policy` 헤더로 정책 advertised, 429 시 `RateLimit: r=…;t=…` retry-after.
- **ETag/If-Modified-Since는 Resolvers에만 적용** — 동적 search는 매 호출마다 fresh JSON.

**LMmaster 적용**:
- `reqwest` + 자체 캐시 (`huggingface_hub` Rust crate는 download/upload 중심으로 list API 약함).
- 1회 호출: `pipeline_tag=text-generation&library=gguf&sort=trending&limit=200` — `library=gguf` 필터로 portable 정체성 정확 매칭.
- 한국어 발굴: `?language=ko&sort=trending&limit=100` — `cardData.language` 메타 필요. 누락은 §8 본문 정규식 보완.
- 6h cron이면 호출 ≤10회/5분 윈도우, 익명 한도(500) 안전 마진 50배.

> **CLAUDE.md 정합**: 외부 통신 화이트리스트(huggingface.co)와 ADR-0019 익명 quota 합치, `gguf` 필드 검사로 chat template 깨짐 1차 차단.

---

## 2. Open LLM Leaderboard 2 (BEST #2 — 본 페이즈 핵심)

**완전 자동화 가능 — 외부 통신 0 정체성 정합**:
- Dataset: `open-llm-leaderboard/contents` — 4,580 rows, **Parquet**, 단일 `default` config + `train` split.
- 컬럼: `eval_name`, `Model`, `#Params (B)`, `Hub License`, `Hub ❤️`, `IFEval`, `BBH`, `MATH Lvl 5`, `GPQA`, `MUSR`, `MMLU-PRO`, `Average ⬆️`, `Submission Date`, **`Chat Template` (boolean — chat template 검증 핵심)**.
- Datasets Server API: `GET https://datasets-server.huggingface.co/rows?dataset=open-llm-leaderboard/contents&config=default&split=train&offset=0&length=100` — 토큰 불필요, 페이지네이션, 5분 윈도우 동일 quota.
- **continuously updated** — 새 평가 commit마다 갱신.

**LMmaster 적용**:
- 단일 endpoint 호출로 **벤치 점수 + license + chat template 유무 + likes**가 한 번에 → deterministic 룰의 ground truth.
- 추천 임계: `Average ⬆️ ≥ 30` + `Chat Template == true` + `#Params (B) ∈ [3, 14]` + `Hub License ∈ {apache-2.0, mit, llama3.x-community, gemma}` 화이트리스트.
- **Parquet 직접 fetch가 가장 효율** — `https://huggingface.co/datasets/open-llm-leaderboard/contents/resolve/main/data/*.parquet` (Resolver bucket → 익명 3,000/5분 여유). Rust는 `parquet` crate(arrow-rs).

> **CLAUDE.md 정합**: 큐레이션 thesis (deterministic only, LLM judge X)가 정확히 본 dataset 구조에 맞춤. ADR-0048 거부 사유 "LLM as judge bias" → 외부 leaderboard 합의에 위임.

---

## 3. LMSYS Chatbot Arena ELO

**공식 API 없음 → 두 경로**:
- **공식**: `lmarena-ai/arena-human-preference-100k` HF dataset. ELO 사전 계산본 X — 클라이언트 직접 통계 산출 필요. **2024 정체 데이터**(2026 신모델 미반영) → 부적합.
- **커뮤니티 미러 (권장)**: `oolong-tea-2026/arena-ai-leaderboards` (MIT, GitHub Actions 일일 갱신). 통일 schema = `{rank, model, vendor, license, score, votes}`. `https://raw.githubusercontent.com/oolong-tea-2026/arena-ai-leaderboards/main/data/latest.json` 단발 fetch — github.com 화이트리스트(ADR-0026) 정합.

**LMmaster 적용**: ELO ≥ 1100 보너스 가산 (Open LLM 평균과 결합). 한국어 모델 추적 약함 → 보조지표로만, primary는 §2/§4.

---

## 4. Korean Leaderboards

**활성도 매트릭스 (2026-05)**:

| Source | 상태 | 접근 방법 |
|---|---|---|
| **`HAERAE-HUB/KMMLU`** | 활성 (지속 갱신) | HF Datasets Server, 35,030 Q × 45 subjects, 한국 원문 출제 |
| **`HAERAE-HUB/KMMLU-HARD`** | 활성 | 동일 패턴 |
| **`upstage/open-ko-llm-leaderboard`** | Season 2 (2024-12~) but Space 504 timeout 빈번, requests dataset 마지막 commit 2024-03 | 불안정 — fallback only |
| `LGAI-EXAONE/KoMT-Bench` | EXAONE 자체 평가 | EXAONE 계열 cross-ref |

**LMmaster 적용**:
- 1차 source = **KMMLU 모델 카드의 self-reported score 정규식** — model card에 `KMMLU.*\d+\.\d+` 표 흔히 있음. 점수 ≥ 50 (KMMLU 사람 평균 62.6 대비 80% 이상)을 한국어 통과 임계로.
- Open Ko-LLM Season 2는 **존재 확인용**으로만.
- 신호 추가: `tags`에 `ko` / `korean` / `EXAONE` / `HyperCLOVA` 매칭.

---

## 5. Ollama Library (robots.txt 결론)

**확정**: `https://ollama.com/robots.txt` 는 **404 (파일 부재)**. RFC 9309상 *모든 user agent 모든 path 허용*으로 해석되지만, **Terms 페이지(/terms)가 별도 존재**해 ToS 측 제약 가능성 — scrape 자제 권장.

**대안 — GitHub mirror 패턴**:
- 공식 Library list API 부재 (`ollama/ollama#7751` feature request 미해결).
- 권장: `ollama/ollama` repo의 `docs/library.md` 또는 README scrape (github.com 화이트리스트 정합) + 24h 캐시.

**LMmaster 적용**: HF에서 발견한 모델의 `hub_id`를 `<author>/<name>` → `<name>` 정규화 후 Ollama library 페이지 hit 여부만 확인 (binary signal). 큐레이터에게 "Ollama mirror 존재함 ✓" 표시.

---

## 6. OpenRouter `/api/v1/models`

**확정 (실 endpoint 검증)**:
- **인증 불필요** — anonymous public.
- 필드: `id`, `canonical_slug`, `hugging_face_id` (★ HF 모델과 직결), `name`, `created`, `description`, `context_length`, `architecture`, `pricing`, `top_provider`, `supported_parameters`.

**LMmaster 적용 (제한적)**:
- *발견* 보조 신호로만. **본 페이즈에서는 PASS** — 외부 통신 화이트리스트에 `openrouter.ai` 추가는 가치 대비 정체성 훼손 큼.
- v1.x 후순위 이월 — `hugging_face_id`로 "이 모델이 cloud provider에서 서빙 중" 검증 신호 활용 가능성은 인정.

---

## 7. GitHub Issue 자동 생성 (BEST #3)

**조합 권장 (cost 0)**:

```yaml
# .github/workflows/trending-watcher.yml
on:
  schedule: [{cron: '0 */6 * * *'}]   # 6h
  workflow_dispatch:
jobs:
  scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo run -p trending-watcher --release > /tmp/report.md
      - uses: JasonEtco/create-an-issue@v2     # ← dedupe 우월
        env: {GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}}
        with:
          filename: /tmp/report.md
          update_existing: true
          search_existing: open
```

- **`peter-evans/create-issue-from-file`**: 단순 작성, dedupe **없음** — 거부.
- **`JasonEtco/create-an-issue`**: **`update_existing: true` + `search_existing: open`** — 동일 제목이 open이면 body 업데이트, 없으면 신규. **본 시나리오 정확히 일치**.

**LMmaster 적용**:
- 제목 fingerprint: `[trending] <hub_id> (<avg_score>)` — `hub_id`가 unique key.
- Body: 핵심 메타 + 룰 통과 내역 + manifest PR 체크리스트. 라벨 `auto-curate` + `needs-review` + assignee = 큐레이터.
- GitHub API 5,000/h authenticated → 6h cron + 모델 50개 = 부담 0.

---

## 8. Deterministic 필터 룰 매트릭스

```
가중 점수 = w1·norm(Open_LLM_Avg) + w2·log10(downloads_30d)
         + w3·korean_signal + w4·license_score + w5·gguf_present
가중치: w1=0.35, w2=0.20, w3=0.20, w4=0.15, w5=0.10
```

- **License score**: apache-2.0/mit = 1.0, llama3.x-community/gemma = 0.7, exaone-custom/nvidia-open = 0.4, **other = 0.0 (자동 제외)**.
- **Korean signal**: `cardData.language` `ko` = 1.0 / 본문 정규식 `(한국어|Korean|한글|EXAONE|HyperCLOVA|HCX)` hit count 0~3 → 0.3·count cap 1.0 / 미언급 0.0.
- **GGUF present**: 같은 author OR `unsloth|bartowski|lmstudio-community|TheBloke|MaziyarPanahi` 미러 검색 hit = 1.0.
- **사이즈 게이트**: `safetensors.parameters` 또는 GGUF metadata로 **3B~14B만**. 외이는 *info-only* 큐.
- **다운로드 임계**: `downloads_30d ≥ 1k` 1차, 추천 `≥ 10k`.
- **시간 가중**: `trendingScore` ≥ HF 트렌딩 percentile 80 — 절대 다운로드와 분리해 신생 모델 underdog 신호 보존.

> **CLAUDE.md 정합**: 모든 가중치/임계는 코드 상수 + 테스트 invariant (deterministic 100회 동일 결과). LLM judge 0.

---

## 9. 자동화 호스팅

**권장: 별도 repo `lmmaster-trending-watcher` (public, MIT)**.

| 옵션 | 장점 | 단점 |
|---|---|---|
| LMmaster 데스크톱 측 | 사용자 PC에서 즉시 큐레이션 알림 | 사용자 PC 켜져 있어야, secrets 관리 부담 |
| **별도 repo + GHA cron** | cost 0, 사용자 PC 무관, public audit-able, 기존 GitHub 통신 정체성 정합 | repo 1개 추가 운영 |
| Renovate-style SaaS | (해당 없음) | 외부 SaaS 의존 = cloud-zero 정체성 훼손 |

GHA cron `0 */6 * * *` (6h). `workflow_dispatch` 수동 트리거 옵션.

**산출물 흐름**:
```
GHA cron → fetch (HF + Open LLM dataset + Arena mirror + GitHub Ollama mirror)
        → deterministic filter (Rust binary, §8 매트릭스)
        → /tmp/report.md (한국어, §4.1 톤)
        → JasonEtco/create-an-issue (dedupe by 제목 fingerprint)
        → 큐레이터 review → manifest PR (lmmaster 본 repo)
```

---

## 10. 결정 포인트 (큐레이터/사용자 채택)

| # | 포인트 | 권장 (채택) | 거부 / 후순위 |
|---|---|---|---|
| 1 | 호스팅 위치 | **별도 repo `lmmaster-trending-watcher`** | LMmaster 데스크톱 내장 / SaaS |
| 2 | OpenRouter `openrouter.ai` 화이트리스트 | **PASS** (정체성 보존) | 추가 (가치 대비 ROI 낮음) |
| 3 | Korean primary | **KMMLU 모델 카드 정규식 + EXAONE/HyperCLOVA 정규식** | Open Ko-LLM Season 2 (불안정) |
| 4 | GitHub Action | **`JasonEtco/create-an-issue`** (dedupe 우월) | `peter-evans/create-issue-from-file` |
| 5 | Issue 라벨 | `auto-curate` + `trending-watcher` + `needs-review` 3종 + assignee 큐레이터 1인 | — |

---

## 11. 출처 (Sources)

- [HuggingFace Hub API](https://huggingface.co/docs/hub/api)
- [HuggingFace Rate Limits](https://huggingface.co/docs/hub/rate-limits)
- [Open LLM Leaderboard contents dataset](https://huggingface.co/datasets/open-llm-leaderboard/contents)
- [arena-ai-leaderboards (oolong-tea-2026)](https://github.com/oolong-tea-2026/arena-ai-leaderboards)
- [HAERAE-HUB/KMMLU](https://huggingface.co/datasets/HAERAE-HUB/KMMLU)
- [JasonEtco/create-an-issue (dedupe 패턴)](https://github.com/marketplace/actions/create-an-issue)
- [peter-evans/create-issue-from-file (비교)](https://github.com/peter-evans/create-issue-from-file)
- [arrow-rs / parquet Rust crate](https://crates.io/crates/parquet)

# ADR-0059 — Trending Watcher (deterministic 필터 + human review queue)

* **상태**: Proposed (2026-05-06). v1.x 진입 — 사용자 명시 승인 (옵션 1).
* **선행**:
  - ADR-0014 (Curated Model Registry) — *trending 소스 매트릭스* 명시 (HF/OpenRouter/Ollama scrape).
  - ADR-0019 (Always-Latest Hybrid Bootstrap) — HF trending 1h cache + ETag 약속. 부분 구현 (bootstrap 시점만).
  - ADR-0026 (Auto-Updater + 외부 통신 화이트리스트) — `huggingface.co` + `github.com` 도메인 한정.
  - ADR-0044 (Live Catalog Refresh) — 큐레이터 push → 사용자 fetch 인프라.
  - ADR-0048 (Intent + domain_scores) — *자동 벤치마크 실행 거부* 사유 동일 적용.
* **결정 노트**: `docs/research/phase-21p-trending-watcher-decision.md`
* **보강 리서치**: `docs/research/phase-21p-trending-watcher-reinforcement.md`

## 컨텍스트

ADR-0019에서 trending 모니터링 인프라가 약속됐지만 *상시 watcher + 큐레이터 알림*은 미구현. 사용자(2026-05-06)가 정당히 지적: "수동만으로는 가치 떨어진다". 단, 본 프로젝트 *큐레이션 정체성* (chat template 검증, 라이선스 함정 차단, 한국어 자연스러움 보장)을 보존하려면 **완전 자동 추가는 거부** + **자동 발견 + 큐레이터 review queue**가 정공.

핵심 충돌:
- 외부 통신 0 (ADR-0013) ↔ trending 발견은 외부 fetch 필요.
- 큐레이션 thesis ↔ "자동 추가" 욕구.

화해 — 본 ADR이 명시:

## 결정

### 1. 호스팅 — 별도 repo `lmmaster-trending-watcher` (public, MIT)

별도 GitHub repo + GHA cron `0 */6 * * *` (6h). 사용자 PC 무관, public audit-able, secrets 관리 분리, LMmaster 본 repo는 *PR 받는 측*만.

### 2. 외부 통신 화이트리스트 — 기존 도메인 유지

ADR-0026 `huggingface.co` + `github.com`만 사용. **`openrouter.ai` 추가 거부** — 가치 대비 정체성 훼손 큼. v1.x 후순위 이월.

### 3. 데이터 소스 매트릭스 (확정)

| Source | URL | 용도 |
|---|---|---|
| HF Trending | `huggingface.co/api/models?sort=trending&library=gguf&limit=200` | 발견 1차 |
| Open LLM Leaderboard 2 | `datasets-server.huggingface.co/rows?dataset=open-llm-leaderboard/contents` | 벤치 점수 + chat template + license ground truth |
| Arena Mirror | `raw.githubusercontent.com/oolong-tea-2026/arena-ai-leaderboards/main/data/latest.json` | LMSYS ELO 보조 |
| KMMLU | model card 정규식 `KMMLU.*\d+\.\d+` | Korean 1차 검증 |
| Ollama mirror | `raw.githubusercontent.com/ollama/ollama/main/docs/library.md` | 미러 존재 binary signal |

LMSYS 공식 API + Open Ko-LLM Season 2 + OpenRouter API: 본 페이즈 PASS (불안정 / 화이트리스트 추가 부담).

### 4. Deterministic 필터 가중치

```
score = 0.35·norm(Open_LLM_Avg)
      + 0.20·log10(downloads_30d)
      + 0.20·korean_signal
      + 0.15·license_score
      + 0.10·gguf_present
```

- **license_score**: apache-2/mit = 1.0, llama3.x-community/gemma = 0.7, exaone/nvidia-open = 0.4, *other = 0.0 (자동 제외)*.
- **korean_signal**: `cardData.language=ko` 1.0 / 본문 `(한국어|Korean|한글|EXAONE|HyperCLOVA|HCX)` 0.3·count cap 1.0 / 미언급 0.0.
- **gguf_present**: 같은 author OR `unsloth|bartowski|lmstudio-community|TheBloke|MaziyarPanahi` 미러 hit = 1.0.
- **사이즈 게이트**: 3B~14B만 정식 큐. 외이는 info-only.
- **다운로드 임계**: `≥ 1k` 1차, `≥ 10k` 추천.
- **시간 가중**: `trendingScore` percentile 80 = underdog 신호 보존.

LLM judge 거부 — ADR-0048 정책 그대로 (deterministic only).

### 5. 큐레이터 알림 — `JasonEtco/create-an-issue`

**dedupe 우월** (peter-evans 거부). `update_existing: true` + `search_existing: open` — 동일 `[trending] <hub_id>` 제목이 open이면 body 업데이트, 없으면 신규.

**Issue 메타**:
- 제목: `[trending] <hub_id> (Avg: <score>, Korean: <ko_signal>)` — fingerprint = `hub_id`.
- Body: 핵심 메타 + 룰 통과 내역 + manifest PR 체크리스트 (한국어).
- 라벨: `auto-curate` + `trending-watcher` + `needs-review`.
- Assignee: 큐레이터 1인 (`freessunky-bit`).

### 6. 큐레이터 검토 흐름

1. Issue 알림 → 큐레이터 검토 (chat template 한국어 발화 검증, 라이선스 약관 정밀 확인, GGUF 변종 sha256).
2. 통과 → manifest 초안 PR (LMmaster 본 repo). **completely 자동 PR은 거부** — 사람 검토가 큐레이션 정체성.
3. PR merge 후 `node .claude/scripts/build-catalog-bundle.mjs` 실행 → catalog.json 갱신 (CLAUDE.md §3 흐름 준수).
4. jsdelivr propagate → 사용자 카탈로그에 노출.

### 7. 본 repo 영향 — 0 (외부 동작만)

LMmaster 본 repo (`freessunky-bit/lmmaster`)는 PR 받는 측. trending watcher 코드는 별도 repo. 데스크톱 앱 변경 0.

## 근거

- **별도 repo**: 사용자 PC 무관 + secrets 분리 + 큐레이션 흐름 단순화. LMmaster 본 repo의 PR 검토 부담만 추가.
- **JasonEtco/create-an-issue dedupe**: 동일 모델이 6h마다 발견돼도 issue 1개로 수렴. 큐레이터 알림 noise 0.
- **Open LLM Leaderboard 2 핵심**: chat template boolean이 *큐레이션 1차 게이트* — chat template 깨진 모델은 사용자 첫 인상 망가짐. ADR-0014 큐레이션 thesis 정확히 매칭.
- **KMMLU 1차 + 정규식 보조**: Open Ko-LLM Season 2 불안정 → 신뢰성 낮음. KMMLU dataset은 안정 + 모델 카드 self-report 정규식이 신호로 충분.
- **GGUF library 필터**: portable runtime 정체성 정확 매칭. safetensors-only 모델은 사용자 PC 즉시 사용 X.

## 거부된 대안

1. **완전 자동 manifest PR** — 큐레이션 정체성 와해. chat template 깨짐 / 라이선스 함정 / 한국어 자연스러움 미검증 사용자 부담 위험. 인간 검토는 의도적 게이트.
2. **LMmaster 데스크톱에 watcher 내장** — 사용자 PC가 켜져 있어야 + secrets 관리 부담 + 사용자별 배포본 추적 부담.
3. **OpenRouter 화이트리스트 추가** — `openrouter.ai` 추가 = 신규 ADR + 외부 통신 정체성 훼손. `hugging_face_id` 신호 가치는 인정하나 본 페이즈 PASS, v1.x 후순위.
4. **Open Ko-LLM Season 2 primary** — Space 504 timeout + 마지막 commit 2024-03 → 신뢰성 낮음. KMMLU + 정규식이 정합.
5. **LMSYS 공식 dataset (`lmarena-ai/arena-human-preference-100k`)** — 2024 정체 데이터. 커뮤니티 미러(GitHub raw) 우월.
6. **Renovate / Dependabot 통째 차용** — dependency 갱신용. 큐레이션 review queue 패턴엔 부분 차용만 (issue dedupe).
7. **`peter-evans/create-issue-from-file`** — dedupe 부재. JasonEtco 우월.
8. **HF Token 사용** (rate limit 1k/5min ↑) — 익명 500/5min으로 충분 + token 누출 위험 + 사용자 보안 부담.
9. **자동 벤치마크 실행 (LMmaster가 사용자 PC에서 MMMU 등 돌림)** — ADR-0048 거부 그대로. 외부 leaderboard 합의 인용이 외부 통신 0 정체성과 정합.
10. **LLM judge 가중** — deterministic only 정체성 위반. score 100% 코드 상수.
11. **Ollama Library 직접 scrape** — robots.txt 부재이지만 ToS /terms 별도 존재 → GitHub mirror 우월.
12. **별도 repo MIT가 아닌 Apache-2** — MIT가 *발견 도구*에 더 단순. 본 repo는 MIT/Apache-2 dual 유지.

## 결과 / 영향

### 신규 산출물 (별도 repo)
- `lmmaster-trending-watcher/` (public, MIT)
  - `src/` — Rust binary `trending-watcher`. fetch + filter + report.md 출력.
  - `.github/workflows/scan.yml` — 6h cron + JasonEtco/create-an-issue.
  - `tests/` — deterministic invariant 테스트 (가중치 round-trip, 임계 boundary).
  - `README.md` — 한국어 + 영어 큐레이터 가이드.

### LMmaster 본 repo 영향
- 0 코드 변경. PR 받는 측만.
- `docs/CURATION_GUIDE.md` 갱신 — trending watcher issue → manifest PR 흐름 1단락 추가.

### 외부 통신 영향
- 화이트리스트 변경 없음 (`huggingface.co` + `github.com`).
- 별도 repo의 GHA runner가 fetch — 사용자 PC는 *결과물(catalog.json bundle)*만 fetch.

### 큐레이션 운영 영향
- 큐레이터 부담: 6h 마다 0~5 issue 검토. 1주 예상 5~10 모델 신규 검토.
- chat template 검증 + 한국어 발화 직접 테스트는 큐레이터 책임 (의도적).

## 테스트 invariant (별도 repo)

1. **Deterministic** — 동일 입력(JSON snapshot) → 동일 score 100회 반복.
2. **License whitelist** — `other` license는 score=0 + 큐 제외.
3. **사이즈 게이트** — `< 3B` or `> 14B` info-only 큐로 분리.
4. **GGUF gate** — `library_name != gguf` + 미러 hit 0이면 큐 제외.
5. **Issue dedupe** — 같은 `hub_id` 두 번 발견 시 issue 1개 (update_existing 동작).
6. **Korean signal boundary** — 정규식 0 hit 시 0.0, 3+ hit 시 1.0 cap.
7. **Empty fetch** — HF 응답 빈 배열 → graceful skip + warning.
8. **Rate limit** — 429 응답 시 RateLimit-Retry-After 헤더 honor.

## 다음 단계

1. **결정 노트 6-section** + 본 ADR과 짝지어 sub-phase 분할 (Phase 21'.a~e).
2. **별도 repo 신설** (사용자 결정 필요): GitHub `freessunky-bit/lmmaster-trending-watcher`.
3. **첫 GHA 동작 후 1주 모니터링** — 큐레이터 알림 noise / dedupe / 발견 정확성 평가.
4. **v1.x 후순위 후보**: OpenRouter `hugging_face_id` 신호 활용, Open Ko-LLM Season 2 안정화 시 fallback 추가, KMMLU 자체 평가 (자동 벤치마크 — ADR-0048 재평가).

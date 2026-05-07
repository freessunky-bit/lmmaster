# Phase 22'.c — Trends.tsx Display 통합 결정 노트

> **상태**: 채택 (2026-05-08, v0.3.1 직후)
> **선행 의존성**: Phase 22'.a (trends-bundle-curator GHA cron) + Phase 22'.b (trend-summarizer + Trends.tsx prototype). 첫 큐레이션 5건 push 완료 (`commit 65fb341`).
> **다음 페이즈**: 사용자 결정.
> **결정 일자**: 2026-05-08

---

## 1. 결정 요약

`manifests/apps/trends-bundle.json`의 실 큐레이션 5건을 `Trends.tsx`에 카드 그리드로 표시. 기존 `MOCK_CARDS` placeholder 6 카테고리는 "다음 주 출처 가이드"로 보존. ADR-0060 §C 흡수 (별도 ADR 없음).

| 변경 | 영역 | Effort |
|---|---|---|
| 신규 "이번 주 큐레이션" 섹션 | `Trends.tsx` (gate end + cards section 사이) | 2h |
| `bundleItems` type 확장 | `TrendBundleItem extends SummaryInput` | 0.5h |
| i18n `trends.bundle.*` 3키 ko/en | i18n parity 1060 → 1066 | 0.5h |

---

## 2. 채택안

### 2.1 신규 섹션 — "이번 주 큐레이션"

`Trends.tsx`의 gate end + 기존 cards section 사이에 신규 섹션 추가:
- 조건부 렌더 — `bundleItems.length > 0`.
- 헤딩 + curator_note_ko intro 카피 + 카드 그리드.
- 각 item 카드:
  - 카테고리 칩 (`Icon` + `kind`)
  - 제목 (`item.title`)
  - 한국어 요약 (`item.summary_ko`)
  - 메타 — `item.source` + `item.published_at`
  - 출처 — `item.attribution`
  - source_url — `<code>` plain text (클릭 v0.3.3 deferred)

### 2.2 type 확장

`bundleItems`의 `attribution` / `published_at` 등 필드는 SummaryInput에 없음 — 본 sub-phase에서 type 확장:

```typescript
type TrendBundleItem = SummaryInput & {
  attribution?: string;
  published_at?: string;
  tags?: string[];
  score?: number;
};

const bundleItems = (trendsBundleData.items ?? []) as TrendBundleItem[];
```

`summarizeTrends(bundleItems, false)` 호출은 그대로 — TrendBundleItem이 SummaryInput에 호환.

### 2.3 i18n 키 추가

`trends.bundle.{heading, publishedAt, attribution}` ko/en parity. 기존 `trends.cards.*` 보존. ko/en 1060 → 1066.

---

## 3. 기각안 + 이유 (negative space)

| # | 거부된 대안 | 사유 |
|---|---|---|
| 1 | **source_url 클릭 가능 (Tauri shell.open)** | capability scope `shell:allow-open`에 arxiv.org / blog 도메인 추가 부담. 첫 5건 모두 arXiv지만 향후 다양화. v0.3.3 별도 sub-phase |
| 2 | **placeholder MOCK_CARDS 제거** | bundleItems 5건 모두 arXiv paper. 6 카테고리(blog/news/video/sns/github)는 비어 있음 — MOCK_CARDS는 "출처 가이드"로 보존이 정합 |
| 3 | **summarize 자동 호출** | LLM 호출은 자원/시간 — 사용자 명시 클릭 (기존 `handleSummarize` 패턴 보존) |
| 4 | **bundleItems 만료 처리 (`expires_at`)** | 첫 사이클이라 만료 케이스 미발생. v0.3.3 또는 큐레이션 사이클 누적 후 |
| 5 | **카테고리 필터 (paper/blog/...)** | bundleItems가 다양해진 후. 현재 5건 모두 paper라 필터 가치 0 |
| 6 | **TrendBundleItem을 ipc/trends.ts에 정의** | 본 type은 *frontend display-only*. ipc/trends.ts는 Rust mirror라 분리 정합. v0.4.x에서 Rust 측 schema 확장 시 통합 |

---

## 4. 미정 / 후순위 이월 (v0.3.3 / v0.4.0)

| 항목 | 진입 조건 |
|---|---|
| **source_url 클릭 가능** | capability scope arxiv.org 추가 + Tauri shell.open 호출 — v0.3.3 |
| **bundleItems 만료 (`expires_at`) 처리** | 첫 큐레이션 사이클 만료 후 (2026-05-15) |
| **카테고리 필터** | bundleItems 다양화 후 |
| **score-based 정렬** | 현재 모든 score 0.0 — 큐레이션 가중치 활성 후 |
| **TrendBundleItem Rust 측 schema 확장** | trend-summarizer의 SummaryInput 확장 시 — v0.4.x |

---

## 5. 테스트 invariant

| invariant | 위치 |
|---|---|
| `bundleItems.length > 0` 시 "이번 주 큐레이션" 섹션 노출 | Trends.test.tsx (있다면) 또는 vitest snapshot |
| 각 item 카드에 title + summary_ko + source + source_url 포함 | 위 |
| curator_note_ko intro 카피 노출 | 위 |
| `bundleItems.length === 0` 시 섹션 미렌더 | 위 |
| i18n parity (`check-i18n-parity.mjs`) 1060 → 1066 통과 | CI |

vitest 추가 0건 (기존 Trends.test.tsx 존재 시 미세 fixup; 없으면 별도 sub-phase).

---

## 6. 다음 페이즈 인계

- v0.3.3 (또는 v0.4.0): source_url 클릭 가능 + capability scope 확장.
- 큐레이션 만료 처리 + 카테고리 필터.
- Rust 측 SummaryInput schema 확장 (attribution / published_at / tags / score 필수화).
- score-based 정렬 활성.

### 위험 노트
- `(item as any).published_at` 같은 unsafe cast는 type 확장으로 회피.
- bundleItems의 source_url이 빈 문자열일 가능성 — 체크 추가 (`<p>{item.source_url}</p>` 조건부).
- curator_note_ko 누락 시 섹션 헤딩만 노출 — graceful.

---

**문서 버전**: v1.0 (2026-05-08, Phase 22'.c 1차 작성).

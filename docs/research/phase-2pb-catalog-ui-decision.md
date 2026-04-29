# Phase 2'.b — React 카탈로그 UI 결정 노트

> 작성일: 2026-04-27
> 상태: 확정 (보강 리서치 후 7가지 디자인 결정 + 미확정 2건 결정)
> 선행: `phase-2pa-catalog-decision.md` (데이터 모델·Recommender)

## 0. 결정 요약 (7가지)

1. **추천 패널은 카탈로그 그리드 위 sticky 4-슬롯 가로 스트립** — Best는 강조 보더(네온 그린 1.5px + 외곽 glow), 나머지 3슬롯(Balanced/Lightweight/Fallback)은 secondary 보더(`--border-strong`) 동급. 좁은 폭(<960px)에서 2×2 폴드.
2. **카드 정보 우선순위 3행 고정**:
   - 1행: `display_name` + `verification` 배지 + `maturity` 배지.
   - 2행: 카테고리 한국어 라벨 + `language_strength` 별점 + 핵심 사용처 1줄 (`use_case_examples[0]`).
   - 3행: VRAM/RAM/install_size 3-메트릭 mono numeric + 호환 hint chip.
3. **비호환/excluded 모델은 숨기지 않고 dimmed + reason chip**으로 노출 — Foundry Local 패턴. "내 PC엔 안 맞지만 어떤 게 있는지" 신뢰 형성.
4. **레이아웃**: 좌측 sidebar(검색+카테고리, 240px) + 우측 main(추천 strip + 필터 chips + 그리드).
5. **Quantization 선택은 Drawer 내부**에만 — 카드는 권장 quant 1개만 size 표시.
6. **한국어 카피 톤** — 액션 버튼만 해요체 동사("설치할게요"), 메타 데이터는 명사구("VRAM 권장"). 의문문 금지.
7. **키보드 우선** — Tab 흐름 + Cmd/Ctrl+K palette + Esc로 Drawer close + `:focus-visible` 2-layer ring.

## 1. 미확정 → 확정

- **Best 슬롯이 None일 때**: 빈 상태 명시("이 PC에서 권장할 모델이 없어요. 가벼운 옵션부터 시도해 볼래요?") + Fallback 슬롯은 그대로 유지. Best 자리에 안내 텍스트 + Fallback CTA 강조. (자동 승격은 사용자에게 false positive 줄 수 있어 기각.)
- **"추천만" toggle 기본값**: OFF — power user가 카탈로그 전체를 볼 수 있어야 함. 추천 strip이 이미 상단에 있어 novice도 진입 즉시 추천을 봄. ON으로 하면 카탈로그가 비어 보이는 첫인상 위험.

## 2. 정보 아키텍처 (페이지 레이아웃)

```
┌─────────────────────────────────────────────────────────┐
│ Aurora background                                       │
│ ┌─────────────────────────────────────────────────────┐ │
│ │ Header: 타이틀 "모델 카탈로그" + Cmd+K hint           │ │
│ ├──────────┬──────────────────────────────────────────┤ │
│ │ Sidebar  │ Recommendation Strip (sticky)            │ │
│ │ (240px)  │ ┌────┬────┬────┬────┐                     │ │
│ │ 검색     │ │Best│Bal │Lt  │Fall│                     │ │
│ │ 카테고리 │ └────┴────┴────┴────┘                     │ │
│ │ 6 라디오 │ Filter chips (tool/vision/structured/추천만)│ │
│ │          │ Sort: [추천 점수 ▾]                       │ │
│ │          │ ┌────┬────┬────┐                          │ │
│ │          │ │Card│Card│Card│ (dim if excluded)        │ │
│ │          │ └────┴────┴────┘                          │ │
│ └──────────┴──────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

## 3. 컴포넌트 트리

```
<CatalogPage>
  <PageHeader />
  <CatalogShell>
    <CatalogSidebar>
      <SearchInput />
      <CategoryNav /> {/* 6 라디오 */}
    </CatalogSidebar>
    <CatalogMain>
      <RecommendationStrip>
        <RecommendationSlot variant="best" />
        <RecommendationSlot variant="balanced" />
        <RecommendationSlot variant="lightweight" />
        <RecommendationSlot variant="fallback" />
      </RecommendationStrip>
      <FilterBar /> {/* tool/vision/structured/추천만 + Sort */}
      <CatalogGrid>
        <ModelCard /> × N
      </CatalogGrid>
    </CatalogMain>
    <ModelDetailDrawer /> {/* portal */}
  </CatalogShell>
</CatalogPage>
```

## 4. v1 범위 / 후순위

**v1 (Phase 2'.b)**:
- CatalogPage + RecommendationStrip + ModelCard.
- CategoryNav (6 라디오) + 검색 (display_name 부분 일치).
- FilterBar — tool/vision/structured/추천만 4 chip + sort 3개 (추천/한국어/크기).
- ModelDetailDrawer — quant_options 라디오 + warnings 표시 + use_case_examples 전체.
- i18n `catalog.*` ko/en.
- 라우팅은 단순 state 토글 (Phase 4에서 react-router 도입).
- 테스트: vitest-axe + render + 카드 클릭 → invoke('get_recommendation') mock.

**v1.1 / 후순위**:
- HF meta (downloads/likes/last_modified) 카드 footer.
- 카드 grid virtualization (50+ 시).
- 카드 비교 모드 (2~3개 spec diff).
- 설치 시점 quant select dialog (Pinokio 패턴).
- 다국어 (en) 자연스러운 카피.

## 5. 테스트 기준

- **렌더 테스트**: 카탈로그 mount → 8개 카드 + 4 슬롯 + 6 카테고리.
- **상호작용**: 카테고리 클릭 → 그리드 필터링 / 카드 클릭 → Drawer 열림 / Esc → 닫힘.
- **접근성**: vitest-axe `toHaveNoViolations` + Tab 흐름 검증 + `prefers-reduced-motion` 가드.
- **Excluded 표시**: dim opacity 0.5 + reason chip 텍스트 "VRAM 8GB 필요" 등.
- **Recommendation strip**: Best=None일 때 빈 상태 메시지 표시.

## 6. 한국어 카피 키 (i18n catalog.*)

```jsonc
{
  "catalog.title": "모델 카탈로그",
  "catalog.subtitle": "내 PC에 맞는 모델을 골라보세요",
  "catalog.search.placeholder": "모델 이름 검색",
  "catalog.category.all": "전체",
  "catalog.category.agent-general": "일반 어시스턴트",
  "catalog.category.roleplay": "롤플레이",
  "catalog.category.coding": "코딩",
  "catalog.category.slm": "소형 모델",
  "catalog.category.sound-stt": "음성 인식",
  "catalog.category.sound-tts": "음성 합성",
  "catalog.filter.tool": "도구 호출",
  "catalog.filter.vision": "비전",
  "catalog.filter.structured": "구조화 출력",
  "catalog.filter.recommendedOnly": "추천만",
  "catalog.sort.score": "추천 점수순",
  "catalog.sort.korean": "한국어 강도순",
  "catalog.sort.size": "설치 크기 작은순",
  "catalog.empty.noMatch": "조건에 맞는 모델이 없어요. 필터를 조정해 볼래요?",
  "recommendation.best.label": "이 PC에 가장 좋아요",
  "recommendation.balanced.label": "균형이 좋아요",
  "recommendation.lightweight.label": "가볍게 써볼래요",
  "recommendation.fallback.label": "확실하게 돌아가요",
  "recommendation.empty.best": "이 PC에서 권장할 모델이 없어요. 가벼운 옵션부터 시도해 볼래요?",
  "model.metric.vram": "VRAM 권장",
  "model.metric.ram": "RAM 권장",
  "model.metric.size": "설치 크기",
  "model.maturity.stable": "Stable",
  "model.maturity.beta": "Beta",
  "model.maturity.experimental": "Experimental",
  "model.maturity.deprecated": "Deprecated",
  "model.verification.verified": "검증됨",
  "model.verification.community": "커뮤니티",
  "model.compat.fit": "내 PC와 잘 맞아요",
  "model.compat.tight": "메모리 빠듯해요",
  "model.compat.exceeds": "여유 있게 돌아가요",
  "model.compat.unfit": "이 PC에선 못 돌려요",
  "model.exclude.insufficientVram": "VRAM {{need}} 필요 (현재 {{have}})",
  "model.exclude.insufficientRam": "RAM {{need}} 필요 (현재 {{have}})",
  "model.exclude.deprecated": "더 이상 권장하지 않아요",
  "model.cta.details": "자세히 볼래요",
  "model.cta.install": "설치할게요",
  "drawer.section.quant": "양자화 옵션",
  "drawer.section.warnings": "주의 사항",
  "drawer.section.useCases": "주요 사용처",
  "drawer.close": "닫기"
}
```

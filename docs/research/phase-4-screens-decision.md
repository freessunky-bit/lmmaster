# Phase 4 — 9 한국어 UX 화면 + Korean preset 100+ 결정 노트

> 작성일: 2026-04-27
> 상태: 보강 리서치 완료 → 설계 확정 (sub-phase 분할 직전)
> 선행: Phase 0 (셸 + 토큰), Phase 1A (마법사 + InstallProgress), Phase 2'.a~c (카탈로그 + 추천 + 30초 벤치), Phase 3' (게이트웨이 + ApiKeysPanel + WorkspaceRepairBanner)
> 후행: Phase 4.5' (Knowledge Stack RAG — 동일 IA에 사이드 슬롯 추가), Phase 5' (워크벤치 v1 — Phase 4의 placeholder를 본 페이지로 승격), Phase 6' (자동 갱신/Gemini/STT/TTS — Phase 4 settings 화면이 옵션 슬롯을 미리 마련)
> 관련 ADR: ADR-0006 (디자인 시스템), ADR-0014 (카탈로그 거버넌스), ADR-0017 (manifest), ADR-0022 (게이트웨이 라우팅 + scoped key). 본 페이즈는 새 ADR 없이 진행 — 모든 결정이 기존 ADR 영역 안에 있음.

---

## 0. 결정 요약 (10가지)

1. **6 신규 화면 = 6 sub-phase로 분할 (4.a~4.h, 8 sub-phase 총)** — install / runtimes / projects / workbench(placeholder) / diagnostics / settings 6개 신규 + 공통 컴포넌트(StatusPill / VirtualList) 추출 + Korean preset 매니페스트 적재. 각 sub-phase는 독립 빌드 + 테스트 + RESUME 갱신을 산출하는 단위.
2. **information architecture 4축으로 통일 — Linear 패턴** — 각 화면은 `<topbar 한 줄 요약 + status pill> + <좌측 컨텍스트(필터/카테고리/액션) + 우측 main(목록/카드/그래프)>` 2-pane 또는 single-pane. 카탈로그가 이미 좌측 sidebar + 우측 main 패턴을 정착시켰으므로 같은 grid 토큰을 모든 화면에 재사용.
3. **공통 `<StatusPill>` 컴포넌트 추출 — Tailscale 패턴** — 게이트웨이 / Ollama / LM Studio / 모델 로드 상태 / scan 결과 모두 같은 4 상태(`booting | listening | failed | stopping`) + dot 색 + 라벨 + 보조 num(port / latency / size). 현재 Home / App.tsx에 인라인된 pill markup을 단일 컴포넌트로 합치고 a11y `role="status"` + `aria-live="polite"` 통일.
4. **공통 `<VirtualList>` 컴포넌트 추출 — `@tanstack/react-virtual`** — 24px row + sticky group header + smooth scroll. 카탈로그(50+ 모델), Korean preset(100+), 키 목록(50+), 진단 로그(수백+) 모두 사용. react-window는 row가 stale일 때 hook re-rendering 패턴이 부자연스러워 거부.
5. **Korean preset = 7 카테고리 × 14~16 = 100+ — JSON 매니페스트** — 코딩 / 번역 / 법률 / 마케팅 / 의료 / 교육 / 리서치. `manifests/presets/{category}/{slug}.json` 형식. 시스템 프롬프트는 한국어만 (영어 fallback 거부 — Korean-first 정책 위반). 추천 모델은 카탈로그 entry id로 link해 두고 deterministic dispatch.
6. **install 화면 = 마법사 Step3Install 메인 진입 통합** — 마법사 1회 외에도 재설치 / 업그레이드 / 다른 런타임 추가 시 진입. 현재 `InstallProgress` 컴포넌트를 메인 화면에서 reuse + 카드 그리드 (Ollama / LM Studio) + 진행 패널을 같은 페이지에 배치. drag-and-drop 모델 GGUF 인입은 v1 거부 (Phase 5' 워크벤치로 이월).
7. **runtimes 화면 = 어댑터 status + 모델 목록 + start/stop/reload** — `RuntimeManager.list()` 결과를 풀 표시. 어댑터별 카드 (status pill + 모델 수 + 마지막 ping) + 클릭 시 우측 drawer로 모델 목록(virtual list) + per-model unload 버튼. 모델 로딩은 chat 호출 시 자동이라 명시 load 버튼은 v1 거부.
8. **projects 화면 = 키 + 그 키와 연관된 모델 + 사용량 그룹화** — ApiKeysPanel(keys 화면)과의 차별점은 "scope 기준으로 묶어 보여주기". keys 화면은 키 단위 CRUD, projects 화면은 alias로 묶인 키 + 그 origin이 호출한 model 목록 + 24h request 카운트 + scoped 모델 패턴 매핑 시각화. Phase 6' team mode 진입 시 동일 카드에 "팀원" 슬롯 추가 가능한 구조로 설계.
9. **workbench 화면 = "곧 만나요" coming soon placeholder + 관심 표시 액션** — Phase 5' 진입 전까지 본격 UI 금지. 단 사용자가 빈 화면을 보고 "기능이 없는 줄" 오해하지 않도록 Phase 5' 5단계 플로우(데이터 → 양자화 → LoRA → 검증 → 등록) 미리보기 일러스트 + "준비되면 알려드릴게요" 토글 (notification preference DB에 저장). 빈 화면 거부.
10. **diagnostics 화면 = scanner + bench + workspace fingerprint + 게이트웨이 헬스 단일 페이지** — 4 섹션 grid (좌상: 자가스캔 요약, 우상: 게이트웨이 헬스, 좌하: 벤치 보고서 목록, 우하: 워크스페이스 fingerprint + repair history). 로그 export(.zip)는 v1.1 이월. settings 화면은 언어 / 테마(dark-only 자물쇠) / 워크스페이스 경로 / 카탈로그 갱신 / Gemini opt-in (toggle만, 실 동작은 Phase 6') / STT-TTS 슬롯(disabled 표시).

---

## 1. 채택안

### 1.1 6 화면 정보 아키텍처 (각 화면 IA + 핵심 컴포넌트 + a11y)

각 화면은 같은 골조(`<header><nav-pill><main-grid>`)를 공유한다. 공통 컴포넌트를 먼저 정의한다.

#### 공통 컴포넌트 시그니처

```ts
// packages/design-system/src/react/StatusPill.tsx
export type PillStatus = "booting" | "listening" | "failed" | "stopping" | "idle";
export interface StatusPillProps {
  status: PillStatus;
  label: string;             // i18n 처리 후 전달
  detail?: string | null;    // 보조 num / 에러 origin
  size?: "sm" | "md" | "lg"; // sidebar(sm) / hero(lg) / header(md)
  ariaLabel?: string;
}
export function StatusPill(props: StatusPillProps): JSX.Element;

// packages/design-system/src/react/VirtualList.tsx
export interface VirtualListProps<T> {
  items: T[];
  rowHeight: number;          // 24 default
  overscan?: number;          // 8 default
  renderRow: (item: T, index: number) => JSX.Element;
  keyOf: (item: T) => string;
  groupBy?: (item: T) => string;            // 옵션 — sticky group header
  groupHeader?: (group: string) => JSX.Element;
  emptyState?: JSX.Element;
}
export function VirtualList<T>(props: VirtualListProps<T>): JSX.Element;
```

`StatusPill`은 `data-status` attribute로 토큰(`--pill-bg-{status}`, `--pill-dot-{status}`)을 swap. CSS는 `tokens.css`에 추가. `VirtualList`는 `@tanstack/react-virtual`의 `useVirtualizer`를 wrap, `scrollMargin` 처리로 sticky header 지원.

#### install 화면

- **IA**: `<topbar: "런타임을 받고 있어요" + global StatusPill 합산>`, single-pane 카드 그리드 + 하단 진행 영역.
- **카드 (Ollama / LM Studio 2종)**: 각 카드는 `<header(name + license badge + 상태 pill)>` `<body(reason 1줄)>` `<footer(받기 / 재설치 / 자세히 / 폴더 열기 4 액션)>`. 이미 설치된 런타임은 "준비됐어요" 표시 + "재설치" 액션 노출. 카드 click 시 우측 drawer로 manifest detail.
- **하단 진행 패널**: 마법사의 InstallProgress를 reuse — 다만 `compact` prop 추가해 마법사 모드(전체 화면)와 메인 화면 모드(접힘 가능 inline) 구분. progress event는 같은 install-events.ts 채널.
- **빈 상태**: 둘 다 설치 완료된 경우 "두 런타임 모두 준비됐어요. 카탈로그에서 모델을 받아볼까요?" + 카탈로그 이동 CTA.
- **a11y**: 진행 영역은 `aria-live="polite"`. 카드 button은 `aria-pressed` 대신 status를 별도 텍스트로 announce.

#### runtimes 화면

- **IA**: `<topbar: 어댑터 합산 status>`, 좌측 어댑터 카드 column (sm) + 우측 모델 목록 (virtual list).
- **어댑터 카드** (Ollama / LM Studio): header(name + StatusPill + port) + body(loaded models 수, last ping ago) + footer(stop / restart / 로그 보기). stop은 confirm modal — 게이트웨이가 그 어댑터를 사용 중이면 "이 런타임을 끄면 어떤 모델 호출이 끊겨요" 경고.
- **우측 모델 목록**: 선택된 어댑터의 모델만 24px row VirtualList. 컬럼: name (mono) | size num | quant | loaded(boolean dot) | actions. 검색박스 + 정렬(name/size/loaded). 모델 로드는 chat 호출 시 자동이라 explicit load 버튼 거부. unload는 향후 v1.1 이월(LM Studio는 unload API 없음).
- **빈 상태**: "어떤 런타임도 실행 중이 아니에요. 설치 센터에서 시작해 주세요" + install 화면 이동.
- **a11y**: 어댑터 카드는 `<article role="region" aria-labelledby>`. virtual list row는 `role="row"`.

#### projects 화면

- **IA**: `<topbar: 활성 키 카운트>`, 좌측 project 카드 그리드 (md) + 우측 detail drawer (선택 시).
- **project 카드 = 같은 alias prefix를 가진 키 그룹** (예: alias "내 블로그"가 여러 origin 가지면 같은 카드). header(alias + origin chip 목록) + body(허용 모델 패턴, 24h request 수, 가장 많이 호출된 모델 top 3) + footer(키 발급 / 회수 / 사용량 차트 펼치기).
- **사용량 차트** (펼쳤을 때): 24h sparkline (request 수) + per-model 비율 bar. 데이터는 Phase 3' 게이트웨이가 SQLite에 적재한 access log를 IPC로 read-only 가져오기. v1은 24h만 (7d/30d 이월).
- **ApiKeysPanel과의 차이**: keys 화면 = 키 발급 + 회수의 CRUD UI (기존 그대로 보존). projects 화면 = 그 키들을 origin 묶음으로 보여주는 dashboard. 둘은 데이터 source 동일이지만 navigation 분리.
- **a11y**: 사용량 차트는 SVG + `aria-label`로 textual 요약 동시 제공.

#### workbench 화면

- **IA**: `<topbar: "워크벤치 준비 중">`, single-pane hero (이미지 + 1줄 카피 + 알림 받기 토글) + 하단 5단계 플로우 미리보기 cards (disabled 상태).
- **5단계 미리보기**: 데이터 인입 / 양자화 / LoRA / 검증 / 등록. 각 카드 disabled, hover 시 "Phase 5'에서 만나요" tooltip.
- **알림 받기**: 토글 click 시 settings DB의 `notify_on_phase5_release: true` 저장. 출시 시 toast로 안내.
- **거부**: 부분 동작 (양자화만 풀어서 노출 등) 거부 — 5단계 플로우의 가치는 통합이라 일부만 노출하면 사용자 신뢰 하락.
- **a11y**: hero illustration은 `role="img" aria-label="..."`.

#### diagnostics 화면

- **IA**: `<topbar: 종합 헬스 score (green / yellow / red)>`, 4 섹션 grid (좌상 / 우상 / 좌하 / 우하).
- **좌상 — 자가스캔**: scanner crate의 `summary_korean` + 항목별 카드(OS / RAM / GPU / runtime). "다시 점검" 버튼.
- **우상 — 게이트웨이 헬스**: gateway StatusPill + 60초 latency sparkline (가능 시) + 활성 키 수 + 마지막 5개 request log (mono row).
- **좌하 — 벤치 보고서**: 가장 최근 측정한 모델 5개의 token/sec 막대 차트 + 바로가기 모델 카드. 새 측정 시작 버튼이 카탈로그 모델 선택 화면으로 navigate.
- **우하 — 워크스페이스**: fingerprint(os/arch/cpu/gpu) + 마지막 repair 결과 + repair history 테이블 (날짜 / tier / invalidated caches).
- **a11y**: 4 섹션 모두 `<section>` + `aria-labelledby`. 차트는 `role="img"`.

#### settings 화면

- **IA**: `<topbar: 빌드 버전 + 마지막 업데이트 체크>`, 좌측 카테고리 nav (sm: 일반 / 워크스페이스 / 카탈로그 / 고급) + 우측 form 패널.
- **일반**: 언어 (한국어 / English), 테마 (dark-only 자물쇠 표시 + "곧 light 추가" hint), 한국어 음성 입력/출력 (disabled + Phase 6' coming).
- **워크스페이스**: 경로 표시 + "다른 폴더로 옮기기" (Tauri shell open polder), 워크스페이스 fingerprint repair 강제 트리거 (red tier 가이드 진입).
- **카탈로그**: registry URL (read-only — 외부 통신 0 정책상 사용자 변경 불가, 표시만), 마지막 갱신 시각, "지금 갱신" 버튼 (registry-fetcher IPC 트리거).
- **고급**: Gemini opt-in 토글 (현재 disabled + ko: "Phase 6'에서 만나요"), STT/TTS 토글, 자가스캔 주기(15분 / 1시간 / 끔), `LMMASTER_ENCRYPT_DB` 표시, 진단 로그 export (v1.1 이월 — disabled).
- **거부**: 키 발급은 keys 화면으로 link만, settings에 다중 입력 거부 (single source of truth = ApiKeysPanel).
- **a11y**: form은 `<fieldset>` + `<legend>`. 토글은 `role="switch"` + `aria-checked`.

### 1.2 Korean preset 100+ 매니페스트 스키마 + 7 카테고리 + 5 sample

#### 카테고리 정의 (7개, 각 14~16 preset = 100+)

| ID | display_name_ko | scope | 추천 모델 (Phase 2' 카탈로그 id) | preset 갯수 (목표) |
|---|---|---|---|---|
| coding | 코딩 어시스턴트 | 코드 작성 / 리팩토링 / 리뷰 / 테스트 / 문서 | qwen2.5-coder:7b, deepseek-coder-v2:lite, codellama:13b | 16 |
| translation | 번역 / 다국어 | ko↔en, ko↔ja, 기술 문서, 자막, 외래어 | exaone:1.2b, hyperclova-x-seed:8b, qwen2.5:7b | 14 |
| legal | 법률 보조 | 계약서 검토, 약관 요약, 한국 판례 검색 (citation 강제) | exaone:32b, hyperclova-x-seed:8b | 14 |
| marketing | 마케팅 / 카피라이팅 | 광고 카피, SEO, 인스타 글, B2B 메일, 슬로건 | exaone:1.2b, qwen2.5:7b | 16 |
| medical | 의료 보조 | 환자 설명, 의학 용어 풀이, 영문 의학 논문 요약 (disclaimer 강제) | exaone:32b, hyperclova-x-seed:8b | 14 |
| education | 교육 / 학습 보조 | 초중고 / 어휘 / 수학 풀이 / 영어 작문 첨삭 / 학습 계획 | exaone:1.2b, exaone:7.8b | 16 |
| research | 리서치 / 요약 | 논문 요약, 회의록 정리, 한국어 검색 결과 합성, 인용 | exaone:32b, qwen2.5:14b | 14 |

총 합산 104개. 카테고리당 14~16 preset이 목표 (페이즈 4.h에서 모자라면 추가 작성). 의료 / 법률은 disclaimer 시스템 프롬프트 의무.

#### preset 데이터 모델

```jsonc
{
  "id": "coding/refactor-extract-method",
  "version": "2026-04-27.1",
  "category": "coding",
  "display_name_ko": "메서드 추출 리팩토링",
  "subtitle_ko": "긴 함수를 의미 단위로 잘라드릴게요",
  "system_prompt_ko": "...",
  "user_template_ko": "다음 코드에서 추출 가능한 메서드를 찾아서 리팩토링해 주세요:\n\n{{code}}",
  "example_user_message_ko": "...",
  "example_assistant_message_ko": "...",
  "recommended_models": ["qwen2.5-coder:7b", "deepseek-coder-v2:lite"],
  "fallback_models": ["qwen2.5:3b"],
  "min_context_tokens": 4096,
  "tags": ["코딩", "리팩토링", "한국어"],
  "verification": "verified",         // verified | community
  "license": "CC0-1.0"
}
```

위 스키마를 Rust serde-renamed kebab-case enum + `tag = "kind"`로 deserialize. Verification은 Phase 2'/G5와 동일 2-tier 거버넌스.

#### 5 sample 시스템 프롬프트 (한국어)

**coding/refactor-extract-method (메서드 추출)**
```
당신은 한국어로 응답하는 시니어 소프트웨어 엔지니어예요. 사용자가 코드를 보여주면, 다음 절차를 따라주세요.

1) 함수의 책임을 한 문장으로 요약해 주세요.
2) 책임이 둘 이상이면, 추출 가능한 의미 단위를 표시해 주세요.
3) 각 단위에 대해 추출된 메서드 시그니처와 호출부 변경을 보여주세요.
4) 단위 테스트가 깨질 가능성이 있으면 명시적으로 경고해 주세요.

응답은 항상 한국어 해요체로 작성하세요. 코드 블록 안의 식별자는 영어 그대로 두세요. 외부 라이브러리 가정은 사용자가 명시하지 않은 한 새로 추가하지 말아주세요.
```

**translation/ko-en-tech (한↔영 기술 번역)**
```
당신은 한국어와 영어를 동시에 다루는 기술 번역가예요. 사용자가 한국어 텍스트를 주면 영어로, 영어 텍스트를 주면 한국어로 번역해 주세요.

원칙:
- API / SDK / 토큰처럼 자리잡은 외래어는 그대로 두세요. 의미가 흐려질 위험이 있으면 첫 등장 시 한 번만 한국어 풀어쓰기를 괄호로 덧붙여 주세요.
- 영문이 평서문이면 한국어는 해요체, 영문이 jargon-heavy 기술 문서면 한국어는 격식체로 맞춰 주세요.
- 번역 외 다른 코멘트(예: "이 부분은 ...")는 사용자가 명시 요청하지 않은 한 추가하지 말아주세요.
- 의역이 나은 부분은 별도 표시 없이 자연스럽게 번역하세요. 사용자가 직역을 명시 요청하면 직역으로 전환하세요.
```

**legal/contract-clause-review (계약 조항 리뷰 — disclaimer 의무)**
```
당신은 한국 민법 / 상법 / 약관규제법을 학습한 법률 보조 도우미예요. 사용자의 계약서 조항을 검토할 때 다음 원칙을 따라주세요.

1) 시작 시 항상 다음 disclaimer를 한 줄 출력하세요: "이 답변은 일반 정보예요. 실제 계약은 변호사 상담을 권해드려요."
2) 조항을 다음 4가지 관점에서 분석해 주세요: ① 법적 유효성 ② 사용자에게 불리한 점 ③ 모호한 표현 ④ 표준 약관과의 차이.
3) 한국 판례를 인용할 때는 사건번호와 선고일을 명시하세요. 확실하지 않으면 "정확한 판례 확인이 필요해요"라고 명시하세요.
4) 외국 법체계(미국 / 영국)는 사용자가 명시 요청한 경우에만 비교 분석하세요.

응답은 한국어 해요체로 작성하세요. 조항별로 헤더(##)로 구분해 주세요.
```

**marketing/instagram-copy (인스타그램 카피)**
```
당신은 한국 SNS 마케팅 카피라이터예요. 사용자가 제품/서비스 정보를 주면 인스타그램용 한국어 카피를 작성해 주세요.

규칙:
- 첫 줄은 hook (8자 이내, 호기심 유발). 줄바꿈 후 본문.
- 본문은 60~120자, 해요체. 이모지는 1~2개만 사용하세요.
- 마지막에 해시태그 5~7개를 한 줄로 모아 주세요. 한국어 해시태그를 우선하고, 영어 해시태그는 글로벌 노출이 필요할 때만 추가하세요.
- 광고법(공정거래위원회 표시광고심사지침)을 위반할 우려가 있는 표현(예: "최고", "유일")은 피하거나 근거를 함께 제시하세요.
- 사용자가 톤(격식 / 친근 / 유머)을 명시하면 그에 맞춰 주세요. 명시하지 않으면 친근한 해요체 default.
```

**education/middleschool-math-tutor (중학교 수학 튜터)**
```
당신은 한국 중학교 수학 과목을 가르치는 친절한 튜터예요. 사용자가 문제를 보여주면 다음 절차로 답해 주세요.

1) 문제를 한국어로 다시 풀어 설명해 주세요 (학생이 무엇을 묻는지 확실히 하기 위해).
2) 풀이 과정을 단계별로 보여주세요. 각 단계마다 어떤 개념을 쓰는지 짧게 설명해 주세요 (예: "이건 분배법칙이에요").
3) 답을 명확히 표시해 주세요 (예: "정답: x = 3").
4) 학생이 자주 실수하는 부분이 있으면 한 줄 경고를 덧붙여 주세요.

원칙:
- 답만 알려주지 말고 풀이를 보여주세요. 학생이 따라 할 수 있게요.
- 어려운 용어는 풀어 써주세요. 한자어는 한 번 풀어 쓴 뒤 사용하세요 (예: "이항(移項) — 양변에서 옮기는 것").
- 한국 중등 교과 과정 범위 안에서 답해 주세요. 고등 / 미적분이 필요한 풀이는 "이건 고등학교에서 배우는 방법이에요"라고 안내하세요.
- 응답은 한국어 해요체로 작성하세요.
```

위 5개는 결정 노트에 직접 포함. 나머지 99+는 Phase 4.h sub-phase에서 7 카테고리당 14~16개씩 작성. 작성 검토 체크리스트(§4 invariant 항목)로 일관성 보장.

#### 매니페스트 적재 / IPC

- 위치: `manifests/presets/{category}/{slug}.json` (104개 파일).
- Rust crate: `crates/preset-registry` (신규) — `manifests/presets/`을 read-only로 deserialize → `PresetCatalog` 구조체 → IPC `get_presets(category?)` / `get_preset(id)`.
- 프런트: `apps/desktop/src/ipc/presets.ts` + `apps/desktop/src/pages/Workbench.tsx` 또는 `Catalog.tsx`에 preset chooser drawer 추가 (워크벤치는 placeholder라 v1은 catalog drawer에 link로 노출).
- 카탈로그 entry id 상호 link 검증: build-time validate script — preset의 `recommended_models[]`가 실제 카탈로그에 존재하는지 검사. 없으면 빌드 실패.

### 1.3 status pill / virtual list / command palette 강화

- **StatusPill**: §1.1의 시그니처 그대로. 현재 `App.tsx` `gateway-pill` + `Home.tsx` `home-gateway-pill` + `OnboardingApp` `wizard-gateway-pill`을 단일 컴포넌트로 합치고 inline CSS 제거. 4 상태별 토큰 추가 (CSS variables in `tokens.css`).
- **VirtualList**: §1.1 시그니처. `@tanstack/react-virtual` 의존성 추가. 카탈로그 `Catalog.tsx`의 카드 그리드는 grid layout이라 그대로 두고, runtimes 모델 목록 / preset 목록 / projects 사용 로그에만 적용.
- **Command palette 추가 명령** (Phase 4 종료 시):
    - `nav.install` / `nav.runtimes` / `nav.projects` / `nav.workbench` / `nav.diagnostics` / `nav.settings` (6개 nav 명령)
    - `catalog.search` (카탈로그 진입 + focus search input)
    - `keys.create` (keys 화면 진입 + modal open)
    - `diagnostics.run` (자가 점검 트리거)
    - `model.bench` (선택 모델이 있으면 30초 측정 시작 — 컨텍스트 의존)
    - `workbench.open` (workbench placeholder 진입)
    - `settings.open` (settings 진입)
    - `system.gateway.copyUrl` (기존 유지)
    - `system.wizard.reopen` (기존 유지)
    - `app.changeLanguage.ko` / `app.changeLanguage.en` (i18n 토글)
- 키 = `nav.install` 등은 Korean keyword(`설치`, `ㅅㅊ`)도 등록 (한국어 초성 검색 차용 — Cherry Studio 패턴).
- shortcut display: `cmd+k`로 팔레트 열고, 명령마다 `shortcut?: string[]` (예: `["⌘", "1"]` for `nav.home`). v1은 표시만 (실 hotkey binding은 v1.1).

### 1.4 ko voice & tone audit (Toss 8원칙 차용)

Phase 4 종료 직전 audit script 1회 + manual checklist:

1. **선택권 주기**: `~할게요` / `~해 주세요` / `~할까요?` 비율 점검. 명령조(`~하세요`) 5% 이내.
2. **숫자는 mono num**: 모든 size / port / latency / score는 `class="num"` 토큰 사용. 한국어 텍스트 안에서 숫자 alignment 일관.
3. **외래어 풀어쓰기 첫 등장**: `런타임` / `토큰` / `매니페스트` 등 첫 등장 시 한 번만 괄호 풀어쓰기. 두 번째부터 그대로.
4. **부정어 회피**: `실패했어요` 단독 사용 금지 — 항상 `다시 시도해 볼래요?` 같이 회복 액션 첨부.
5. **숫자 단위**: `8GB` / `5분` / `30초` 단위 띄어쓰기 통일 (한국어 표기법 — 띄어쓰지 않음, 숫자+단위 붙여쓰기 default).
6. **기술 jargon**: `Origin` / `CORS` / `EULA` 같은 영어는 처음 등장 시 한 번만 한국어 설명 추가.
7. **전문가 수준 톤 회피**: 의료 / 법률 카테고리 preset은 disclaimer 의무 (§1.2 sample 참조).
8. **i18n key 1:1 매핑**: ko.json과 en.json 키 set 일치 검증 (test invariant).

audit script는 `scripts/i18n-audit.ts` (Phase 4.h) — ko.json 모든 string에 대해 위 8 원칙 휴리스틱 검사 (정규식 + 빈도 카운트). violation 발견 시 warning + 수정 권고. **실패 시 빌드 차단 안 함** (false positive 가능 — 사람이 최종 결정).

---

## 2. 기각안 + 이유 (negative space — 의무 섹션)

### 2.1 9 화면을 1개 페이지의 tab으로 합치기

- **시도 / 검토 내용**: Linear / Raycast 일부 패턴은 sidebar 없이 tab strip + sub-nav만으로 구성. 데스크톱에서 sidebar 절약 + 화면 면적 확보 효과.
- **거부 이유**: 9 nav 키 중 install / runtimes / projects / diagnostics / settings는 서로 깊이 다른 데이터(설치 진행 / 런타임 상태 / 키 분석 / 진단 / 설정)라 단일 tab strip에 모두 노출하면 화면 전환 cost가 오히려 증가. 또한 Tauri 데스크톱은 기본 1280px+ 가정이라 sidebar 240px 점유가 부담스럽지 않음. 카탈로그 / projects 같은 데이터-heavy 화면은 좌측 컨텍스트 sidebar(필터 / 카테고리)가 필요해 nav sidebar와 별도 레이어가 자연스럽다 — 이 둘이 같은 페이지의 tab이 되면 sidebar 2단으로 nesting 되면서 정보 위계가 깨짐.
- **재검토 트리거**: 사용자 PC 해상도 telemetry로 720p 이하 비중이 30%+ 가 되거나, 사용자 피드백에서 sidebar로 인한 화면 좁음 호소가 반복되면 재검토.

### 2.2 preset 시스템 프롬프트에 영어 fallback 추가

- **시도 / 검토 내용**: Cherry Studio assistant preset처럼 카테고리당 영어 system prompt도 함께 두면 영문 모델(Llama / Mistral)에서 더 자연스러울 수 있음.
- **거부 이유**: Korean-first 정책(CLAUDE.md §4.1, memory `korean_first_principle`) 정면 위반. 영어 fallback이 있으면 모델이 한국어 능력이 약할 때 "쉬운 길"로 가버려 한국어 응답 품질이 하락. 또한 영어 system prompt는 EXAONE / HyperCLOVA-X 같은 한국어 1순위 모델에 오히려 inferior(이들은 한국어로 instruction-tune 되어 있음). v1.1 글로벌 사용자 진입 시 i18n key 분리(`prompt.ko` / `prompt.en`)로 확장하되 v1은 Korean only.
- **재검토 트리거**: v1.1 진입 시점 또는 영문 사용자 피드백 누적 시.

### 2.3 react-window 채택

- **시도 / 검토 내용**: react-window는 small + battle-tested + 0 deps. VariableSizeList / FixedSizeList API가 직관적.
- **거부 이유**: react-window는 ref 기반 imperative API라 React 18 concurrent + Suspense 환경에서 row content가 stale data를 보일 때 강제 reset이 어색함. `@tanstack/react-virtual`은 hook 기반이라 React 18 + concurrent에 자연스럽게 통합되고, sticky group header / horizontal virtualizer / dynamic row size 모두 hook 단일 API로 처리. Virtua도 후보였으나 한국 fonts (변동 폭 글리프)에서 measureElement가 오버헤드를 만들고, react-virtual은 measureElement를 옵션으로 둘 수 있어 우리는 24px 고정 row만 우선.
- **재검토 트리거**: dynamic row size(예: 진단 로그의 멀티라인 stack trace)가 필요해지면 Virtua 또는 react-virtual의 measureElement로 확장 검토.

### 2.4 workbench 화면을 일부 기능(양자화만)이라도 v1에 노출

- **시도 / 검토 내용**: 양자화는 Phase 5'에서 가장 가벼운 단계라 v1에 일부 분리 노출 가능.
- **거부 이유**: 워크벤치의 가치 주장은 "5단계 통합 1-click" — 양자화만 분리하면 사용자가 결과 GGUF로 무엇을 할지 막막해져 (Ollama Modelfile / LM Studio 등록 단계가 빠짐) 오히려 기능 신뢰가 하락. 또한 양자화는 llama-quantize CLI 호출 패턴이 안정적이라 절반 완성으로 노출하면 v1.1 통합 시 마이그레이션 부담. coming soon placeholder가 더 정직한 UX.
- **재검토 트리거**: Phase 5' 일정이 6개월 이상 지연되거나, 사용자 피드백에서 양자화-only 요구가 반복되면 재검토.

### 2.5 키 / projects 합친 단일 페이지

- **시도 / 검토 내용**: keys CRUD와 projects dashboard 데이터 source가 동일하므로 한 페이지에 합쳐서 nav 한 개 줄이기.
- **거부 이유**: keys는 "키 발급·회수" 액션 중심, projects는 "사용량 분석 + 묶어 보기" 분석 중심. 액션과 분석을 같은 페이지에 혼재하면 확률적으로 잘못된 키를 회수할 위험(e.g., 사용량 차트 위에 회수 버튼이 보임). 두 페이지의 navigation 분리 + Cross-link로 충분.
- **재검토 트리거**: 사용자 워크플로 telemetry에서 keys → projects 이동 빈도가 90%+ 면 통합 검토.

### 2.6 settings에 Gemini API 키 입력란 추가

- **시도 / 검토 내용**: Phase 6'에서 Gemini opt-in을 다루지만 settings 화면에 미리 자리만 만들 수 있음.
- **거부 이유**: API 키 입력은 단순 form이 아니라 권한 / 사용 정책 / disclaimer / opt-in 흐름이 필요하다. Phase 6'에서 통합 ADR 후 한 번에 출시하는 것이 안전. v1 settings는 disabled 토글 + "Phase 6'에서 만나요" 문구로 자리만 마련.
- **재검토 트리거**: Phase 6' 진입 시.

### 2.7 한국어 초성 검색을 메인 검색어 처리에 도입 (카탈로그 / preset 검색 box)

- **시도 / 검토 내용**: command palette는 초성(`ㅎ` → "홈") 매칭을 넣을 가치 있음. 같은 패턴을 카탈로그 모델 검색에도 적용 가능.
- **거부 이유**: 카탈로그 모델명은 영문 + 숫자 + ":quant" 조합이라 한글 초성이 거의 발생하지 않음 — 도입 cost 대비 효용 0에 가까움. 한국어 preset 이름 검색(예: "메서드 추출")에는 적용 가치 있으나 v1은 단순 substring match로 충분 (preset 100+ 규모에서). 초성 검색은 v1.1 preset 1000+ 진입 시 검토.
- **재검토 트리거**: preset 카탈로그 1000+ 진입 시.

### 2.8 화면 전환 transition (page slide animation) 도입

- **시도 / 검토 내용**: Linear는 navigation 시 slide 애니메이션, Raycast는 fade. 부드러운 인상.
- **거부 이유**: Tauri WebView는 OS WebView2 기반이라 GPU compositing 비용이 native보다 약간 높다. 9 화면 모두에 transition을 깔면 첫 진입 시 frame drop 위험. 현재 카탈로그 drawer 같은 부분 슬라이드만 framer-motion으로 사용하고 page-level은 instant swap. CSS `prefers-reduced-motion` 사용자도 보호.
- **재검토 트리거**: 사용자 피드백에서 "페이지 전환이 갑작스러워요" 같은 호소가 반복되면 fade(120ms) 정도로 재도입 검토.

---

## 3. 미정 / 후순위 이월

| 항목 | 이월 사유 | 진입 조건 / 페이즈 |
|---|---|---|
| Workbench v1 (양자화 + LoRA + 등록) | Phase 5' 본 페이즈가 별도 page | Phase 5' |
| Knowledge Stack RAG sidebar | Phase 4.5'에서 채팅 화면과 함께 도입 | Phase 4.5' |
| Gemini opt-in 실제 동작 | API 키 / 정책 / 안전장치는 Phase 6' | Phase 6' |
| STT / TTS 한국어 음성 | 모델 선정 + 라이선스 검토는 Phase 6' | Phase 6' |
| 자동 갱신 toast `다음 실행 때` | Phase 6'에서 self-update 채널 + 토스트 통합 | Phase 6' |
| 진단 로그 export (.zip) | bug report 채널이 있어야 의미 있음 | v1.1 |
| 모델 unload (LM Studio) | LM Studio API에 unload 없음 | LM Studio API 변경 시 |
| 7d / 30d projects 사용량 차트 | 24h 표시로 v1 충분 | v1.1 |
| settings 내 다중 워크스페이스 | 워크스페이스 multi-instance는 v2 검토 | v2 |
| 명령 팔레트 hotkey binding (실 키) | UI 표시만 v1, key bind는 conflict 검토 후 | v1.1 |
| preset 사용자 직접 작성 / 수정 | 매니페스트 read-only, 사용자 작성은 별도 file API | v1.1 |
| preset versioning + diff UI | 매니페스트 version 필드만 v1, diff는 v2 | v2 |
| projects 카드 팀원 슬롯 | team mode 전체와 함께 | Phase 6' team mode |

---

## 4. 테스트 invariant

본 sub-phase가 깨면 안 되는 동작 — 다음 세션이 리팩토링하다 우연히 깨면 빨간불이 켜지도록 테스트화.

- **i18n 1:1 매핑**: `ko.json` / `en.json` 키 집합 동일. 1개라도 누락 / 잉여 시 vitest 실패 (`scripts/i18n-key-parity.test.ts`).
- **ko voice 휴리스틱**: ko.json 모든 string에 대해 `~하세요` 비율이 5% 이내, `실패했어요` 단독 출현 0건, `취소` 단독 사용 시 항상 회복 액션 동반 (warning level — fail 아님).
- **preset 카탈로그 cross-link**: 각 preset의 `recommended_models[]`이 카탈로그 `models.json`에 실제로 존재. 빌드 시 검증 (`scripts/preset-validate.ts`) — 없으면 build fail.
- **preset 매니페스트 schema**: 모든 preset JSON이 `PresetSchema.zod`로 parse 성공. 누락 필드 0.
- **preset Korean-only**: 모든 preset의 `system_prompt_ko` / `user_template_ko` / `example_user_message_ko` 한자/영어 비율 < 30% (loanword 제외).
- **StatusPill a11y**: vitest-axe `axe(StatusPill renders).violations.toEqual([])` per status × per size matrix.
- **VirtualList 빈 상태**: items=[]일 때 emptyState 렌더, scroll 없음, axe 0 violation.
- **VirtualList 렌더 정확성**: items=200 + rowHeight=24 시 visible row 수 ≈ container height / 24 ± overscan, off-screen row는 DOM에 없음 (queryAllByRole length check).
- **command palette 6 신규 nav 명령 등록**: `useCommandRegistration` 호출 후 `commands.length >= 12` 검증.
- **command palette Korean 초성 매칭**: query="ㅅㅊ" → "설치" 명령 매칭 (`scripts/palette-filter.test.ts`).
- **install 화면 reuse check**: install 화면의 InstallProgress가 마법사 모드와 메인 모드 모두에서 동일 IPC 채널 (`install-events.ts`)을 구독. 두 모드에서 동시에 마운트되어도 이벤트 중복 구독 0건.
- **runtimes stop confirm**: stop 버튼 click 시 modal 표시되는지 vitest 검증. 그 어댑터를 사용 중인 키가 있으면 경고 메시지 포함.
- **projects ↔ keys 데이터 일관성**: 같은 SQLite source에서 두 페이지가 read-only로 가져옴. revoke 후 projects 카운트 1초 안에 갱신 (이벤트 buffer).
- **workbench placeholder**: 기능 버튼 모두 disabled, 알림 토글만 enabled. axe 0 violation.
- **diagnostics 4 섹션 isolation**: 한 섹션의 데이터 fetch 실패가 다른 섹션을 깨지 않음 (Error Boundary per-section).
- **settings dark-only lock**: 테마 라디오 dark만 selectable, light는 disabled + tooltip ko.
- **settings 외부 통신 0**: registry URL은 read-only. 사용자 input 변경 불가능 (input attribute readonly).
- **catalog → preset cross-nav**: 카탈로그 모델 카드 → preset 추천 drawer 진입 시 추천 preset 정렬 deterministic (모델 id별 동일 순서).
- **presets 카탈로그 deterministic**: getPresets(category) 결과 순서가 매번 같다 (id alphabet 정렬).

---

## 5. 다음 페이즈 인계

### 5.1 sub-phase 분할 (4.a~4.h)

| sub-phase | 범위 | 산출물 | 검증 |
|---|---|---|---|
| 4.a | 공통 컴포넌트 — StatusPill / VirtualList | `packages/design-system/src/react/{StatusPill,VirtualList}.tsx` + tokens.css 추가 + 단위 test + a11y test | vitest pass + 기존 화면 (App / Home / OnboardingApp) StatusPill로 마이그레이션 |
| 4.b | install 화면 | `apps/desktop/src/pages/Install.tsx` + 카드 / 진행 panel reuse + IPC 연결 | install 마법사 / 메인 모드 모두 dev 실행 OK |
| 4.c | runtimes 화면 | `apps/desktop/src/pages/Runtimes.tsx` + RuntimeManager IPC binding + 모델 목록 VirtualList | runtimes 페이지에서 ollama / lm-studio 상태 + 모델 목록 표시 |
| 4.d | projects 화면 | `apps/desktop/src/pages/Projects.tsx` + 사용량 IPC (read-only) + 카드 그리드 | 사용 로그 24h sparkline 정상 표시 |
| 4.e | workbench placeholder | `apps/desktop/src/pages/Workbench.tsx` + 알림 settings DB 키 | 알림 토글 저장 / 복원 OK |
| 4.f | diagnostics 화면 | `apps/desktop/src/pages/Diagnostics.tsx` + 4 섹션 + Error Boundary | 4 섹션 모두 데이터 표시 + 1 섹션 fail 시 다른 3 OK |
| 4.g | settings 화면 + command palette 강화 | `apps/desktop/src/pages/Settings.tsx` + nav.* 6 신규 명령 + 한국어 초성 매칭 | settings form 저장 / 복원 OK + cmd+K로 모든 화면 nav 가능 |
| 4.h | Korean preset 100+ 매니페스트 + preset-registry crate + preset chooser drawer + ko voice audit | `manifests/presets/{category}/*.json` × 104개 + `crates/preset-registry/` + `apps/desktop/src/components/presets/PresetChooserDrawer.tsx` + `scripts/i18n-audit.ts` | 빌드 시 cross-link 검증 통과 + audit script run OK |

각 sub-phase 종료 시 RESUME.md 갱신 + 검증 명령 run + 다음 sub-phase 진입 조건 명시.

### 5.2 선행 의존성

- **Phase 3' 게이트웨이 access log SQLite 테이블** — projects 화면 사용량 표시에 필요. Phase 3'에서 schema (`access_logs(timestamp, key_id, model, origin, request_id)`) 추가 + read-only IPC `get_access_log_24h(key_id?)` 노출. Phase 3' 완료 전까지 projects 화면(4.d)은 mock data로 진행 후 wire-up.
- **Phase 2'.c 벤치 보고서 SQLite 테이블** — diagnostics 좌하 차트에 필요. 이미 Phase 2'.c에서 schema 정의됨 (`bench_reports(model_id, timestamp, tps, ttft, ...)`) → IPC `get_recent_bench_reports(limit=5)` 신규.
- **Phase 1'(scanner crate) 잔여** — diagnostics 좌상 자가스캔 카드는 이미 `onScanSummary` IPC가 있음. 그대로 reuse.

### 5.3 다음 sub-phase로 가는 진입 조건

- 4.a 완료 → StatusPill / VirtualList가 `packages/design-system/src/react/`에 export, App / Home에 마이그레이션 완료, 기존 동작 regression 0.
- 4.b 완료 → install 화면이 마법사 + 메인 모드 모두 정상, IPC 채널 충돌 0.
- 4.c 완료 → runtimes 화면 데이터 binding + stop confirm + virtual list 동작.
- 4.d 완료 → projects 화면 mock 또는 실 데이터로 사용량 표시.
- 4.e 완료 → workbench placeholder + 알림 settings 저장.
- 4.f 완료 → diagnostics 4 섹션 모두 표시 + Error Boundary.
- 4.g 완료 → settings + 12+ command palette nav 동작.
- 4.h 완료 → preset 104개 + drawer + audit script.

### 5.4 위험 노트

- **InstallProgress 재사용 충돌**: Phase 1A에서 InstallProgress가 마법사 step3에 깊이 결합되어 있다. 메인 화면 진입 시 IPC subscribe가 두 군데서 일어나면 progress 이벤트가 듀얼 fire. 채택안: install-bridge.ts에 subscriber count + 이벤트 fan-out 단일화. 4.b 진입 시 이 패턴 먼저 검증.
- **gateway access log volume**: 사용자가 24h 내 1만 request 이상 호출하면 sparkline 계산 무거워짐. 채택안: 시간 단위 1시간 bucket 집계 후 24개 점만 가져오기 (raw row 가져오지 않음). 4.d sub-phase 진입 시 IPC 시그니처 그대로 적용.
- **preset cross-link 빌드 차단**: Phase 2'.a에서 카탈로그 모델 id가 변경(yanked / renamed)되면 preset cross-link이 깨져 빌드 fail. 채택안: 카탈로그 yanked 모델 id에 `aliases[]` 필드를 두고 alias도 cross-link valid로 인정. Phase 2'.a 모델 id 정책 ADR-0014에 alias 항목 추가 검토.
- **VirtualList SSR safety**: Tauri는 SSR이 아니라 client-only지만 vitest 환경에서 jsdom + react-virtual의 ResizeObserver mock이 필요. 채택안: vitest setup.ts에 ResizeObserver polyfill 추가.
- **i18n key parity drift**: 새 화면 6개가 한 sub-phase로 나오면서 ko.json 키가 빠르게 늘어난다. ko 추가 후 en 추가 누락이 잦을 위험 — 4.a sub-phase에서 i18n parity test를 먼저 추가해 이후 sub-phase가 자연스럽게 통과.
- **Korean preset의 의료/법률 disclaimer 일관성**: sample 5개는 정직하게 disclaimer 포함했지만 100+ 작성 시 사람 작성자가 빠뜨릴 위험. 4.h sub-phase에 schema 검증으로 의료/법률 카테고리는 system_prompt에 `disclaimer:` 또는 `면책` 키워드 포함 의무화 (zod refinement).
- **command palette Korean 초성 매칭의 잘못된 false positive**: `ㅎ` → "홈" 매칭이 `ㅎ` → "한국어" 같은 더 일반적 단어를 함께 매칭해 너무 많은 결과. 채택안: 초성 정확도 ≥ 60% 필터 + label 시작 음절 우선.

---

## 6. 참고

### 글로벌 사례 / 패턴 출처

- **Linear** — sidebar + 우측 main 2-pane IA + topbar 상태 표시. https://linear.app
- **Tailscale** — windowed UI status pill 4 상태 + dot 색 + port 표시. https://tailscale.com
- **Raycast** — command palette 그룹 + shortcut + 한국어 초성 검색 가치. https://raycast.com
- **Cherry Studio** — assistant preset 300+ 카테고리 분류 패턴, JSON 매니페스트 차용. https://cherry-ai.com (영어 위주, 우리는 Korean-first 변형).
- **Vercel dashboard** — projects 카드 그리드 + alias + origin chip. https://vercel.com
- **JetBrains Toolbox 3.3** — settings 화면의 "다음 실행 때 적용" 토스트 패턴 (Phase 6' 본격 도입, Phase 4 settings에 자리만).
- **Toss UX writing 8원칙** — 한국어 voice & tone audit 기준. https://toss.tech (공개 가이드)
- **shadcn/ui + Radix** — dense info 패턴 + a11y. https://ui.shadcn.com
- **AnythingLLM** — workspace 카드 패턴 (projects 카드 차용 일부).
- **`@tanstack/react-virtual`** — virtual list 후크 기반 API. https://tanstack.com/virtual

### 관련 ADR

- ADR-0006 (디자인 시스템 토큰) — StatusPill / VirtualList 토큰 추가, 본 페이즈에서 ADR 수정 없이 토큰만 신설.
- ADR-0014 (카탈로그 거버넌스) — preset cross-link 시 alias 정책 보강 검토.
- ADR-0017 (manifest) — preset 매니페스트도 같은 read-only 정책 + 2-tier verification 차용.
- ADR-0022 (게이트웨이 라우팅 + scoped key) — projects 화면이 access log를 소비. read-only.

### 메모리 항목 추가/갱신 (Phase 4 종료 후 메모리에 반영)

- 신규 메모리 후보: `phase4_screen_pattern` — "9 화면은 sidebar + 좌측 컨텍스트 + 우측 main 2-pane 패턴, StatusPill + VirtualList 공통화" 1줄.
- 신규 메모리 후보: `korean_preset_governance` — "preset은 read-only 매니페스트, 의료/법률 disclaimer 의무, Korean only" 1줄.
- 갱신: `design_system_contract` — StatusPill / VirtualList 추가 사실 반영.

---

**문서 버전**: v1.0 (2026-04-27 초안). Phase 4 sub-phase 진입 시 §5.1 표를 기준으로 진행, 각 sub-phase 종료 시 RESUME.md에 산출물 + 검증 결과 기록 후 다음 sub-phase 진입.

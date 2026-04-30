# Phase 11'~12' 결정 노트 — v1.x "도메인 축" 종합 설계

**작성**: 2026-04-30 · **대상 페이즈**: 11'.a/b/c + 12'.a/b + 13'.g.2 wiring 종합
**범위**: 사용자 비전 = "의도(intent) → 하드웨어 적합 후보 → 도메인 벤치마크 → 모델 셀렉트 → 도메인 특화 물꼬"를 v1.x에 안전하게 합류시키는 thesis 확장.
**원칙**: **이슈 최소화** — 신규 페이지/스키마 신설을 최소화하고, 기존 코드(Catalog.tsx, Workbench.tsx, ModelEntry, recommender) **확장 + 호환성 유지**가 1순위.

---

## §1. 결정 요약

1. **카테고리 enum은 손대지 않아요** — `coding/roleplay/agent-general/slm/sound-stt/sound-tts`는 큐레이션 색인으로 보존. UI 표면에서는 *의도(intent)*가 1차 축이 되고, 카테고리는 사이드바 보조 필터로 강등.
2. **두 신규 필드만 ModelEntry에 추가** — `domain_scores: Map<IntentId, f32>` + `intents: Vec<IntentId>`. legacy 호환은 `#[serde(default)]`로 폴백, schema_version bump 없음.
3. **Recommender는 시그니처 확장 + 기존 경로 보존** — `compute(host, target, entries, intent: Option<IntentId>)` 형태. `intent=None`이면 현재 로직 그대로(backward compat). `Some`이면 `domain_scores[intent]`를 weighted-sum에 가산.
4. **HF 직접 검색·바인딩은 하이브리드(C안)** — 큐레이션이 1급, "지원 외" 라벨로 명확히 구분된 검색 결과 패널. "큐레이션 추가 요청" 흐름이 큐레이션 신호 피드백 루프.
5. **Workbench는 3단 사다리로 재배치** — 입구 = 프롬프트 템플릿(즉시), 중간 = RAG 시드(중기), 깊은 길 = LoRA 파인튜닝(현재 5단계 보존, 깊은 진입). 의도 컨텍스트 바를 새로 추가.
6. **비전 모달리티(이미지 입력)는 별도 sub-phase 13'.h**로 분리. 영상 분석은 v2+ deferred.
7. **카탈로그 라이브 갱신 + minisign**은 **Phase 13'.g.2 (이미 DEFERRED 큐)** 그대로 진행 — a/b/c/d 4단계 분할이 적정.

---

## §2. 채택안 (각 결정의 구체)

### 2.1 ModelEntry 스키마 확장 (Phase 11'.a)

```rust
// crates/model-registry/src/manifest.rs
pub struct ModelEntry {
    // ... (기존 필드 그대로) ...

    /// 의도 태그 — 사용자 입력(intent picker)와 매칭되는 자유 태그.
    /// 카테고리(`category`)와 별개 축. 한 모델이 여러 intent에 속할 수 있음.
    /// 예: ["vision-image", "translation-ko-en"], ["coding-python", "agent-tool-use"].
    /// 누락 시 빈 vec — 기존 entries는 schema bump 없이 호환.
    #[serde(default)]
    pub intents: Vec<IntentId>,

    /// 도메인 벤치마크 점수 — `IntentId → 0.0..100.0`.
    /// 누락된 intent는 점수 미보유로 처리(추천에서 가중 0).
    /// 큐레이터가 공식 leaderboard 또는 benchmarks paper에서 인용 + 출처는
    /// `community_insights.sources`에 누적.
    #[serde(default)]
    pub domain_scores: std::collections::BTreeMap<IntentId, f32>,
}

/// 의도 ID — kebab-case. Intent 사전(`shared_types/intents.rs`)에 등록된 것만 허용.
pub type IntentId = String;
```

**Intent 사전 (v1.x 시드)** — `shared_types/intents.rs`:
```rust
pub const INTENT_VOCABULARY: &[(&str, &str)] = &[
    ("vision-image",        "이미지 분석"),
    ("vision-multimodal",   "이미지+텍스트 멀티모달"),
    ("translation-ko-en",   "한↔영 번역"),
    ("translation-multi",   "다국어 번역"),
    ("coding-general",      "코딩"),
    ("coding-fim",          "코드 자동완성 (FIM)"),
    ("agent-tool-use",      "에이전트 / 도구 사용"),
    ("roleplay-narrative",  "롤플레이 / 서사"),
    ("ko-conversation",     "한국어 대화"),
    ("ko-rag",              "한국어 RAG"),
    ("voice-stt",           "음성 인식"),
    // v1.x 시드 11종. v2 확장.
];
```

### 2.2 Recommender 확장 (Phase 11'.b)

```rust
// crates/model-registry/src/recommender.rs
pub fn compute(
    host: &HostFingerprint,
    target: ModelCategory,
    entries: &[ModelEntry],
    intent: Option<&IntentId>,  // ← 신규 (Option 유지로 backward compat)
) -> Recommendation { /* ... */ }

fn evaluate(entry: &ModelEntry, host: &HostFingerprint, target: ModelCategory, intent: Option<&IntentId>) -> Result<Scored, ExclusionReason> {
    // ... (기존 score 계산) ...

    // [신규] Intent score weighted-sum
    if let Some(iid) = intent {
        if let Some(score) = entry.domain_scores.get(iid) {
            // 0..100 → 0..40 가중. 카테고리 Same(+20) 대비 우위 — 의도가 1차 신호라는 것을 명시.
            s += (score * 0.4) as i32;
        }
        // 모델이 intent를 *명시*하면 추가 +5 (큐레이터 의도 표명).
        if entry.intents.iter().any(|t| t == iid) {
            s += 5;
        }
    }
    // ...
}
```

**기존 경로 보존**: 모든 호출자는 `intent=None`으로 호출 가능 → 기존 16 invariant 테스트 0건 깨짐.

### 2.3 Catalog.tsx 의도 보드 (Phase 11'.b)

기존 sidebar/grid 구조는 보존. `RecommendationStrip` 위에 **IntentBoard 컴포넌트**를 신설 + state 한 줄 추가.

```tsx
// apps/desktop/src/pages/Catalog.tsx (변경 부위만)
const [intent, setIntent] = useState<IntentId | null>(null);

// recommendation 재계산 시 intent 전달
useEffect(() => {
  // ...
  getRecommendation(targetCat, intent ?? undefined)  // ← intent 추가
  // ...
}, [category, intent]);

// JSX
<IntentBoard
  selected={intent}
  onSelect={setIntent}
  vocabulary={INTENT_VOCABULARY}
/>
<RecommendationStrip ... />
<ModelGrid ... />  // ModelCard에 intent 전달 → 도메인 점수 바 노출
```

**ModelCard 도메인 점수 바**: `intent`가 선택돼 있고 `domain_scores[intent]`가 존재하면 카드 하단에 mono-numeric 점수 바(`<div class="score-bar num"><div style={width: `${score}%`} /></div>`). 없으면 기존 표시(language_strength 등).

**비교 패널**: 카드 우상단에 "비교에 추가" 체크박스. 3개 도달 시 화면 하단 sticky bar에 ModelCompareDrawer 노출 (레이더 차트 + 표 폴백 — `prefers-reduced-motion` 시 표만).

### 2.4 HF 하이브리드 검색·바인딩 (Phase 11'.c)

**백엔드** — 신규 모듈 `apps/desktop/src-tauri/src/hf_search.rs`:
```rust
// HF Hub Search API: GET https://huggingface.co/api/models?search={q}&limit=20&sort=likes
// 외부 통신 정책 예외 — ADR-0026 §1 (jsDelivr/HF는 화이트리스트).
pub async fn search_hf_models(query: &str, limit: usize) -> Result<Vec<HfSearchHit>, HfSearchError> { /* ... */ }
```

**IPC**:
- `search_hf_models(query: String) -> Vec<HfSearchHit>` — 검색
- `register_hf_model(repo: String, file: Option<String>) -> CustomModel` — 사용자 PC에 "지원 외" 모델로 등록 (기존 CustomModelsSection 패턴 재활용)

**프론트엔드** — Catalog 검색바 옆에 토글 추가:
```
🔍 [검색어]                    [📂 카탈로그 ⚪ HF에서 찾기]
```

HF 모드 진입 시 결과 모달:
```
┌─ HuggingFace에서 찾았어요 (20건) ──────────────────┐
│ ▢ elyza/Llama-3-ELYZA-JP-8B                       │
│   ⬇ 12.3k · ❤ 247 · 갱신 3일 전                   │
│   ⚠ 큐레이션 외 모델                               │
│   [지금 시도해 볼게요 →] [큐레이션 추가 요청]      │
│ ▢ ...                                              │
└────────────────────────────────────────────────────┘
```

"큐레이션 추가 요청" → GitHub Issue URL을 **시스템 브라우저 open** (`tauri::api::shell::open` 또는 사용자 클릭 보호된 외부 링크). 외부 통신 0 정책상 자동 POST는 거부. **이슈 템플릿** repo에 `.github/ISSUE_TEMPLATE/curation-request.yml` 신설 — repo URL/quant 선호/사용자 의도 prefilled.

"지금 시도해 볼게요" → CustomModelsSection의 `register-custom-model` IPC와 동일 흐름. 노란 "지원 외" 배지 + 도메인 점수 비활성 표기.

### 2.5 Workbench 3단 사다리 (Phase 12'.a/b)

기존 5단계(data→quantize→lora→validate→register)는 **"파인튜닝" 깊은 진입**으로 보존. 신규 stage 2개를 *입구*에 추가:

```
[Workbench 컨텍스트 바: 의도 ⟂ 모델 표시]
   ↓
[Stage 0: 프롬프트 템플릿] (신규, 즉시 사용)
   ↓ (선택: 더 깊게)
[Stage 1: RAG 시드] (신규 — knowledge-stack 연결)
   ↓ (선택: 더 깊게)
[Stage 2~6: data/quantize/lora/validate/register] (기존 5단계 보존)
```

**Stage 0 — 프롬프트 템플릿 (Phase 12'.a)**:
- 의도 컨텍스트가 있을 때 자동 매핑 — `vision-image` 의도 → `use_case_examples` 중 비전 케이스 우선 노출
- "내 패턴 저장" → `~/.lmmaster/prompts/<intent>/<name>.json` (로컬, 외부 통신 0 보장)

**Stage 1 — RAG 시드 (Phase 12'.b)**:
- 이미 있는 `knowledge-stack` crate + `EmbeddingModelPanel` 재활용 (Workspace 페이지에 있음)
- "내 자료 추가" → 폴더 picker → embed → 모델에 연결
- `ko-rag` 의도일 때 KURE-v1 자동 권장

**컨텍스트 바**:
```tsx
<WorkbenchContextBar intent={intent} model={selectedModel}>
  🖼 이미지 분석 · Qwen2-VL-7B · llama.cpp · 한국어 ▲▲▲
  [의도 변경] [모델 변경]
</WorkbenchContextBar>
```

URL/state 전달: `Catalog.tsx`의 "이 모델로 시작 →" 버튼이 `window.location.hash = "#/workbench?model=<id>&intent=<iid>"` 형태로 라우팅. Workbench가 해시 파싱.

### 2.6 비전 IPC (Phase 13'.h)

- Tauri IPC `chat_with_image(model_id, prompt, image_base64) -> stream` — 이미지를 base64로 인코딩하여 전달.
- llama.cpp / Ollama vision 어댑터(이미 `runner-llama-cpp`/`adapter-ollama` crate 존재) 호출 — Ollama는 `images: [base64]` 필드, llama.cpp는 `--image` 플래그.
- Chat 페이지에 **이미지 첨부 버튼** (paperclip icon) — `vision_support: true` 모델 + `vision-image` 의도일 때만 활성.
- Workbench Stage 0 프롬프트 템플릿에서도 이미지 업로드 진입.

### 2.7 카탈로그 minisign wiring (Phase 13'.g.2)

이미 ADR-0047 인프라 머지 + DEFERRED 큐에 a/b/c/d 4단계로 분할돼 있어요. **그대로 진행** + v1 ship에 끼워 넣음:

| Sub | 작업 | 검증 신호 |
|---|---|---|
| **g.2.a** | `crates/registry-fetcher/build.rs` — env `LMMASTER_CATALOG_PUBKEY[_SECONDARY]` 빌드 시점 임베드 | `cargo build` 시 pubkey present 여부 단위 테스트 |
| **g.2.b** | `FetcherCore::fetch_one_with_signature` — body fetch 후 `<id>.json.minisig` 추가 fetch + verify | 모킹된 변조 body 거부 invariant |
| **g.2.c** | Diagnostics SignatureSection — verify 실패 시 빨간 카드 + bundled fallback + fresh fetch 차단 | 시뮬레이션 verify 실패 후 UI 빨간 카드 노출 |
| **g.2.d** | `.github/workflows/sign-catalog.yml` — main push 시 `rsign sign` + secret keypair | CI dry-run 통과 + 서명 파일 artifact |

---

## §3. 기각안 + 이유 (negative space 보존)

| # | 기각안 | 거부 이유 |
|---|---|---|
| 1 | **카테고리 enum 확장** (vision/translation/video 카테고리 신설) | 모델 1개가 여러 도메인에 걸침(Qwen2-VL = vision + 한국어 + tool-use). enum 확장은 1:1 분류 강요 → 큐레이션 마찰 + 사용자 멘탈 모델 충돌. *Intent 자유 태그가 N:N 매핑*으로 더 자연스러움. |
| 2 | **트렌딩/뉴 카테고리 추가** (시변 신호를 카테고리로) | deterministic 원칙 위반 — 같은 모델이 카테고리 사이를 왔다갔다 함. 이미 `tier="new"` 메타 축으로 풀려 있음(2026-04-30 Phase 13'.e.1). |
| 3 | **schema_version bump** (1 → 2) | `#[serde(default)]`로 호환 가능 → bump 불필요. legacy 12 entries 백필 부담 0. |
| 4 | **HF 모델 자동 큐레이션 등록** (검색 결과를 큐레이션 카탈로그에 자동 추가) | thesis(`competitive_thesis` 메모리) 핵심 = 큐레이터 검증. 자동 등록은 검증 0이라 thesis 와해 + chat template 깨짐 책임 사용자 부담. **B 안(이슈 템플릿)으로 사용자 신호만 큐레이터에게 전달**. |
| 5 | **HF API 자동 POST(GitHub API로 issue 자동 생성)** | 외부 통신 0 정책 + token 관리 비용. 시스템 브라우저 open으로 사용자 클릭 보호 유지. |
| 6 | **워크벤치 신규 페이지 분리** ("프롬프트 워크벤치" / "RAG 워크벤치" / "파인튜닝 워크벤치" 3개 페이지) | 사용자 멘탈 모델 분산 + 컨텍스트(의도+모델) 전달 비용. 단일 페이지에 사다리 진입이 더 자연스러움. |
| 7 | **영상 분석 v1.x 포함** | 모델 풀(VideoLLaMA, Qwen2-VL video mode) VRAM 16-24GB+ → 일반 PC 사양 초과. 입력 IPC 복잡도(프레임 샘플링 + 시간축) 큼. v2+ 분리. |
| 8 | **자세 분석 (pose estimation) 통합** | LLM 영역이 아니라 ONNX/MediaPipe 영역 — Tauri 런타임 stack 변경 필요. v2 별개 thesis. |
| 9 | **벤치마크 자동 실행 (LMmaster가 사용자 PC에서 MMMU 등 돌림)** | dataset 라이선스 + 시간 비용(수십 분~시간). 큐레이터가 공식 leaderboard 인용 + sources 명시가 사용자 신뢰 + 외부 통신 0과 정합. |
| 10 | **intent 자유 텍스트 → Gemini 매핑** | 의존성 추가 + Gemini 키 강제 + 외부 통신. v1.x는 미리 정의된 11종 칩으로 시작 → 사용자 피드백 누적 후 v2에서 Gemini 매핑 검토. |
| 11 | **현재 Recommender 전면 교체** (sklearn-style ranking model) | deterministic 원칙 위반 + 학습 데이터 부재. weighted-sum 가산 1줄 확장이 정합 + 검증 가능. |
| 12 | **Catalog.tsx 신규 페이지로 분리** ("탐색" / "비교" / "큐레이션 외" 3페이지) | 라우팅 분산 → 사용자가 모델 보러 어디 가야 할지 혼란. 단일 페이지에 모달/sticky bar로 풀이가 더 자연스러움. |

---

## §4. 미정 / 후순위 이월

- **영상 분석 (video LLM 통합)** — v2+. 모델 풀 형성 + VRAM 요구 완화 후 재검토.
- **자세 분석 (pose estimation)** — v2+ 별개 제품 축 또는 플러그인.
- **자유 텍스트 의도 입력 (Gemini 매핑)** — v2+. v1.x는 칩만.
- **Intent 사전 v2 확장** — 사용자 사용량 누적 후 추가 (예: `medical-imaging`, `legal-ko`, `code-review-pr` 등).
- **HF 검색 결과 자동 quant 매칭** — v2+. v1.x는 사용자가 quant 직접 선택 (CustomModel 패턴 재활용).
- **벤치마크 출처 자동 인용 (leaderboard scraper)** — v2+. v1.x는 큐레이터 수동 입력 + `community_insights.sources`.

---

## §5. 테스트 invariant (CLAUDE.md §4.4 + §7 DoD)

### 5.1 Phase 11'.a (스키마)

| invariant | 검증 방식 |
|---|---|
| `intents`/`domain_scores` 누락 시 빈 컬렉션 폴백 | legacy 12+12=24 entries 파싱 테스트 (`manifest_parses_legacy_entry_without_optional_fields` 패턴 재활용) |
| `intents` 중복 entry 없음 | manifest validator 테스트 |
| `domain_scores` 값 범위 0..100 | manifest validator 테스트 |
| `IntentId`가 `INTENT_VOCABULARY`에 등록된 것만 허용 | manifest validator 테스트 (build-catalog-bundle.mjs CI gate) |
| round-trip 직렬화 | 기존 `manifest_round_trip_with_minimal_entry` 패턴 |

### 5.2 Phase 11'.b (Recommender)

| invariant | 검증 방식 |
|---|---|
| `intent=None`일 때 모든 기존 16 테스트 0건 깨짐 | 기존 테스트 그대로 통과 |
| `intent=Some(x)` + entry에 `domain_scores[x]=80` → 동일 카테고리 중 우위 | 신규 invariant 테스트 |
| `intent=Some(unknown)` → 모든 entry score 변동 없음 (graceful no-op) | 신규 |
| determinism: 동일 입력 100회 → 동일 결과 | 기존 패턴 |

### 5.3 Phase 11'.c (HF 검색·바인딩)

| invariant | 검증 방식 |
|---|---|
| HF API 5xx → graceful 한국어 에러 (스택 트레이스 X) | `HfSearchError::Upstream` variant + 한국어 메시지 단언 |
| 빈 query → empty result (네트워크 호출 X) | 단위 테스트 |
| 검색 결과는 모두 "지원 외" 라벨로 처리 | UI 컴포넌트 a11y 테스트 |
| "큐레이션 추가 요청" 클릭 → 외부 브라우저 open (window.open 또는 tauri::api::shell::open mock) | E2E mock 테스트 |
| HF에서 등록한 CustomModel은 도메인 점수 비활성 (도메인 점수 바 미렌더) | UI 단위 테스트 |

### 5.4 Phase 12'.a/b (Workbench 사다리)

| invariant | 검증 방식 |
|---|---|
| URL hash `#/workbench?model=X&intent=Y` 파싱 → 컨텍스트 바 노출 | 단위 테스트 (vitest) |
| Stage 0 프롬프트 템플릿 — `intent`에 매칭되는 `use_case_examples`만 노출 | UI 단위 테스트 |
| Stage 1 RAG — `vision-*` intent일 때 EmbeddingModelPanel은 graceful "이미지 RAG는 v2에서 지원" 안내 | UI 단위 테스트 |
| Stage 0/1 → Stage 2(LoRA) 진입 시 기존 5단계 invariant 0건 깨짐 | 기존 vitest 그대로 통과 |
| 의도/모델 변경 시 Workbench state 리셋 (xstate idle 복귀) | 기존 reducer 테스트 |

### 5.5 Phase 13'.g.2 (minisign wiring)

기존 ADR-0047 결정 노트 §5에 정의된 invariant 그대로 + 신규 4건:

- `LMMASTER_CATALOG_PUBKEY` 미설정 시 빌드 fail (또는 runtime 경고)
- 변조 body → `SignatureVerifyFailed` + bundled fallback 자동 강등
- secondary key 90일 overlap 기간 verify 통과
- Diagnostics SignatureSection 빨간 카드 노출 + 한국어 메시지 ("카탈로그 서명을 확인하지 못했어요.")

---

## §6. 다음 페이즈 인계 (의존성 그래프 + 진입 조건)

```
[Phase 13'.g.2] minisign wiring (a/b/c/d) ──┐
   진입 조건: 0 (인프라 머지 완료, DEFERRED 큐에 분할됨)  │
                                             ├── 독립적, 병렬 가능
[Phase 13'.f.2] 큐레이션 잔여 18 모델 ────────┘
   진입 조건: 0 (스키마 영향 X — domain_scores는 기존 모델 백필과 분리)

[Phase 11'.a] ModelEntry 스키마 + Intent 사전
   진입 조건: 0 (즉시)
   산출물: domain_scores + intents 필드, IntentId enum, INTENT_VOCABULARY, 24 entries 백필
        ↓
[Phase 11'.b] Catalog 의도 보드 + Recommender 가중
   진입 조건: 11'.a 머지
   산출물: IntentBoard 컴포넌트, recommender intent 파라미터, ModelCard 도메인 점수 바, 비교 Drawer
        ↓ (선택적 의존)
[Phase 11'.c] HF 하이브리드 검색·바인딩
   진입 조건: 11'.a 머지 (intents 필드는 검색 hit에 빈 배열로 채움)
   산출물: hf_search.rs IPC, Catalog HF 토글, 검색 모달, "지원 외" 등록 흐름, GitHub Issue 템플릿

[Phase 12'.a] Workbench 컨텍스트 바 + 프롬프트 템플릿
   진입 조건: 11'.b 머지
   산출물: WorkbenchContextBar, Stage 0 (PromptTemplateStep), URL hash 라우팅
        ↓
[Phase 12'.b] RAG 시드 진입점
   진입 조건: 12'.a 머지
   산출물: Stage 1 (RagSeedStep), knowledge-stack 연결, ko-rag intent → KURE-v1 권장

[Phase 13'.h] 비전 IPC + 이미지 입력
   진입 조건: 11'.b + 12'.a 머지
   산출물: chat_with_image IPC, Chat 페이지 paperclip 버튼, Stage 0 이미지 업로드 통합
```

### 위험 매트릭스

| 위험 | 영향 | 완화 |
|---|---|---|
| Intent 사전 v1.x 11종이 사용자 needs를 못 덮음 | 의도 칩에 "맞는 게 없어요" → 카탈로그 1차 사용성 저하 | "기타 / 자유 입력은 v2에서 지원" 한국어 빈 상태 카피 + 사용자 신호 텔레메트리(어떤 의도가 누락됐는지) → v1.x 시드 확장 |
| 큐레이터 백필 부담 (24 entries × 11 intents × benchmark 인용) | 시간 소요 큰 큐레이션 작업 → Phase 11'.a 진입 지연 | **점진 백필** — 모델당 *해당하는 intent만* domain_score 채움. 누락 intent는 0이 아니라 *미보유*(Map에 키 없음). 큐레이터가 시간 날 때 추가. |
| HF 검색 결과 chat template 깨짐 → 사용자 신뢰 추락 | "지원 외" 모델 등록 후 출력 깨짐 | 노란 "지원 외" 배지 + 등록 시 `notes: "이 모델은 큐레이션되지 않아 출력이 깨질 수 있어요"` 자동 prepend + chat template 검증 휴리스틱(첫 응답 unicode replacement char 비율) |
| Workbench 진입 흐름이 기존 사용자에 혼란 | 기존 5단계만 알던 사용자가 신규 Stage 0/1로 진입 시 멘탈 모델 충돌 | URL hash가 없으면 *기존 5단계*로 진입(default). 컨텍스트 바는 hash 있을 때만 노출. 가이드 페이지(`Guide.tsx`)에 새 사다리 섹션 추가. |
| 비전 IPC base64 인코딩 메모리 부담 | 큰 이미지(>10MB) → IPC 페이로드 폭증 | 클라이언트에서 max 4096px 리사이즈 + JPEG 90% 압축. 이미지 path 전달 옵션도 검토(파일 access scope 확장 필요). |
| Recommender 가중치 0.4가 너무 강해 카테고리 신호 무시 | 의도 점수가 카테고리 적합성을 누름 | 시뮬레이션 테스트(`recommender_test.rs`에 시나리오 5종 추가) — 비전 의도 + agent-general 카테고리 모델이 vision-image 의도 80점 모델을 넘지 못함. 0.4 → 0.3 조정 가능. |
| minisign keypair 분실 시 모든 사용자 카탈로그 갱신 차단 | DR 사고 → 사용자 PC 갱신 불능 | secondary key 90일 overlap 기간(이미 인프라 있음) + bundled fallback + Diagnostics 빨간 카드 + 키 회전 SOP 문서화 |

---

## §7. UI/UX 와이어 종합 (재현 시 참조)

### 7.1 Catalog 페이지 (Phase 11'.b 후)

```
┌──────────────────────────────────────────────────────────────┐
│ 모델 카탈로그                                                 │
│ 마지막 갱신: 2시간 전              [다시 불러오기]            │
├──────────────────────────────────────────────────────────────┤
│  [🖼 이미지] [🎬 영상] [💬 한국어] [💻 코딩] [🎙 STT] [🌐 번역] │ ← Phase 11'.b NEW
│  [🤖 에이전트] [🎭 롤플레이] [📚 RAG] [✏ 자유...]              │
├──────────────────────────────────────────────────────────────┤
│ 검색 [_______________]  [📂 카탈로그 ⚪ HF에서 찾기]          │ ← Phase 11'.c NEW
├──────────────────────────────────────────────────────────────┤
│  [전체] [🔥 NEW] [agent] [coding] [roleplay] [slm] ...        │ ← 기존 sidebar
│                                                              │
│  ★ 추천: best · balanced · lightweight · fallback (4슬롯)    │
│                                                              │
│  ┌─ Qwen2-VL-7B ──────────── 추천 ⭐ ──┐                    │
│  │ 이미지 분석 ████████░░ 53.7         │ ← Phase 11'.b NEW   │
│  │ VRAM 7.2GB · 한국어 ▲▲▲             │                     │
│  │ [▢ 비교] [상세] [이 모델로 시작 →]  │ ← Phase 12'.a NEW   │
│  └──────────────────────────────────────┘                    │
│  ...                                                          │
└──────────────────────────────────────────────────────────────┘
[비교 sticky bar] 3개 선택 시 하단 sticky 노출 (Phase 11'.b NEW)
```

### 7.2 HF 검색 모달 (Phase 11'.c)

```
┌─ HuggingFace에서 찾았어요 (20건) ─────────────────────────┐
│ ⓘ 큐레이션 외 모델은 호환성·한국어 강도가 검증되지         │
│   않았어요. 도메인 점수도 표시되지 않아요.                 │
├────────────────────────────────────────────────────────────┤
│ ▢ elyza/Llama-3-ELYZA-JP-8B            [모델 카드]         │
│   ⬇ 12.3k · ❤ 247 · 갱신 3일 전                            │
│   ⚠ 큐레이션 외                                            │
│   [지금 시도해 볼게요 →] [큐레이션 추가 요청]              │
│ ▢ ...                                                      │
└────────────────────────────────────────────────────────────┘
```

### 7.3 Workbench 페이지 (Phase 12'.a/b 후)

```
┌──────────────────────────────────────────────────────────────┐
│ 워크벤치                                                      │
│ 🖼 이미지 분석 · Qwen2-VL-7B · llama.cpp · 한국어 ▲▲▲        │ ← Phase 12'.a
│ [의도 변경] [모델 변경]                                       │
├──────────────────────────────────────────────────────────────┤
│  ① 시작 (프롬프트)  ② RAG  ③ 데이터 ④ 양자화 ⑤ LoRA ⑥ 검증 ⑦ 등록  │ ← Phase 12'.a/b
│  ●━━━━━━━━━━━━━━━━━━○━━━━━○━━━━━○━━━━━○━━━━━○━━━━━○        │
├──────────────────────────────────────────────────────────────┤
│ ┌─ ① 프롬프트 템플릿 ───────────────────────┐                │ ← Phase 12'.a NEW
│ │ • 음식 사진 → 칼로리·영양 추정             │                │
│ │ • 운동 자세 1프레임 → 자세 코칭            │                │
│ │ • 표정 → 감정 라벨링                       │                │
│ │ • 영수증 → 항목 OCR + 분류                 │                │
│ │ [선택] [내 패턴 저장]                       │                │
│ └────────────────────────────────────────────┘                │
│                                                               │
│ ┌─ ② RAG 시드 (선택) ──────────────────────┐                 │ ← Phase 12'.b NEW
│ │ "내 도메인 자료를 추가해서 모델이          │                 │
│ │  내 맥락을 이해하게 해 볼래요?"            │                 │
│ │ [폴더 연결] [어떻게 작동하나요?]            │                 │
│ └────────────────────────────────────────────┘                 │
│                                                                │
│ ┌─ 더 깊게 (LoRA 파인튜닝) ─────────────────┐                 │ ← 기존 5단계
│ │ [③ 데이터 → ④ 양자화 → ⑤ LoRA → ⑥ 검증 → ⑦ 등록]            │
│ └────────────────────────────────────────────┘                 │
└──────────────────────────────────────────────────────────────┘
```

### 7.4 디자인 시스템 토큰 준수 (CLAUDE.md §4.2/§4.3)

- 도메인 점수 바: `--neon-green` accent + `--surface-2` 배경 + `font-variant-numeric: tabular-nums` 토큰
- "지원 외" 배지: `--warning-a-3` 토큰(필요 시 신설) — 인라인 색상 금지
- 의도 칩 selected state: `--primary-a-3` focus ring + `--neon-green` 글자색
- Esc 닫기: HF 검색 모달, 비교 Drawer, 컨텍스트 변경 메뉴 모두 통일
- `prefers-reduced-motion`: 비교 Drawer 레이더 차트 → 표 폴백, sticky bar slide → 즉시 노출

---

## §8. References

- ADR-0044 — Live catalog refresh (jsDelivr 4-tier)
- ADR-0045 — Model tier + community insights schema
- ADR-0026 — 외부 통신 0 정책 + jsDelivr/HF 화이트리스트
- ADR-0047 — minisign Ed25519 서명 검증 (infrastructure)
- ADR-0048 (이번 페이즈 산출 후보) — Intent 축 + domain_scores schema (Phase 11'.a)
- ADR-0049 (이번 페이즈 산출 후보) — HF 하이브리드 검색·바인딩 (Phase 11'.c)
- ADR-0050 (이번 페이즈 산출 후보) — Workbench 3단 사다리 + 비전 IPC (Phase 12' + 13'.h)
- 결정 노트: `phase-13pe1-schema-decision.md`, `phase-13pa-live-catalog-decision.md`, `phase-13pg-catalog-signature-decision.md`
- DEFERRED.md — Phase 13'.f.2, 13'.g.2.{a,b,c,d}
- 기존 코드 인터페이스:
  - `crates/model-registry/src/manifest.rs` — ModelEntry 스키마
  - `crates/model-registry/src/recommender.rs` — compute() 진입점
  - `apps/desktop/src/pages/Catalog.tsx` — IntentBoard 삽입 위치 (RecommendationStrip 위)
  - `apps/desktop/src/pages/Workbench.tsx` — Stage 0/1 삽입 위치 (STEP_KEYS 앞에)
  - `apps/desktop/src/components/catalog/CustomModelsSection.tsx` — HF 등록 패턴 재활용
  - `crates/knowledge-stack/` — RAG 시드 (Stage 1) 연결
  - `apps/desktop/src-tauri/src/hf_meta.rs` — HF Hub API 호출 패턴 재활용

---

**다음 standby**: Phase 11'.a (ModelEntry 스키마 확장 + Intent 사전 v1.x 시드). 결정 노트 머지 후 즉시 진입.

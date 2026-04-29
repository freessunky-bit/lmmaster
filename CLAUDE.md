# Claude Code — LMmaster 프로젝트 워크플로 규칙

> 모든 세션 시작 시 자동 로드. `.claude/settings.local.json`은 *권한*을, 본 문서는 *행동 규칙*을 정의한다.
> 사용자/프로젝트/제품 컨텍스트는 `docs/PRODUCT.md`, `docs/PHASES.md`, `docs/RESUME.md`, `docs/adr/`, 그리고 `~/.claude/projects/.../memory/MEMORY.md`를 우선 참조.

---

## 0. 권한 파일 분담 (auto tracker 정책)

| 파일 | 역할 | 주의 |
|---|---|---|
| `~/.claude/settings.json` | 사용자 전역 — 모델/effort + `skipDangerousModePermissionPrompt: true` | bypass 진입 prompt 자체를 글로벌 스킵 |
| `.claude/settings.json` | 프로젝트 공유 (git 추적) — **bypass 정책 + 광범위 allow + ask/deny** | **이 파일이 source of truth.** IDE auto-permission tracker가 이 파일에는 손대지 않음 |
| `.claude/settings.local.json` | 프로젝트 로컬 (git 무시) — IDE auto-tracker가 자동 갱신 | base는 `defaultMode: bypassPermissions`만 두고 비워둠. 자동 누적은 무해 (allow는 union) |

**적용 시점**: settings 변경은 **다음 Claude Code 세션부터** 완전히 적용됩니다. 현재 세션 끝난 후 새 세션 시작 시 자동 활성.

---

## 1. 자율 실행 정책 (autonomy policy)

사용자는 **대기 상태 없이 다음 페이즈로 자동 진행**을 원한다. 다음 작업은 확인 없이 즉시 수행한다.

- 파일 읽기 / 쓰기 / 편집 (프로젝트 트리 내).
- 새 폴더 / 파일 생성.
- 빌드·린트·테스트·포맷 명령 실행 (`pnpm`, `cargo`, `npm`, `node`, `npx`).
- 의존성 추가 (`pnpm install`, `cargo`의 workspace.dependencies 등록).
- 메모리 저장 / `docs/research/<phase>-decision.md` 작성.
- web search / web fetch 리서치.

**확인이 필요한 액션** (실행 전 사용자 승인):
- `git push`, `git reset --hard`, `git checkout -- <file>`, `git restore`, `git clean -f`, `git branch -D`.
- `rm -rf`, `Remove-Item -Recurse -Force`, `rmdir /s`.
- `.env` 또는 `secrets/**` 읽기·편집·작성.
- 새 ADR 필요한 큰 아키텍처 분기.
- 두 개 이상 동등한 설계안 중 선택.
- 의존성을 크게 갈아끼우는 결정 (Tauri major upgrade, runtime stack 교체 등).

토큰 한계로 완성도가 위협되면 sub-phase로 분할 후 `docs/RESUME.md`에 인계 메모를 남기고 자연스럽게 멈춘다.

---

## 2. 페이즈 운영 (phase strategy)

매 페이즈는 4단계로 진행:

1. **보강 리서치** — 별도 Agent로 글로벌 라이브러리·GitHub·베스트 프랙티스 (엘리트 사례) 조사. 결과는 `docs/research/<phase>-decision.md`로 종합. 메인 컨텍스트 보존 목적.
2. **설계 조정** — 결정 노트 / ADR / repo tree 반영. 큰 변경은 ADR 신설.
3. **프로덕션 구현** — `unimplemented!()` 금지. 로깅 + 에러 처리 + 테스트 포함. 한국어 1차.
4. **검증** — `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace` + `pnpm exec tsc -b` + `pnpm run build`.

각 sub-phase 끝에 `docs/RESUME.md`에 산출물·검증 결과·다음 standby를 기록.

---

## 3. 빌드 / 검증 명령 — 정식 형식만 사용

**❌ 사용 금지**: PowerShell 동적 exe 호출.

```powershell
# 금지 — Claude Code가 이 형태를 만들면 settings.local.json 매번 새 prompt 발생.
& "$env:USERPROFILE\.cargo\bin\cargo.exe" clippy ...
```

**✅ 사용 권장**: 정식 PATH 명령 또는 `.claude/scripts/` 헬퍼 스크립트.

```powershell
cargo clippy --workspace --all-targets -- -D warnings
pnpm exec tsc -b
pnpm run build
.\.claude\scripts\verify.ps1
```

`cargo` / `pnpm` / `node`는 사용자 PATH에 등록돼 있다 (`%USERPROFILE%\.cargo\bin`, pnpm globalbin). PATH 미작동 시 헬퍼 스크립트 사용.

### 헬퍼 스크립트 (`.claude/scripts/`)

| 스크립트 | 동작 |
|---|---|
| `verify.ps1` | 풀 검증: cargo fmt --check + clippy + test + tsc + vite build |
| `cargo-clippy.ps1` | `cargo clippy --workspace --all-targets -- -D warnings` |
| `cargo-test.ps1` | `cargo test --workspace` (테스트 카운트 추출) |
| `cargo-fmt.ps1` | `cargo fmt --all` (적용) + `--check` 검증 |
| `frontend-build.ps1` | `cd apps/desktop && pnpm exec tsc -b && pnpm run build` |
| `frontend-tsc.ps1` | `cd apps/desktop && pnpm exec tsc -b` |

스크립트는 PATH 보강 + 출력 트리밍을 자체 처리한다. Claude Code가 매 페이즈 검증마다 이 6개 중 하나로 호출하면 prompt 0.

---

## 4. 코드 품질 / 스타일

### 4.1 한국어 카피 톤 매뉴얼 (사용자 향 모든 텍스트)

| 영역 | 톤 | 예시 (✅) | 금지 (❌) |
|---|---|---|---|
| 액션 버튼 | 해요체 동사 | "설치할게요", "취소할래요", "다시 시도할게요" | "설치", "Install", "설치하시겠습니까?" |
| 메타 라벨 | 명사구 | "VRAM 권장", "설치 크기", "한국어 강도" | "VRAM이 추천됩니다" |
| Hint chip | 해요체 짧은 문장 | "내 PC와 잘 맞아요", "메모리 빠듯해요" | "이 모델 적합합니다" |
| 에러 / 경고 | 해요체 사실 진술 | "VRAM이 부족해요. 8GB 필요해요." | "Error: insufficient VRAM" |
| 빈 상태 | 해요체 안내 | "조건에 맞는 모델이 없어요. 필터를 조정해 볼래요?" | "No results" |
| 진행 상태 | 진행형 + 해요체 | "받고 있어요", "압축 풀고 있어요" | "Downloading…" |

**금지**:
- 의문문 호명 ("어떤 모델 좋으세요?")
- 공식체 ("설치하시겠습니까?", "확인하십시오")
- 영어 뱅크 문구 그대로 노출 ("Click to install", "Loading…")
- 외래어 남발 — 단, 다음 loanwords는 **유지**: VRAM, RAM, 모델, 토큰, GPU, API, 런타임, 양자화. 첫 등장 시 풀어쓰기 OK ("VRAM(그래픽 메모리)").

i18n 키 추가 시 `ko.json`과 `en.json` **동시 갱신** 필수 — 한쪽만 추가하면 fallback이 깨져 한국어 화면에 영어 키가 노출돼요.

### 4.2 코드 컨벤션

- **디자인 토큰만**: `packages/design-system/src/tokens.css`. 인라인 색·여백·radius 금지. 알파 wash가 필요하면 `--{color}-a-N` 토큰을 추가하고 사용 (직접 `rgba()` 인라인 X).
- **kebab-case serde**: enum variant은 `#[serde(rename_all = "kebab-case")]` + 다중 variant은 `#[serde(tag = "kind")]` tagged enum 일관. `BenchErrorReport` / `ExclusionReason` / `ScanApiError`가 표준 형태.
- **에러 enum**: thiserror 기반 + 한국어 `#[error(...)]` 메시지. `Display`만으로 사용자에게 노출 가능해야 함.
- **외부 통신 0**: localhost-only 바인딩, no-proxy. 자가스캔/벤치/카탈로그 결과는 디바이스 로컬.
- **deterministic 우선**: 판정/추천/스코어링은 deterministic 로직. LLM은 사용자 향 자연어 요약만.
- **EULA 준수**: LM Studio는 `open_url`만 (silent install 금지), Ollama는 silent install OK (MIT).

### 4.3 UI / 컴포넌트 게이트 (a11y + 일관성)

- **Button reset**: 모든 `<button>`은 `appearance/background/border/text-align/font/width` 명시. focus-visible ring 토큰(`--primary-a-3`) 필수.
- **Navigation은 `<button>` 또는 `<a href>`**: 라우팅용 `<a>` (href 없는) 금지 — 키보드/스크린리더 부적합.
- **role + aria 명시**: dialog는 `role="dialog" aria-modal="true" aria-labelledby` 3종 세트. radiogroup은 `role="radiogroup"` + 자식 `role="radio" aria-checked`.
- **Esc / 배경 클릭** 닫기 패턴 통일 — Drawer/Modal/Palette 모두.
- **focus 첫 요소** auto-focus — Drawer 열림 시 close 버튼, 마법사 진입 시 첫 input.
- **`prefers-reduced-motion`**: spotlight effect / slide animation은 토큰 차원에서 자동 비활성. 신규 애니메이션 추가 시 토큰만 사용 (`--dur-*`, `--ease-*`).
- **mono numeric**: 숫자 표기(`.num` 클래스)는 `font-variant-numeric: tabular-nums` 적용된 토큰만 사용.

### 4.4 테스트 invariant (sub-phase DoD)

매 신규 모듈은 다음 invariant 중 **해당하는 것**을 반드시 테스트:

| 영역 | 필수 invariant |
|---|---|
| Recommender / 점수 함수 | determinism (동일 입력 100회 동일 결과), 빈 입력, 충돌 케이스 |
| Manifest 로더 | 빈 디렉터리, 잘못된 schema_version, JSON parse 실패, 중복 ID |
| 캐시 | round-trip, missing file, TTL 만료, fingerprint mismatch, invalidate idempotent |
| Tagged enum (serde) | `kind` 필드 값 검증 + round-trip |
| React 컴포넌트 | a11y (vitest-axe `violations.toEqual([])`), 빈 상태, Esc/키보드, **scoped 쿼리** (`within()` / `data-testid`) — `getByText`로 중복 가능한 텍스트 단언 금지 |
| 에러 메시지 | 한국어 포함 단언 (string 단언 1회) |

**테스트 brittle 방지**:
- 텍스트 단언은 *마커 텍스트*에 한정 (예: i18n 키 또는 unique substring). 화면 전체 prose 단언 금지.
- 동일 텍스트가 여러 곳에 나올 수 있으면 `getAllByText().length` 또는 `within(scope).getByText()`.
- 카운트(test count) 변동 시 RESUME에 차분(+N) 명시.

### 4.5 결정 노트 필수 6-섹션

`docs/research/<phase>-decision.md` 또는 `<phase>-reinforcement.md`는 다음 6 섹션 필수 — `docs/DECISION_NOTE_TEMPLATE.md` 참조:

1. **결정 요약** — N가지 핵심 결정을 1줄 bullet로.
2. **채택안** — 각 결정의 구체적 채택 내용.
3. **기각안 + 이유** — 검토했지만 채택 안 한 옵션과 *왜 거부했는지*. **이게 negative space — 다음 세션이 같은 함정에 다시 빠지지 않게 하는 장치.**
4. **미정 / 후순위 이월** — v1.x로 미루는 항목 + 이유.
5. **테스트 invariant** — 본 sub-phase가 깨면 안 되는 invariant 목록.
6. **다음 페이즈 인계** — 의존성, 진입 조건, 위험 노트.

---

## 5. 메모리 활용

세션마다 `~/.claude/projects/.../memory/MEMORY.md` 자동 로드.

핵심 메모리 (이미 등록):
- `project_identity` / `tech_stack_defaults` / `korean_first_principle` / `design_system_contract` / `gemini_boundary` / `document_first_workflow` / `phase_strategy` / `product_pivot_v1` / `zero_knowledge_and_self_scan` / `autonomous_mode` / `competitive_thesis`.

신규 메모리 추가 기준:
- 사용자 명시 요청.
- 프로젝트 결정 (ADR 수준은 아니지만 다음 세션이 알아야 할 항목).
- 사용자 피드백 (corrections + confirmations 양쪽).

---

## 6. 안전 가드

- `git push` 전 항상 사용자에게 확인.
- `.env` / `secrets/**` 직접 편집 금지 (사용자 명시 요청 시만).
- `node_modules`, `target`, `dist` 직접 편집 금지 — 빌드 산출물.
- 의심스러운 파이프 (`curl ... | sh`, `iex (...)`) 금지.

---

## 7. Sub-phase Definition of Done (DoD)

매 sub-phase 완료 = 다음 모두 충족:

**구현**:
- [ ] 결정 노트 6-섹션 완전 (특히 §4.5의 *기각안 + 이유*).
- [ ] 신규 모듈에 §4.4 테스트 invariant 적용.
- [ ] 한국어 카피 §4.1 톤 + i18n ko/en 동시 갱신.
- [ ] UI 변경 시 §4.3 a11y/포커스/키보드 게이트.

**검증** (`.claude/scripts/verify.ps1` 또는 개별 명령):
- [ ] `cargo fmt --all -- --check` ✅
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` ✅
- [ ] `cargo test --workspace` ✅ (테스트 카운트 RESUME에 차분 명시)
- [ ] `pnpm exec tsc -b` ✅
- [ ] `pnpm exec vitest run` ✅ (UI 변경 시)

**문서**:
- [ ] `docs/RESUME.md`에 산출물·검증값·차분 테스트 카운트·다음 standby 기록.
- [ ] 다음 sub-phase 진입 조건 (선행 의존성, 보강 리서치 영역) 명시.
- [ ] 결정 노트 + (변경 면적이 크면) ADR.

**메모리** (필요 시):
- [ ] 사용자 명시 요청 / 새 결정 / 사용자 피드백(corrections + confirmations).
- [ ] 메모리는 "Claude는 일시적, 문서·테스트·메모리는 영속"이라는 분리 원칙. 결정의 *왜*를 메모리에 남기지 말고 결정 노트에. 메모리는 "다음 세션 시작 시 즉시 알아야 할 메타-맥락"만.

**자율 진입**:
- [ ] 토큰 예산이 다음 sub-phase 완성도를 위협하지 않는지 자가 평가. 위협 시 분할.
- [ ] 사용자 신호 ("진행해", "이어서") 또는 사용자 부재 + 진입 조건 충족 시 자동 진행. 옵션 나열 / 결정 대기 금지.

---

## 8. Negative space 보존 원칙

세션 compaction은 **결론은 살리지만 *왜 다른 선택지를 거부했는지*는 옅어진다**. 이를 막기 위해:

1. **결정 노트 §3 "기각안 + 이유"는 의무**. "X 안을 봤는데 Y 이유로 거부했다"를 한 줄이라도 남길 것.
2. **테스트가 negative space의 ground truth**. invariant 테스트 = "이 동작은 *깨지면 안 된다*"의 기계적 보존.
3. **코드 주석은 WHY만**. WHAT은 코드가 말함. WHY 중에서도 "이렇게 안 한 이유"가 가장 가치 높음 (예: `// wgpu drop — windows-core 0.61 vs Tauri 0.62 trait 충돌`).
4. **메모리 feedback 메모는 corrections + confirmations 양쪽**. 사용자 승인도 negative space (다른 안을 거부한 흔적).

---

**문서 버전**: v1.1 (2026-04-27 — §4 카피/코드/UI/테스트 매트릭스 + §7 DoD + §8 negative space 추가). 사용자 워크플로 / 컨벤션 변화 시 본 문서 갱신 후 메모리 동기화.

# ADR-0056 — i18n 일관성 + thiserror 프런트엔드 노출 정책 (이모지 거부 + kind 기반 에러 매핑)

* **상태**: Accepted (2026-05-03). Phase R-D 머지와 함께 적용.
* **선행**: ADR-0010 (Korean-first principle). ADR-0011 (디자인 시스템 공유). ADR-0052 (R-A path boundary — `PathDenied` variant 신설). ADR-0055 (R-C — `derive_filename` `InvalidSpec` variant 강화). CLAUDE.md §4.1 (한국어 카피 톤) + §4.3 (픽토그램 정책 — 컬러 이모지 직접 사용 금지).
* **컨텍스트**: 2026-05-02 GPT Pro 검수에서 v0.0.1 ship-blocker 4건이 i18n / 에러 노출 카테고리에서 식별됨.
  1. **K1**: `apps/desktop/src/i18n/{ko,en}.json`에 컬러 이모지 인라인 11건(`✅❌⚠ℹ`). CLAUDE.md §4.3 `lucide-react` 픽토그램 정책 위반.
  2. **K2**: R-A의 `PathDenied` 에러 + R-C의 `derive_filename InvalidSpec`을 frontend가 한국어로 노출할 i18n 키 누락. 사용자가 영문 thiserror Display 메시지를 raw로 보거나 kind switch 분기 누락.
  3. **K3**: `Catalog.tsx` 5건 한국어 hardcoded prose (`마지막 갱신`, `갱신하고 있어요…`, `다시 불러오기`, refresh button title, refresh empty state) 미 i18n. 영어 locale에서 한국어 노출 → broken UX. 또한 i18n 호출의 `defaultValue` 일부가 stale `⚠ chip` 참조.
  4. **K4 (thiserror frontend 노출 정책)**: backend `PortableApiError` / `ActionError` 등 thiserror enum이 `Display` 한국어 메시지를 갖지만, frontend는 `kind` 기반 i18n switch 없이 raw Display 노출. 한국어 사용자 OK이나 영어 locale 토글 시 영어 사용자에게 한국어 메시지 노출.
* **결정 노트**: `docs/research/phase-r-d-frontend-polish-decision.md`

## 결정

1. **K1 — i18n 이모지 인라인 0건 강제** — `apps/desktop/src/i18n/{ko,en}.json`의 `✅❌⚠ℹ` 11건 제거. 텍스트는 plain prose로 유지하고, 컴포넌트 측에서 `lucide-react` 아이콘(`CheckCircle2 / XCircle / AlertTriangle / Info`) prepend.
2. **K1 — `Diagnostics.SignatureSection` lucide 아이콘 wiring** — `signatureIcon(tone)` helper가 tone(ok/warn/error/neutral) → lucide 컴포넌트 매핑. message `<p>`에 `<Icon size={14}>` + `<span>{message}</span>` 분리. 기존 인라인 emoji 시각 보강 + 더 깔끔한 a11y(`aria-hidden` icon + 텍스트가 screen reader 경로).
3. **K1 — `catalog.commercial.chip` (`⚠ 비상업` / `⚠ Non-commercial`) 정리** — `ModelCard`는 이미 `<TriangleAlert size={12}>` icon prepend 중. i18n 텍스트의 `⚠` prefix는 *중복* — text만 `비상업` / `Non-commercial`로.
4. **K1 — tooltip 텍스트의 emoji 참조 정리** — `catalog.adultContent.toggleTitle` / `catalog.hfSearch.triggerTitle`의 "⚠ chip" / "⚠ 라벨" 같은 *설명 prose 내부 emoji 참조*는 "경고 아이콘" / "warning chip" 같은 *말로* 표현.
5. **K2 — `errors.path-denied` + `errors.path-traversal` + `errors.generic` 신설** — 최상위 `errors` namespace 추가. ko/en 동시 갱신.
   ```json
   "errors": {
     "path-denied": "workspace 밖 경로에는 가져올 수 없어요. 워크스페이스 안의 폴더를 골라 주세요.",
     "path-traversal": "안전하지 않은 경로가 들어 있어 받지 못했어요. 다른 다운로드 링크를 사용해 주세요.",
     "generic": "예기치 못한 오류가 났어요. 잠시 뒤에 다시 시도해 볼래요?"
   }
   ```
6. **K3 — `Catalog.tsx` hardcoded → i18n key 5건** — `catalog.refresh.lastAt` / `.never` / `.title` / `.busy` / `.action` 신규. ko/en 양쪽 정의. defaultValue도 갱신.
7. **K3 — `PortableApiError` TypeScript 미러에 `path-denied` variant 추가** — `apps/desktop/src/ipc/portable.ts`의 union에 `{ kind: "path-denied"; reason: string }`. tsc-narrow 친화.
8. **K4 — kind 기반 i18n switch 패턴** — `PortableImportPanel.tsx`의 catch 블록에서 `e.kind === "path-denied"` 분기 → `setError("errors.path-denied")` (i18n 키). 기존 fallback path는 `runner::${e.message}` (Korean prose 그대로 표시) 유지. 두 layer 정합:
   - Backend thiserror Display는 *한국어 단독 사용자* fallback path 유지 (이미 한국어).
   - Frontend가 *kind 인식* → 정식 i18n 키로 전환 (locale 토글 친화).
9. **`signatureIcon` helper에 lucide 컴포넌트 타입 매핑** — TypeScript `typeof CheckCircle2` 반환 — 기본 동작 잘 정합 (모든 lucide 아이콘이 `LucideIcon` 동일 인터페이스).

## 근거

- **이모지 거부 = CLAUDE.md §4.3 정책 일관 강제**: Phase 14' v1에서 컴포넌트 측 이모지는 lucide로 교체 완료. i18n JSON 측이 stale 잔재. 본 ADR로 정책 재확인 + 잔재 청소.
- **lucide currentColor + monochrome stroke**: 컬러 이모지(`⚠`)는 OS별 렌더링 폰트에 의존 (Windows Emoji, Apple Color Emoji 다름) → 디자인 시스템 일관성 깨짐. lucide는 currentColor 기반 — 항상 토큰 컬러로 렌더.
- **`errors` 최상위 namespace**: 기존 `screens.*.errors`는 *컨텍스트별*. cross-cutting 시스템 에러(path-denied / path-traversal / generic)는 최상위가 정합.
- **kind 기반 switch over Display string**: locale 토글 시 영어 사용자가 한국어 메시지 보지 않도록. backend Display는 *backend 로그 + 한국어 사용자 fallback* 역할만.
- **Catalog refresh prose i18n**: `Catalog.tsx`는 핵심 사용자 진입점. 영어 locale 토글 시 핵심 화면이 한국어로 깨지면 v0.0.1 ship 블락.

## 거부된 대안

1. **이모지를 그대로 두고 lucide-react로 *추가 prepend*만 (icon + emoji 둘 다 표시)**: 시각적 중복 + 사용자 혼란. 이모지 제거가 정합.
2. **i18n에 Unicode symbol(`✓ ✗ ⚠`) 유지 (컬러 이모지 X)**: Unicode mark도 OS 폰트 의존 — Apple/Windows 다른 렌더링. lucide 일관 정책이 더 깔끔.
3. **errors namespace를 backend kind와 1:1 매핑** (예: `errors.portable.path-denied`): 컨텍스트별 분기 폭증. 최상위 `errors.path-denied`가 cross-cutting OK.
4. **PortableImportPanel catch에서 always raw Display string 사용**: locale 토글 깨짐. kind switch가 정공.
5. **Catalog refresh prose를 prop drill로 (page parent에서 i18n 처리)**: t() in-place가 더 간결.
6. **lucide 아이콘을 SVG inline으로 직접 작성**: tree-shaking 깨짐. `import { CheckCircle2 } from "lucide-react"` named import가 정공 (Phase 14' v1 정책 일관).
7. **path-denied i18n 키를 변수 보간(`{{reason}}`)으로**: thiserror Display의 reason 자체가 한국어 prose → 사용자에게 그대로 노출 시 자연. variable interpolation 대신 *고정 prose*로 단순화 + reason은 Display fallback 경로에서 노출.
8. **K4를 모든 thiserror enum에 적용 (`StoreError` / `KnowledgeError` / `BenchError` 등)**: 영향 범위 폭증. 본 sub-phase는 R-A/R-C에서 만진 `PortableApiError` / `ActionError`만. 다른 enum은 *현재 한국어 Display 사용 중*이고 사용자 영향 작음. v1.x에서 통합 audit.
9. **vitest 테스트 추가 (i18n 키 sync 검사)**: ko/en JSON sync는 *수동 audit* 충당. 자동 테스트는 v1.x에서 typed-i18n-keys crate 도입 시.
10. **CSP `unsafe-inline` 변경**: 본 sub-phase 범위 외 (R-A에서 결정).

## 결과 / 영향

- **`apps/desktop/src/i18n/{ko,en}.json`**:
  - `diagnostics.signature.{verified,failed,missing,bundled,disabled}` 5건 emoji 제거.
  - `catalog.commercial.chip` emoji 제거.
  - `catalog.adultContent.toggleTitle` / `catalog.hfSearch.triggerTitle` prose에서 `⚠` 참조 → "경고 아이콘" / "warning chip"로 변환.
  - 신규 최상위 `errors` namespace + 3 키.
  - 신규 `catalog.refresh.{lastAt, never, title, busy, action}` 5 키.
- **`apps/desktop/src/pages/Diagnostics.tsx`**:
  - `lucide-react` 4 아이콘 import (`CheckCircle2 / XCircle / AlertTriangle / Info`).
  - `SignatureSection`에 `Icon` prepend (`size={14}` + `aria-hidden`).
  - `signatureIcon(tone)` helper (~10 LOC).
  - `signatureMessage` defaultValue에서 `❌` / `⚠` 제거.
- **`apps/desktop/src/pages/Catalog.tsx`**:
  - 5건 hardcoded → `t(key, defaultValue)` 패턴.
  - `catalog.adultContent.toggleTitle` defaultValue prose 갱신 (`⚠ chip` → `경고 아이콘`).
- **`apps/desktop/src/components/portable/PortableImportPanel.tsx`**:
  - catch 블록에 kind 기반 분기 추가 (`path-denied` → `errors.path-denied`).
  - 기존 raw message fallback 유지 (백워드 호환).
- **`apps/desktop/src/ipc/portable.ts`**:
  - `PortableApiError` union에 `{ kind: "path-denied"; reason: string }` variant 추가.
- **백워드 호환**:
  - 기존 i18n 키 변경 *내용*만(시작 emoji 제거) — 키 이름 자체는 그대로. fallback 영향 0.
  - PortableApiError union 확장은 superset → 기존 caller 0건 깨짐.
- **테스트**: vitest sweep clean (i18n 키 변경은 컴포넌트 단언이 i18n 호출만 의존, 텍스트 prose 단언은 *마커 텍스트* 한정 — Phase 14' v1 정책).

## References

- 결정 노트: `docs/research/phase-r-d-frontend-polish-decision.md`
- GPT Pro 검수: 2026-05-02 30-issue static review (K1+K2+K3+K4 4건 본 ADR로 해소)
- 코드:
  - `apps/desktop/src/i18n/{ko,en}.json` (이모지 제거 + 신규 namespace)
  - `apps/desktop/src/pages/Diagnostics.tsx` (lucide wiring)
  - `apps/desktop/src/pages/Catalog.tsx` (i18n hardcoded 정리)
  - `apps/desktop/src/components/portable/PortableImportPanel.tsx` (kind switch)
  - `apps/desktop/src/ipc/portable.ts` (PathDenied variant)
- 관련 ADR: 0010 (Korean-first), 0011 (Design system contract — Phase 14' v1 lucide 정책), 0052 (R-A PathDenied), 0055 (R-C derive_filename)
- 후속: Phase R-E (architecture v1.x — A1 chat protocol decoupling / A2 bench trait / C2 OpenAI compat 공통화 / P1 KnowledgeStorePool / P4 channel cancel / R2 cancellation token / T3 wiremock — POST v0.0.1 release)

# Phase R-D — Frontend Polish 결정 노트

> 2026-05-03. GPT Pro 검수 30건 중 v0.0.1 ship-blocker i18n / 에러 노출(K1+K2+K3+K4) 4건을 본 sub-phase에서 해소.

## 1. 결정 요약

- **D1 (K1)**: i18n ko/en에서 컬러 이모지 11건 (`✅❌⚠ℹ`) 제거. 컴포넌트 측 lucide-react 아이콘 prepend로 시각적 보강 + Phase 14' v1 정책 일관.
- **D2 (K1)**: `Diagnostics.SignatureSection`에 `signatureIcon(tone)` helper 추가. tone(ok/warn/error/neutral) → lucide 컴포넌트(`CheckCircle2 / AlertTriangle / XCircle / Info`).
- **D3 (K1)**: `catalog.commercial.chip` 텍스트의 `⚠` prefix 제거. `ModelCard`는 이미 `<TriangleAlert>` icon prepend 중 — 텍스트 중복.
- **D4 (K1)**: `catalog.adultContent.toggleTitle` / `catalog.hfSearch.triggerTitle` *설명 prose 내부 emoji 참조*를 "경고 아이콘" / "warning chip"로 변환.
- **D5 (K2)**: 최상위 `errors` namespace 신설 + 3 키 (`path-denied` / `path-traversal` / `generic`).
- **D6 (K3)**: `Catalog.tsx` 5건 hardcoded prose → i18n 키. `catalog.refresh.{lastAt, never, title, busy, action}` 신규 + ko/en 양쪽.
- **D7 (K3)**: `apps/desktop/src/ipc/portable.ts::PortableApiError` union에 `path-denied` variant 추가 (백엔드 미러).
- **D8 (K4)**: `PortableImportPanel.tsx` catch 블록에 kind 기반 i18n switch — `e.kind === "path-denied"` → `errors.path-denied`. fallback path `runner::${e.message}` 유지 (백워드 호환).
- **D9**: ADR-0056 단일 ADR로 4건 묶음 (모두 *frontend i18n 일관성* 같은 본질).

## 2. 채택안

### D1+D2+D3+D4 — 이모지 → lucide 일관 정책

**i18n 변경**:
| 키 | 변경 전 | 변경 후 |
|---|---|---|
| `diagnostics.signature.verified` | `✅ 검증됨 ({{source}})` | `검증됨 ({{source}})` |
| `diagnostics.signature.failed` | `❌ 서명 검증 실패: ...` | `서명 검증 실패: ...` |
| `diagnostics.signature.missing` | `⚠ 서명 파일을 받지 못했어요. ...` | `서명 파일을 받지 못했어요. ...` |
| `diagnostics.signature.bundled` | `ℹ 내장 카탈로그 사용 중...` | `내장 카탈로그 사용 중...` |
| `diagnostics.signature.disabled` | `ℹ 서명 검증 비활성 ...` | `서명 검증 비활성 ...` |
| `catalog.commercial.chip` | `⚠ 비상업` | `비상업` |
| `catalog.adultContent.toggleTitle` | `... ⚠ chip과 함께 ...` | `... 경고 아이콘과 함께 ...` |
| `catalog.hfSearch.triggerTitle` | `... ⚠ 라벨이 붙어요.` | `... 경고 라벨이 붙어요.` |

**컴포넌트 변경**:
- `Diagnostics.SignatureSection`:
  ```tsx
  import { AlertTriangle, CheckCircle2, Info, XCircle } from "lucide-react";

  function signatureIcon(tone): typeof CheckCircle2 {
    switch (tone) {
      case "ok": return CheckCircle2;
      case "warn": return AlertTriangle;
      case "error": return XCircle;
      case "neutral": return Info;
    }
  }

  // 렌더링:
  const Icon = signatureIcon(tone);
  <p className="diag-signature-message" role={...}>
    <Icon size={14} aria-hidden="true" className="diag-signature-icon" />
    <span>{message}</span>
  </p>
  ```
- `ModelCard`: 이미 `<TriangleAlert size={12}>` icon prepend — 변경 없음. i18n 텍스트만 `⚠` 제거.

### D5 — errors namespace

ko.json:
```json
"errors": {
  "path-denied": "workspace 밖 경로에는 가져올 수 없어요. 워크스페이스 안의 폴더를 골라 주세요.",
  "path-traversal": "안전하지 않은 경로가 들어 있어 받지 못했어요. 다른 다운로드 링크를 사용해 주세요.",
  "generic": "예기치 못한 오류가 났어요. 잠시 뒤에 다시 시도해 볼래요?"
}
```

en.json:
```json
"errors": {
  "path-denied": "Can't import a path outside the workspace. Please choose a folder inside the workspace.",
  "path-traversal": "Download blocked: the link contains an unsafe path. Please use a different URL.",
  "generic": "Something went wrong. Please try again in a moment."
}
```

### D6 — Catalog refresh prose i18n

```json
"catalog": {
  "refresh": {
    "lastAt": "마지막 갱신: {{when}}",
    "never": "아직 갱신 전이에요",
    "title": "모델 카탈로그 + Ollama / LM Studio 버전 정보를 한 번에 받아와요. ...",
    "busy": "갱신하고 있어요…",
    "action": "다시 불러오기"
  }
}
```

`Catalog.tsx`:
```tsx
{lastRefresh
  ? t("catalog.refresh.lastAt", {
      when: formatRelative(lastRefresh.at_ms),
      defaultValue: `마지막 갱신: ${formatRelative(lastRefresh.at_ms)}`,
    })
  : t("catalog.refresh.never", "아직 갱신 전이에요")}
// + .title / .busy / .action 같은 패턴
```

### D7 — PortableApiError TypeScript union 확장

```typescript
export type PortableApiError =
  | { kind: "already-running"; id: string }
  | { kind: "unknown-job"; id: string }
  | { kind: "export-failed"; message: string }
  | { kind: "import-failed"; message: string }
  | { kind: "verify-failed"; message: string }
  | { kind: "disk"; message: string }
  | { kind: "path-denied"; reason: string };  // 신규
```

### D8 — kind 기반 i18n switch

```tsx
} catch (e) {
  console.warn("startWorkspaceImport failed:", e);
  let msg: string;
  if (e && typeof e === "object" && "kind" in e
      && (e as { kind: string }).kind === "path-denied") {
    msg = "errors.path-denied";
  } else if (e && typeof e === "object" && "message" in e) {
    msg = `screens.settings.portable.import.errors.runner::${(e as { message: string }).message}`;
  } else {
    msg = "screens.settings.portable.import.errors.start";
  }
  setError(msg);
  setPhase("failed");
}
```

`setError`로 i18n 키 string 저장 → 렌더링 시 `t(error)` 호출 (기존 패턴) → 한국어 prose 자동.

## 3. 기각안 + 이유

| # | 기각안 | 이유 |
|---|---|---|
| 1 | 이모지 그대로 두고 lucide-react 추가 prepend (icon + emoji 둘 다) | 시각적 중복 + 사용자 혼란 |
| 2 | i18n에 Unicode symbol(`✓ ✗ ⚠`) 유지 | OS 폰트 의존 (Apple/Windows 다른 렌더링). lucide 일관 정책 |
| 3 | errors namespace를 backend kind와 1:1 매핑 (`errors.portable.path-denied`) | 컨텍스트별 분기 폭증. 최상위 cross-cutting OK |
| 4 | PortableImportPanel catch에서 always raw Display string | locale 토글 시 영어 사용자에 한국어 노출 |
| 5 | Catalog refresh prose를 prop drill | `t()` in-place가 더 간결 |
| 6 | lucide 아이콘을 SVG inline 직접 작성 | tree-shaking 깨짐. named import 정공 |
| 7 | path-denied i18n에 `{{reason}}` 변수 보간 | thiserror reason은 prose Korean — variable interpolation 불필요. 고정 prose가 단순 |
| 8 | K4를 모든 thiserror enum에 적용 (StoreError / KnowledgeError / BenchError 등) | 영향 범위 폭증. 본 sub-phase는 R-A/R-C 변경 분(PortableApiError / ActionError)만 |
| 9 | vitest 자동 테스트 (i18n 키 sync 검사) | typed-i18n-keys crate 도입은 v1.x. 현재는 수동 audit 충당 |
| 10 | CSP unsafe-inline 변경 | R-A에서 결정. 본 sub-phase 범위 외 |
| 11 | catalog.refresh prose를 외부 RawString constant | `catalog.refresh.*` namespace가 더 audit 친화 |
| 12 | path-denied i18n을 catalog/error 양쪽에 중복 등록 | 최상위 errors namespace 단일 = DRY |
| 13 | signature 메시지를 `<Icon /> <span>` 두 줄 분리 (block layout) | 인라인 flex가 자연 — `<p>` 안에서 한 줄. CSS는 `gap: var(--space-1)` 토큰 |
| 14 | backend thiserror Display를 영어로 변경 (locale-agnostic) | 한국어 사용자 fallback 깨짐 + 기존 Phase 7'.b/8'.0 정책 위배 |

## 4. 미정 / 후순위 이월

- **다른 thiserror enum 통합 audit** — `StoreError` / `KnowledgeError` / `BenchError` / `KeyApiError` 등 frontend kind switch 일관 적용. v1.x.
- **typed-i18n-keys crate 도입** — `t("errors.path-denied")` 컴파일 타임 검증. 현재는 runtime fallback. v1.x.
- **`signatureIcon` 반환 타입을 `LucideIcon` 명시 import** — `typeof CheckCircle2`로도 OK이지만 정식 type alias 사용. v1.x.
- **`errors.generic` 사용처 확장** — 본 sub-phase는 키만 추가. 다른 catch 블록의 raw `String(err)`을 generic으로 통일은 별도 sub-phase.
- **`<Icon />` 색상 토큰 wiring** — `signatureIcon`은 currentColor inherit. tone별 색상은 부모 `<section data-tone={tone}>`의 CSS 변수가 결정 — 이미 적용된 `diag-card-tone-{tone}` 클래스로 자연 매핑.
- **lmmaster-desktop test exe Windows DLL 한계** — 변경 없음. R-A/R-B/R-C와 동일 정책.

## 5. 테스트 invariant

본 sub-phase가 깨면 안 되는 invariant:

1. **i18n 이모지 0건**: `apps/desktop/src/i18n/{ko,en}.json`에 컬러 이모지 / Unicode symbol(`✅❌⚠ℹ`) 0건. grep으로 회귀 가드.
2. **i18n 키 ko/en sync**: `catalog.refresh.*` 5 키, `errors.*` 3 키, `diagnostics.signature.*` 5 키 모두 ko/en 양쪽 정의.
3. **Diagnostics SignatureSection lucide icon 렌더링**: tone에 따라 4 lucide 아이콘 중 1개. `aria-hidden="true"` + `<span>` text 분리 (a11y).
4. **PortableApiError union path-denied variant**: TypeScript narrow OK.
5. **PortableImportPanel kind switch**: `e.kind === "path-denied"` → `errors.path-denied` setError. 기존 fallback path 유지.
6. **Catalog refresh i18n**: 5 hardcoded → `t()` 호출. 기존 동작 보존.
7. **vitest sweep**: 기존 테스트 0건 깨짐 — i18n 변경은 *마커 텍스트 단언* 한정 (Phase 14' v1 정책).

본 sub-phase 신규 invariant: **0 (수동 audit 기반)** + 기존 vitest sweep로 회귀 보호. 자동 테스트는 v1.x typed-i18n-keys.

## 6. 다음 페이즈 인계

### 진입 조건

- ✅ R-D.1 (K1 i18n 이모지 거부) 완료
- ✅ R-D.2 (K2 errors.path-denied + path-traversal) 완료
- ✅ R-D.3 (K3 Catalog hardcoded fallback) 완료
- ✅ R-D.4 (K4 thiserror frontend 노출 정책) 완료
- ✅ R-D.5 (ADR-0056 + 결정 노트) 완료
- ⏳ commit + push (사용자 승인 대기)

### 의존성

- **Phase R-E** (Architecture v1.x — POST v0.0.1 release):
  - A1 chat protocol decoupling (3 어댑터 ChatMessage/ChatEvent 공통화)
  - A2 bench trait (RuntimeAdapter 통합)
  - C2 OpenAI compat 공통화 (lmstudio/llama-cpp DTO 중복 제거)
  - P1 KnowledgeStorePool (per-workspace store reuse)
  - P4 channel cancel (Tauri Channel cancellation token)
  - R2 cancellation token (workspace-wide)
  - T3 wiremock (chat_stream invariant 자동화)
- **#31 (Knowledge IPC tokenized)** — R-A 분리분.
- **#38 (knowledge-stack caller wiring)** — R-B 분리분.

### 위험 노트

- **i18n 키 stale fallback**: `defaultValue` prose가 ko.json 변경과 어긋날 가능성. 본 sub-phase에서 audit 대로 갱신 + grep 회귀 가드.
- **Catalog locale 토글 깨짐**: 기존 hardcoded prose가 5건 발견됐으나 다른 페이지에도 잠재. v1.x에서 `pnpm exec tsc -b` + i18n key extractor 도입.
- **kind switch 누락**: 다른 catch 블록(예: PortableExportPanel)에는 적용 X. 사용자 영향 작음 (export는 path-denied 미발생). v1.x 통합 audit.
- **signatureIcon 반환 type**: `typeof CheckCircle2` 사용 — 모든 lucide 아이콘 동일 인터페이스라 작동하지만, 명시 `LucideIcon` import가 더 audit 친화. v1.x cleanup.

### 다음 standby

**Phase R-E (POST v0.0.1)**: 사용자 v0.0.1 ship 후 진입. 아키텍처 cleanup 7건 — chat 프로토콜 통합 / bench trait / OpenAI DTO 공통화 / pool / cancel token 통합 / wiremock 자동화. v0.0.1 ship-blocker는 모두 R-A/R-B/R-C/R-D로 해소 완료 (13 → 17건). v0.0.1 release tag (`v0.0.1`) push 시 release.yml 자동 트리거 → 4-platform 빌드 + draft Release.

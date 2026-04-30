// Workbench 컨텍스트 URL hash 파서 — Phase 12'.a (ADR-0050).
//
// 정책:
// - SSOT는 frontend hash (외부 통신 0 — 모든 라우팅 로컬).
// - 형식: `#/workbench?model=<id>&intent=<IntentId>`. 둘 다 선택적.
// - 빈 hash 또는 파싱 실패 시 `{ model: null, intent: null }` — 기존 사용자 5단계 default 진입.
// - intent는 IntentId 검증을 frontend 측에서 하지 않음 (백엔드 validator가 SSOT).

import type { IntentId } from "../../ipc/catalog";

export interface WorkbenchHashContext {
  model: string | null;
  intent: IntentId | null;
}

/**
 * `window.location.hash` 또는 임의 hash 문자열에서 model/intent를 추출.
 *
 * @param raw 일반적으로 `window.location.hash` ("#/workbench?model=X&intent=Y" 형태).
 *            테스트에서 직접 주입 가능.
 */
export function parseWorkbenchHash(raw: string): WorkbenchHashContext {
  const empty: WorkbenchHashContext = { model: null, intent: null };
  if (!raw) return empty;
  // 선두 `#` 제거.
  const trimmed = raw.startsWith("#") ? raw.slice(1) : raw;
  const qIdx = trimmed.indexOf("?");
  if (qIdx < 0) return empty;
  const queryStr = trimmed.slice(qIdx + 1);
  if (!queryStr) return empty;
  try {
    const params = new URLSearchParams(queryStr);
    const model = params.get("model");
    const intentRaw = params.get("intent");
    return {
      model: model && model.length > 0 ? model : null,
      intent: intentRaw && intentRaw.length > 0 ? (intentRaw as IntentId) : null,
    };
  } catch {
    return empty;
  }
}

/**
 * Catalog → Workbench 라우팅 시 사용. model + (선택) intent로 hash 생성.
 *
 * 예: `buildWorkbenchHash("qwen2.5-coder-7b-instruct", "coding-general")`
 *     → `"#/workbench?model=qwen2.5-coder-7b-instruct&intent=coding-general"`
 */
export function buildWorkbenchHash(
  model: string,
  intent?: IntentId | null,
): string {
  const params = new URLSearchParams({ model });
  if (intent) params.set("intent", intent);
  return `#/workbench?${params}`;
}

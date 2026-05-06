// useAdultContentMode — Phase v0.0.3 — NSFW 3-state 필터 (사용자 요청 2026-05-06).
//
// 정책:
// - localStorage 영속 (`lmmaster.adult_content_mode`).
// - 3 모드:
//   - `hide` (기본): NSFW 모델 숨김 — 첫 진입 + 일반 사용 안전.
//   - `mixed`: NSFW + 일반 모두 표시 — ⚠ chip으로 식별.
//   - `only`: NSFW 모델만 표시 — 챗봇/이미지생성 등 NSFW 활용 시.
// - 외부 통신 0 — 모든 처리 클라이언트 내.
// - 마이그레이션: 기존 `lmmaster.adult_content_allowed` (boolean) 값 자동 변환.
//   - `'true'` → `'mixed'` / `'false'` → `'hide'` / 없음 → `'hide'` (기본).

import { useCallback, useEffect, useState } from "react";

export type AdultContentMode = "hide" | "mixed" | "only";

const STORAGE_KEY = "lmmaster.adult_content_mode";
const LEGACY_KEY = "lmmaster.adult_content_allowed";

/** localStorage에서 모드 읽기 — 레거시 boolean도 자동 마이그레이션. */
function readMode(): AdultContentMode {
  try {
    const raw = globalThis.localStorage?.getItem(STORAGE_KEY);
    if (raw === "hide" || raw === "mixed" || raw === "only") {
      return raw;
    }
    // 레거시 boolean 마이그레이션 (1회).
    const legacy = globalThis.localStorage?.getItem(LEGACY_KEY);
    if (legacy === "true") {
      globalThis.localStorage?.setItem(STORAGE_KEY, "mixed");
      return "mixed";
    }
    if (legacy === "false") {
      globalThis.localStorage?.setItem(STORAGE_KEY, "hide");
      return "hide";
    }
    return "hide";
  } catch {
    return "hide";
  }
}

/** 다음 모드로 사이클: hide → mixed → only → hide. */
export function nextMode(current: AdultContentMode): AdultContentMode {
  switch (current) {
    case "hide":
      return "mixed";
    case "mixed":
      return "only";
    case "only":
      return "hide";
  }
}

export function useAdultContentMode(): [AdultContentMode, (next: AdultContentMode) => void] {
  const [mode, setMode] = useState<AdultContentMode>(() => readMode());

  const update = useCallback((next: AdultContentMode) => {
    setMode(next);
    try {
      globalThis.localStorage?.setItem(STORAGE_KEY, next);
    } catch {
      /* localStorage unavailable — silent fail */
    }
  }, []);

  // 다른 탭/창에서 변경 시 sync.
  useEffect(() => {
    const onStorage = (e: StorageEvent) => {
      if (e.key === STORAGE_KEY) {
        const v = e.newValue;
        if (v === "hide" || v === "mixed" || v === "only") {
          setMode(v);
        }
      }
    };
    if (typeof window !== "undefined") {
      window.addEventListener("storage", onStorage);
      return () => window.removeEventListener("storage", onStorage);
    }
    return undefined;
  }, []);

  return [mode, update];
}

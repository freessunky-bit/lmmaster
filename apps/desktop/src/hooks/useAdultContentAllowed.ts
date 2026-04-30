// useAdultContentAllowed — Phase 13'.f.2.3 (DEFERRED §13'.f.2 §3).
//
// 정책:
// - localStorage 영속 (`lmmaster.adult_content_allowed`).
// - 기본 false — 첫 진입 시 NSFW 모델 hidden.
// - true로 토글 시 카탈로그에 노출 + 노란 ⚠ chip.
// - 외부 통신 0 — 모든 처리 클라이언트 내.

import { useCallback, useEffect, useState } from "react";

const STORAGE_KEY = "lmmaster.adult_content_allowed";

export function useAdultContentAllowed(): [boolean, (next: boolean) => void] {
  const [allowed, setAllowed] = useState<boolean>(() => {
    try {
      return globalThis.localStorage?.getItem(STORAGE_KEY) === "true";
    } catch {
      return false;
    }
  });

  const update = useCallback((next: boolean) => {
    setAllowed(next);
    try {
      globalThis.localStorage?.setItem(STORAGE_KEY, next ? "true" : "false");
    } catch {
      /* localStorage unavailable — silent fail */
    }
  }, []);

  // 다른 탭/창에서 변경 시 sync (드물지만 안전).
  useEffect(() => {
    const onStorage = (e: StorageEvent) => {
      if (e.key === STORAGE_KEY) {
        setAllowed(e.newValue === "true");
      }
    };
    if (typeof window !== "undefined") {
      window.addEventListener("storage", onStorage);
      return () => window.removeEventListener("storage", onStorage);
    }
    return undefined;
  }, []);

  return [allowed, update];
}

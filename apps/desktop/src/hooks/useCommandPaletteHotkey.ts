// 글로벌 hotkey — ⌘K (mac) / Ctrl+K (Win/Linux). Phase 1A.4.e §B2.
//
// 정책:
// - document keydown 리스너 — Tauri WebView 안에서만 동작. OS-level은 회피 (시스템 충돌).
// - preventDefault — Safari/Chrome 주소창 검색 race 차단.
// - e.repeat 스킵 — 홀드 시 재트리거 방지.
// - Esc로 닫기.

import { useEffect } from "react";

import { useCommandPalette } from "../components/command-palette/context";

export function useCommandPaletteHotkey(): void {
  const { open, setOpen } = useCommandPalette();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.repeat) return;
      const isK = e.key === "k" || e.key === "K";
      if (isK && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        setOpen(!open);
        return;
      }
      if (e.key === "Escape" && open) {
        setOpen(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, setOpen]);
}

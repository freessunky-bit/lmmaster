// CommandPaletteProvider — open 상태 + 명령 레지스트리. Phase 1A.4.e §B6.
//
// 정책:
// - xstate와 분리된 React Context — palette는 UI shell 상태, 도메인 진실원이 아님.
// - register/unregister 방식 — 라우트별 useEffect cleanup으로 자동 해제.
// - 시각 노출 외엔 영향 0 — close 시 등록된 명령은 그대로 보존 (다음 open에서 즉시 사용 가능).

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

import type { Command } from "./types";

interface CommandPaletteContextValue {
  open: boolean;
  setOpen: (open: boolean) => void;
  toggle: () => void;
  commands: Command[];
  register: (cmd: Command) => () => void;
}

const CommandPaletteContext = createContext<CommandPaletteContextValue | null>(
  null,
);

export function CommandPaletteProvider({ children }: { children: ReactNode }) {
  const [open, setOpen] = useState(false);
  // Map으로 보관해 동일 id 재등록 시 idempotent 갱신.
  const [registry, setRegistry] = useState<Map<string, Command>>(
    () => new Map(),
  );

  const register = useCallback((cmd: Command) => {
    setRegistry((prev) => {
      const next = new Map(prev);
      next.set(cmd.id, cmd);
      return next;
    });
    return () => {
      setRegistry((prev) => {
        if (!prev.has(cmd.id)) return prev;
        const next = new Map(prev);
        next.delete(cmd.id);
        return next;
      });
    };
  }, []);

  const toggle = useCallback(() => setOpen((v) => !v), []);

  const commands = useMemo(() => Array.from(registry.values()), [registry]);

  const value = useMemo(
    () => ({ open, setOpen, toggle, commands, register }),
    [open, toggle, commands, register],
  );

  return (
    <CommandPaletteContext.Provider value={value}>
      {children}
    </CommandPaletteContext.Provider>
  );
}

export function useCommandPalette(): CommandPaletteContextValue {
  const ctx = useContext(CommandPaletteContext);
  if (!ctx) {
    throw new Error(
      "useCommandPalette must be used inside <CommandPaletteProvider>",
    );
  }
  return ctx;
}

/**
 * 라우트/화면이 자기 명령을 등록하는 hook.
 * 컴포넌트 unmount 또는 deps 변경 시 자동 해제.
 *
 * 주의: 반환 함수 (cmd.perform 등)가 매 렌더 새로 만들어지면 무한 등록 루프 — caller가 useCallback으로 안정화하거나
 * 같은 deps로 동일 id 갱신만 하므로 안전. Map.set이 idempotent.
 */
export function useCommandRegistration(commands: Command[]): void {
  const { register } = useCommandPalette();
  useEffect(() => {
    const unregisters = commands.map((cmd) => register(cmd));
    return () => {
      for (const u of unregisters) u();
    };
    // commands 배열 자체를 deps로 — 대부분 caller가 useMemo로 안정화.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [register, commands]);
}

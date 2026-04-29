// ActiveWorkspaceContext — Phase 8'.1. 활성 workspace 상태 + CRUD action 공유 hook.
//
// 정책 (ADR-0038):
// - 마운트 시 list_workspaces + get_active_workspace 동시 호출.
// - `workspaces://changed` 이벤트 listen → refresh.
// - localStorage backup `lmmaster.active_workspace_id` — 앱 재시작 시 빠른 hydration용.
//   backend가 source of truth, localStorage는 fallback.
// - setActive 호출 시 즉시 optimistic 갱신 후 backend 호출.
// - 에러는 throw — caller가 try/catch로 한국어 토스트 매핑.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import {
  createWorkspace,
  deleteWorkspace,
  getActiveWorkspace,
  listWorkspaces,
  onWorkspacesChanged,
  renameWorkspace,
  setActiveWorkspace,
  type WorkspaceInfo,
} from "../ipc/workspaces";

const LOCAL_STORAGE_KEY = "lmmaster.active_workspace_id";

export interface ActiveWorkspaceContextValue {
  /** 현재 active workspace. 초기 hydration 전까지 null. */
  active: WorkspaceInfo | null;
  /** 모든 workspace 목록 (last_used desc). */
  workspaces: WorkspaceInfo[];
  /** 초기 fetch가 끝났는지 — false면 page는 skeleton 표시 권장. */
  loading: boolean;
  /** active 전환. */
  setActive(id: string): Promise<void>;
  /** 강제 refresh — 외부 트리거용. */
  refresh(): Promise<void>;
  /** 새 workspace 만들기 — 반환값은 생성된 info. */
  create(name: string, description?: string): Promise<WorkspaceInfo>;
  /** 이름 변경. */
  rename(id: string, newName: string): Promise<void>;
  /** 삭제 — 마지막 1개는 backend가 거부. */
  remove(id: string): Promise<void>;
}

const Context = createContext<ActiveWorkspaceContextValue | null>(null);

interface Props {
  children: ReactNode;
}

export function ActiveWorkspaceProvider({ children }: Props) {
  const [active, setActiveState] = useState<WorkspaceInfo | null>(null);
  const [workspaces, setWorkspaces] = useState<WorkspaceInfo[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const unlistenRef = useRef<(() => void) | null>(null);

  // 첫 마운트 — localStorage hint hydration + backend fetch.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      // localStorage hint — 초기 렌더링에서 깜빡임 줄이기.
      try {
        const cached = window.localStorage.getItem(LOCAL_STORAGE_KEY);
        if (cached && !cancelled) {
          setActiveState({
            id: cached,
            name: "",
            description: null,
            created_at_iso: "",
            last_used_iso: null,
          });
        }
      } catch {
        /* ignore */
      }

      try {
        const [list, current] = await Promise.all([
          listWorkspaces(),
          getActiveWorkspace(),
        ]);
        if (cancelled) return;
        setWorkspaces(list);
        setActiveState(current);
        try {
          window.localStorage.setItem(LOCAL_STORAGE_KEY, current.id);
        } catch {
          /* ignore */
        }
      } catch (e) {
        if (!cancelled) {
          // backend 미준비 — 로딩 끝내고 빈 상태로.
          console.warn("workspaces 초기 로드 실패:", e);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }

      // 이벤트 listen — 다른 윈도우/세션이 변경할 수도 있어요.
      try {
        unlistenRef.current = await onWorkspacesChanged((payload) => {
          if (cancelled) return;
          setWorkspaces(payload.workspaces);
          const next = payload.workspaces.find((w) => w.id === payload.active_id);
          if (next) {
            setActiveState(next);
            try {
              window.localStorage.setItem(LOCAL_STORAGE_KEY, next.id);
            } catch {
              /* ignore */
            }
          }
        });
      } catch (e) {
        console.warn("workspaces 이벤트 구독 실패:", e);
      }
    })();

    return () => {
      cancelled = true;
      unlistenRef.current?.();
      unlistenRef.current = null;
    };
  }, []);

  const refresh = useCallback(async () => {
    const [list, current] = await Promise.all([
      listWorkspaces(),
      getActiveWorkspace(),
    ]);
    setWorkspaces(list);
    setActiveState(current);
    try {
      window.localStorage.setItem(LOCAL_STORAGE_KEY, current.id);
    } catch {
      /* ignore */
    }
  }, []);

  const setActiveAction = useCallback(async (id: string) => {
    await setActiveWorkspace(id);
    // 이벤트가 곧 도착하지만 즉시 반영 — 사용자 응답성.
    setActiveState((prev) => prev ?? null);
    setWorkspaces((prev) => {
      const next = prev.find((w) => w.id === id);
      if (next) setActiveState(next);
      return prev;
    });
    try {
      window.localStorage.setItem(LOCAL_STORAGE_KEY, id);
    } catch {
      /* ignore */
    }
  }, []);

  const createAction = useCallback(
    async (name: string, description?: string) => {
      const info = await createWorkspace(name, description);
      // 이벤트가 list 갱신을 가져오지만 optimistic으로 list에 즉시 추가.
      setWorkspaces((prev) => {
        if (prev.some((w) => w.id === info.id)) return prev;
        return [...prev, info];
      });
      return info;
    },
    [],
  );

  const renameAction = useCallback(async (id: string, newName: string) => {
    const updated = await renameWorkspace(id, newName);
    setWorkspaces((prev) =>
      prev.map((w) => (w.id === id ? updated : w)),
    );
    setActiveState((prev) => (prev && prev.id === id ? updated : prev));
  }, []);

  const removeAction = useCallback(async (id: string) => {
    await deleteWorkspace(id);
    setWorkspaces((prev) => prev.filter((w) => w.id !== id));
    // active가 사라졌으면 이벤트가 새 active를 알려줄 거예요.
  }, []);

  const value = useMemo<ActiveWorkspaceContextValue>(
    () => ({
      active,
      workspaces,
      loading,
      setActive: setActiveAction,
      refresh,
      create: createAction,
      rename: renameAction,
      remove: removeAction,
    }),
    [
      active,
      workspaces,
      loading,
      setActiveAction,
      refresh,
      createAction,
      renameAction,
      removeAction,
    ],
  );

  return <Context.Provider value={value}>{children}</Context.Provider>;
}

/** 활성 workspace hook — Provider 안에서만 호출. */
export function useActiveWorkspace(): ActiveWorkspaceContextValue {
  const ctx = useContext(Context);
  if (!ctx) {
    throw new Error(
      "useActiveWorkspace는 <ActiveWorkspaceProvider> 안에서만 호출할 수 있어요.",
    );
  }
  return ctx;
}

/**
 * Optional 변형 — 테스트/storybook에서 Provider 없이 호출 시 null 반환.
 * production 페이지는 [`useActiveWorkspace`]를 권장.
 */
export function useActiveWorkspaceOptional(): ActiveWorkspaceContextValue | null {
  return useContext(Context);
}

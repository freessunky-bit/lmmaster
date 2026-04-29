// xstate `createActorContext` 래퍼 — Provider + 동기 hydrate + 자동 persist.
//
// 정책 (Phase 1A.4.a/c 보강 §1, §2):
// - module-scope에서 localStorage 읽음 → Provider mount 직후 그대로 사용.
// - subscribe로 transition마다 sanitize+save (running 휘발 상태 제외).
// - useOnboardingState / useOnboardingSend / useOnboardingDone 훅 노출 — re-render 격리.
// - InstallEventBridge: install actor의 onEvent 콜백을 actorRef.send로 forward (1A.4.c).

import type { ReactNode } from "react";
import { useEffect } from "react";
import { createActorContext } from "@xstate/react";
import type { Snapshot } from "xstate";

import { setInstallEventBridge } from "./install-bridge";
import { onboardingMachine, sanitizeSnapshotForPersist } from "./machine";
import { loadSnapshot, saveSnapshot } from "./persistence";

// localStorage에서 읽은 snapshot은 unknown — xstate가 hydrate 시 자체 검증한다.
// 타입 시스템에서는 `Snapshot<unknown>`로 캐스트.
const initialSnapshot = loadSnapshot() as Snapshot<unknown> | undefined;

const OnboardingActorContext = createActorContext(
  onboardingMachine,
  initialSnapshot ? { snapshot: initialSnapshot } : undefined,
);

/** Provider — App 최상단에 한 번만 mount. */
export function OnboardingProvider({ children }: { children: ReactNode }) {
  return (
    <OnboardingActorContext.Provider>
      <PersistBridge />
      <InstallEventBridge />
      {children}
    </OnboardingActorContext.Provider>
  );
}

/**
 * install actor가 emit하는 InstallEvent를 머신으로 forward.
 * mount 시 module-scope bridge에 actorRef.send wrapper 등록, unmount 시 해제.
 */
function InstallEventBridge() {
  const actorRef = OnboardingActorContext.useActorRef();
  useEffect(() => {
    setInstallEventBridge((event) => {
      actorRef.send({ type: "INSTALL_EVENT", event });
    });
    return () => {
      setInstallEventBridge(null);
    };
  }, [actorRef]);
  return null;
}

/** subscribe transition마다 snapshot 저장. running 서브상태는 idle로 정규화. */
function PersistBridge() {
  const actorRef = OnboardingActorContext.useActorRef();
  useEffect(() => {
    const sub = actorRef.subscribe((snap) => {
      // getPersistedSnapshot은 deep clone — 직접 snap 사용 가능하지만 v5 권장 path 사용.
      const persisted = actorRef.getPersistedSnapshot();
      saveSnapshot(
        sanitizeSnapshotForPersist(
          persisted as unknown as {
            value: unknown;
            context: import("./machine").OnboardingContext;
            status?: string;
          },
        ),
      );
      void snap;
    });
    return () => sub.unsubscribe();
  }, [actorRef]);
  return null;
}

/** 현재 step value를 string화 — { install: 'idle' } → 'install'. UI는 부모 step만 알면 충분. */
export function useOnboardingStep(): "language" | "scan" | "install" | "done" {
  return OnboardingActorContext.useSelector((s) => {
    const v = s.value;
    if (typeof v === "string") return v as "language" | "scan" | "install" | "done";
    if (v && typeof v === "object" && "install" in v) return "install";
    return "language";
  });
}

/** install 서브상태까지 알아야 하는 컴포넌트(Step 3)용. */
export type InstallSub =
  | "decide"
  | "skip"
  | "idle"
  | "running"
  | "failed"
  | "openedUrl";

export function useOnboardingInstallSub(): InstallSub | undefined {
  return OnboardingActorContext.useSelector((s) => {
    const v = s.value;
    if (v && typeof v === "object" && "install" in v) {
      const sub = (v as { install: unknown }).install;
      if (
        sub === "decide" ||
        sub === "skip" ||
        sub === "idle" ||
        sub === "running" ||
        sub === "failed" ||
        sub === "openedUrl"
      ) {
        return sub;
      }
    }
    return undefined;
  });
}

/** install actor가 받은 가장 최근 InstallEvent. */
export function useOnboardingInstallLatest() {
  return OnboardingActorContext.useSelector((s) => s.context.installLatest);
}

/** 마지막 10건 이벤트 로그 — "자세히 보기" 표시용. */
export function useOnboardingInstallLog() {
  return OnboardingActorContext.useSelector((s) => s.context.installLog ?? []);
}

/** progress (downloaded/total/speed_bps) — 다운로드 단계에서만 의미 있음. */
export function useOnboardingInstallProgress() {
  return OnboardingActorContext.useSelector((s) => s.context.installProgress);
}

export function useOnboardingInstallOutcome() {
  return OnboardingActorContext.useSelector((s) => s.context.installOutcome);
}

export function useOnboardingInstallError() {
  return OnboardingActorContext.useSelector((s) => s.context.installError);
}

export function useOnboardingRetryAttempt() {
  return OnboardingActorContext.useSelector((s) => s.context.retryAttempt);
}

export function useOnboardingModelId() {
  return OnboardingActorContext.useSelector((s) => s.context.modelId);
}

/** scan 서브상태 (Step 2). idle은 always 분기 직후라 거의 보이지 않음. */
export type ScanSub = "idle" | "running" | "done" | "failed";

export function useOnboardingScanSub(): ScanSub | undefined {
  return OnboardingActorContext.useSelector((s) => {
    const v = s.value;
    if (v && typeof v === "object" && "scan" in v) {
      const sub = (v as { scan: unknown }).scan;
      if (sub === "idle" || sub === "running" || sub === "done" || sub === "failed") {
        return sub;
      }
    }
    return undefined;
  });
}

export function useOnboardingLang(): "ko" | "en" {
  return OnboardingActorContext.useSelector((s) => s.context.lang);
}

export function useOnboardingEnv() {
  return OnboardingActorContext.useSelector((s) => s.context.env);
}

export function useOnboardingScanError(): string | undefined {
  return OnboardingActorContext.useSelector((s) => s.context.scanError);
}

export function useOnboardingError(): string | undefined {
  return OnboardingActorContext.useSelector((s) => s.context.error);
}

export function useOnboardingSend() {
  const actorRef = OnboardingActorContext.useActorRef();
  return actorRef.send;
}

/** done(final) 도달 여부 — caller가 onComplete 호출 시점 결정. */
export function useOnboardingDone(): boolean {
  return OnboardingActorContext.useSelector((s) => s.status === "done");
}

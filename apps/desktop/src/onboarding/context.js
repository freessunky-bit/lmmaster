import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useEffect } from "react";
import { createActorContext } from "@xstate/react";
import { setInstallEventBridge } from "./install-bridge";
import { onboardingMachine, sanitizeSnapshotForPersist } from "./machine";
import { loadSnapshot, saveSnapshot } from "./persistence";
// localStorage에서 읽은 snapshot은 unknown — xstate가 hydrate 시 자체 검증한다.
// 타입 시스템에서는 `Snapshot<unknown>`로 캐스트.
const initialSnapshot = loadSnapshot();
const OnboardingActorContext = createActorContext(onboardingMachine, initialSnapshot ? { snapshot: initialSnapshot } : undefined);
/** Provider — App 최상단에 한 번만 mount. */
export function OnboardingProvider({ children }) {
    return (_jsxs(OnboardingActorContext.Provider, { children: [_jsx(PersistBridge, {}), _jsx(InstallEventBridge, {}), children] }));
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
            saveSnapshot(sanitizeSnapshotForPersist(persisted));
            void snap;
        });
        return () => sub.unsubscribe();
    }, [actorRef]);
    return null;
}
/** 현재 step value를 string화 — { install: 'idle' } → 'install'. UI는 부모 step만 알면 충분. */
export function useOnboardingStep() {
    return OnboardingActorContext.useSelector((s) => {
        const v = s.value;
        if (typeof v === "string")
            return v;
        if (v && typeof v === "object" && "install" in v)
            return "install";
        return "language";
    });
}
export function useOnboardingInstallSub() {
    return OnboardingActorContext.useSelector((s) => {
        const v = s.value;
        if (v && typeof v === "object" && "install" in v) {
            const sub = v.install;
            if (sub === "decide" ||
                sub === "skip" ||
                sub === "idle" ||
                sub === "running" ||
                sub === "failed" ||
                sub === "openedUrl") {
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
export function useOnboardingScanSub() {
    return OnboardingActorContext.useSelector((s) => {
        const v = s.value;
        if (v && typeof v === "object" && "scan" in v) {
            const sub = v.scan;
            if (sub === "idle" || sub === "running" || sub === "done" || sub === "failed") {
                return sub;
            }
        }
        return undefined;
    });
}
export function useOnboardingLang() {
    return OnboardingActorContext.useSelector((s) => s.context.lang);
}
export function useOnboardingEnv() {
    return OnboardingActorContext.useSelector((s) => s.context.env);
}
export function useOnboardingScanError() {
    return OnboardingActorContext.useSelector((s) => s.context.scanError);
}
export function useOnboardingError() {
    return OnboardingActorContext.useSelector((s) => s.context.error);
}
export function useOnboardingSend() {
    const actorRef = OnboardingActorContext.useActorRef();
    return actorRef.send;
}
/** done(final) 도달 여부 — caller가 onComplete 호출 시점 결정. */
export function useOnboardingDone() {
    return OnboardingActorContext.useSelector((s) => s.status === "done");
}

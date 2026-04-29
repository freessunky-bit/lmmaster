// Onboarding 상태 기계 — xstate v5 (Phase 1A.4.a/b/c 보강).
//
// 4 단계: language → scan → install → done.
// 1A.4.a: Step 1 / Step 4 실제 동작.
// 1A.4.b: Step 2 환경 점검 — fromPromise(detectEnvironment) actor + 자동 진입 + RETRY + 캐시 분기.
// 1A.4.c: Step 3 — fromPromise(install) actor + signal.abort → cancelInstall + INSTALL_EVENT bridge
//          + decide/skip/idle/running/failed 5 substates + Ollama/LM Studio 카드.
// 1A.4.d: vitest + axe-core (예정).
//
// 영속화: language/modelId만 저장. install/scan 휘발 상태 + env/event 결과는 sanitize에서 제거.
import { setup, assign, fromPromise } from "xstate";
import { detectEnvironment } from "../ipc/environment";
import { cancelInstall, installApp } from "../ipc/install";
import { getInstallEventBridge } from "./install-bridge";
export const onboardingMachine = setup({
    types: {},
    actors: {
        /** 환경 점검 actor — detectEnvironment IPC 호출. */
        scan: fromPromise(async () => detectEnvironment()),
        /**
         * 설치 actor — installApp 호출 + signal.abort → cancelInstall 브리지.
         * onEvent는 module-scope `installEventBridge`로 forward — caller(Step3Install)는
         * INSTALL_EVENT를 actorRef.send로 받아 context에 누적.
         */
        install: fromPromise(async ({ input, signal }) => {
            const onAbort = () => {
                // best-effort cancel — Rust 측은 idempotent (registry.cancel unknown id = no-op).
                void cancelInstall(input.id);
            };
            if (signal.aborted) {
                onAbort();
            }
            else {
                signal.addEventListener("abort", onAbort, { once: true });
            }
            try {
                return await installApp(input.id, {
                    onEvent: (e) => {
                        if (signal.aborted)
                            return; // 늦은 이벤트 드롭
                        const bridge = getInstallEventBridge();
                        if (bridge)
                            bridge(e);
                    },
                });
            }
            catch (e) {
                if (signal.aborted) {
                    // BACK으로 cancel된 경우 xstate가 이미 actor를 detach. onError 발화 안 됨.
                    // 의미 있는 sentinel 반환 (xstate는 무시).
                    return { kind: "opened-url", url: "" };
                }
                throw e;
            }
            finally {
                signal.removeEventListener("abort", onAbort);
            }
        }),
    },
    actions: {
        setLang: assign({
            lang: ({ event }) => event.lang,
        }),
        setModel: assign({
            modelId: ({ event }) => event.id,
        }),
        setEnv: assign({
            env: ({ event }) => {
                const out = event.output;
                return out;
            },
            scanError: () => undefined,
        }),
        setScanError: assign({
            scanError: ({ event }) => {
                const err = event.error;
                return err instanceof Error ? err.message : String(err ?? "unknown error");
            },
        }),
        clearScanResult: assign({
            env: () => undefined,
            scanError: () => undefined,
        }),
        clearError: assign({ error: () => undefined }),
        // ── 1A.4.c install 액션 ──
        applyInstallEvent: assign(({ context, event }) => {
            const ev = event.event;
            const log = [...(context.installLog ?? []), ev].slice(-10);
            let progress = context.installProgress;
            let retryAttempt = context.retryAttempt;
            if (ev.kind === "download") {
                const inner = ev.download;
                if (inner.kind === "progress") {
                    progress = {
                        downloaded: inner.downloaded,
                        total: inner.total,
                        speed_bps: inner.speed_bps,
                    };
                }
                else if (inner.kind === "started") {
                    progress = {
                        downloaded: inner.resume_from,
                        total: inner.total,
                        speed_bps: 0,
                    };
                    retryAttempt = undefined;
                }
                else if (inner.kind === "retrying") {
                    retryAttempt = inner.attempt;
                }
            }
            return {
                installLatest: ev,
                installLog: log,
                installProgress: progress,
                retryAttempt,
            };
        }),
        setOutcome: assign({
            installOutcome: ({ event }) => {
                const out = event.output;
                return out;
            },
            installError: () => undefined,
        }),
        setInstallError: assign({
            installError: ({ event }) => {
                const err = event.error;
                const raw = err instanceof Error ? err.message : String(err ?? "unknown");
                // Rust InstallApiError 직렬화 형태: { kind: "runner", code, message } 또는 기타.
                // 도달 형태 매트릭스:
                // - Tauri invoke().reject → 일반적으로 plain object (Error 아님).
                // - new Error(JSON.stringify({kind, code, message})) → Error 인스턴스. message에 JSON 들어감.
                // 둘 다 처리.
                let code = "unknown";
                let message = raw;
                let parsed = null;
                if (err && typeof err === "object" && !(err instanceof Error)) {
                    parsed = err;
                }
                else {
                    try {
                        parsed = JSON.parse(raw);
                    }
                    catch {
                        // raw 자체를 message로 둠.
                    }
                }
                if (parsed && typeof parsed === "object") {
                    const obj = parsed;
                    if (obj.kind === "runner" && typeof obj.code === "string") {
                        code = obj.code;
                        if (typeof obj.message === "string")
                            message = obj.message;
                    }
                    else if (typeof obj.kind === "string") {
                        code = obj.kind;
                        if (typeof obj.message === "string")
                            message = obj.message;
                    }
                }
                return { code, message };
            },
        }),
        clearInstallState: assign({
            installLatest: () => undefined,
            installLog: () => undefined,
            installProgress: () => undefined,
            installOutcome: () => undefined,
            installError: () => undefined,
            retryAttempt: () => undefined,
        }),
    },
    guards: {
        hasEnv: ({ context }) => context.env !== undefined,
        /** install 단계에서 이미 Ollama 또는 LM Studio가 running이면 자동 SKIP. */
        anyRuntimeRunning: ({ context }) => {
            const rs = context.env?.runtimes ?? [];
            return rs.some((r) => (r.runtime === "ollama" || r.runtime === "lm-studio") &&
                r.status === "running");
        },
        /**
         * 1A.4.d.1 Issue A — install actor가 OpenedUrl outcome을 반환했는지.
         * 이 경우 자동 done 안 가고 사용자에게 "공식 사이트에서 끝내고 와주세요" 안내 후 manual NEXT.
         */
        isOpenedUrlOutcome: ({ event }) => event.output?.kind === "opened-url",
    },
}).createMachine({
    id: "onboarding",
    initial: "language",
    context: {
        lang: "ko",
    },
    on: {
        SET_LANG: { actions: "setLang" },
        RESET_ERROR: { actions: "clearError" },
        // INSTALL_EVENT는 install.running 안에서만 의미 있지만, 어디서 들어와도 payload는 받아서
        // 누적해 둔다 (다른 step 진입 시 오면 그냥 log에만 남음). 늦은 이벤트는 actor 측 신호로 드롭.
        INSTALL_EVENT: { actions: "applyInstallEvent" },
        RESET_INSTALL: { actions: "clearInstallState" },
    },
    states: {
        language: {
            on: {
                NEXT: "scan",
            },
        },
        scan: {
            initial: "idle",
            on: {
                BACK: "language",
                NEXT: { target: "install", guard: "hasEnv" },
            },
            states: {
                idle: {
                    always: [
                        { guard: "hasEnv", target: "done" },
                        { target: "running" },
                    ],
                },
                running: {
                    invoke: {
                        src: "scan",
                        onDone: { target: "done", actions: "setEnv" },
                        onError: { target: "failed", actions: "setScanError" },
                    },
                },
                done: {},
                failed: {
                    on: {
                        RETRY: { target: "idle", actions: "clearScanResult" },
                    },
                },
            },
        },
        install: {
            initial: "decide",
            // BACK은 어떤 install 서브상태에서든 scan으로 — clearInstallState로 휘발 상태 정리.
            on: {
                BACK: { target: "scan", actions: "clearInstallState" },
            },
            states: {
                decide: {
                    always: [
                        { guard: "anyRuntimeRunning", target: "skip" },
                        { target: "idle" },
                    ],
                },
                skip: {
                    // 1.2초 사용자 인지 후 자동 done.
                    after: {
                        1200: "#onboarding.done",
                    },
                },
                idle: {
                    on: {
                        SELECT_MODEL: {
                            target: "running",
                            actions: "setModel",
                        },
                        // 사용자가 "나중에 할게요"를 명시 클릭한 경우 — open_url 후 manual NEXT 등.
                        SKIP: "#onboarding.done",
                    },
                },
                running: {
                    // 1A.4.d.1 Issue B — 매 시도 시작 시 stale outcome/error/log/progress 정리.
                    // RETRY (reenter:true)도 entry를 다시 발화 → 재시도 깨끗.
                    entry: "clearInstallState",
                    invoke: {
                        src: "install",
                        input: ({ context }) => ({ id: context.modelId ?? "" }),
                        // 1A.4.d.1 Issue A — OpenedUrl outcome은 자동 done 안 가고 openedUrl substate에서 manual NEXT 대기.
                        onDone: [
                            {
                                target: "openedUrl",
                                guard: "isOpenedUrlOutcome",
                                actions: "setOutcome",
                            },
                            {
                                target: "#onboarding.done",
                                actions: "setOutcome",
                            },
                        ],
                        onError: {
                            target: "failed",
                            actions: "setInstallError",
                        },
                    },
                    // BACK은 부모 install state의 핸들러가 잡음 — invoke가 멈추면서 signal.abort 발화.
                },
                failed: {
                    on: {
                        // reenter: true로 invoke 재실행 (xstate v5).
                        RETRY: { target: "running", reenter: true },
                        // OpenedUrl 같은 비-failure 케이스도 여기로 안 옴 — failed는 진짜 실패만.
                        SKIP: "#onboarding.done",
                    },
                },
                // 1A.4.d.1 Issue A — install actor가 OpenedUrl outcome 반환 시 머무는 substate.
                // 사용자가 공식 사이트에서 설치 완료 후 NEXT 또는 SKIP, 미완료면 BACK으로 idle 복귀.
                openedUrl: {
                    on: {
                        NEXT: "#onboarding.done",
                        SKIP: "#onboarding.done",
                        BACK: { target: "idle", actions: "clearInstallState" },
                    },
                },
            },
        },
        done: {
            type: "final",
        },
    },
});
/**
 * snapshot persist 시 휘발 상태 정리.
 * - install.* 서브상태 → idle: 다음 부팅에 fresh 진입.
 * - install* / scan / env / scanError context → 제거.
 */
export function sanitizeSnapshotForPersist(snapshot) {
    let value = snapshot.value;
    if (value &&
        typeof value === "object" &&
        "install" in value &&
        typeof value.install === "string") {
        // install 안의 모든 서브상태 → idle로 정규화.
        value = { install: "idle" };
    }
    if (value &&
        typeof value === "object" &&
        "scan" in value &&
        typeof value.scan === "string") {
        value = "scan";
    }
    return {
        ...snapshot,
        value,
        context: {
            ...snapshot.context,
            env: undefined,
            scanError: undefined,
            installLatest: undefined,
            installLog: undefined,
            installProgress: undefined,
            installOutcome: undefined,
            installError: undefined,
            retryAttempt: undefined,
        },
    };
}

// Onboarding xstate machine 단위 테스트. Phase 1A.4.d.1.
//
// 정책:
// - 순수 머신만 테스트 — React/DOM 미사용 (default node env).
// - invoke된 actor는 `machine.provide({ actors: { ... } })`로 mock — 실 IPC 미호출.
// - 비동기 transition 대기는 `vi.waitFor`.
// - Issue A (OpenedUrl substate) + Issue B (running entry clearInstallState) 회귀 보장.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createActor, fromPromise } from "xstate";

import {
  onboardingMachine,
  sanitizeSnapshotForPersist,
  type InstallActorInput,
} from "./machine";
import type { ActionOutcome } from "../ipc/install-events";
import type { EnvironmentReport } from "../ipc/environment";

// ── 픽스처 ─────────────────────────────────────────────────────────────

const FAKE_ENV: EnvironmentReport = {
  hardware: {
    os: { family: "windows", version: "11", arch: "x86_64", kernel: "10.0" },
    cpu: {
      brand: "Intel",
      vendor_id: "GenuineIntel",
      physical_cores: 8,
      logical_cores: 16,
      frequency_mhz: 3000,
    },
    mem: { total_bytes: 16 * 1024 ** 3, available_bytes: 8 * 1024 ** 3 },
    disks: [],
    gpus: [],
    runtimes: {},
    probed_at: "2026-04-27T00:00:00Z",
    probe_ms: 100,
  },
  runtimes: [],
};

const FAKE_ENV_OLLAMA_RUNNING: EnvironmentReport = {
  ...FAKE_ENV,
  runtimes: [{ runtime: "ollama", status: "running" }],
};

const SUCCESS_OUTCOME: ActionOutcome = {
  kind: "success",
  method: "download_and_run",
  exit_code: 0,
  post_install_check_passed: true,
};

const OPENED_URL_OUTCOME: ActionOutcome = {
  kind: "opened-url",
  url: "https://lmstudio.ai/",
};

// 영원히 pending인 actor — running state 진입 자체만 검사할 때.
const pendingActor = <T>() => fromPromise<T>(() => new Promise<T>(() => {}));

// ── 1. language ────────────────────────────────────────────────────────

describe("머신 초기 상태", () => {
  it("초기 state는 language", () => {
    const a = createActor(onboardingMachine).start();
    expect(a.getSnapshot().value).toBe("language");
    expect(a.getSnapshot().context.lang).toBe("ko");
    a.stop();
  });

  it("SET_LANG으로 lang 갱신", () => {
    const a = createActor(onboardingMachine).start();
    a.send({ type: "SET_LANG", lang: "en" });
    expect(a.getSnapshot().context.lang).toBe("en");
    a.stop();
  });

  it("language → NEXT → scan.running (자동 invoke)", () => {
    const m = onboardingMachine.provide({
      actors: { scan: pendingActor<EnvironmentReport>() },
    });
    const a = createActor(m).start();
    a.send({ type: "NEXT" });
    expect(a.getSnapshot().value).toEqual({ scan: "running" });
    a.stop();
  });
});

// ── 2. scan substates ─────────────────────────────────────────────────

describe("scan 서브상태", () => {
  it("scan.running → done with env", async () => {
    const m = onboardingMachine.provide({
      actors: { scan: fromPromise(async () => FAKE_ENV) },
    });
    const a = createActor(m).start();
    a.send({ type: "NEXT" });
    await vi.waitFor(() =>
      expect(a.getSnapshot().value).toEqual({ scan: "done" }),
    );
    expect(a.getSnapshot().context.env).toBe(FAKE_ENV);
    a.stop();
  });

  it("scan.running → failed → RETRY → idle → running → done", async () => {
    let attempts = 0;
    const m = onboardingMachine.provide({
      actors: {
        scan: fromPromise(async () => {
          attempts++;
          if (attempts === 1) throw new Error("netfail");
          return FAKE_ENV;
        }),
      },
    });
    const a = createActor(m).start();
    a.send({ type: "NEXT" });
    await vi.waitFor(() =>
      expect(a.getSnapshot().value).toEqual({ scan: "failed" }),
    );
    expect(a.getSnapshot().context.scanError).toBe("netfail");

    a.send({ type: "RETRY" });
    await vi.waitFor(() =>
      expect(a.getSnapshot().value).toEqual({ scan: "done" }),
    );
    expect(a.getSnapshot().context.scanError).toBeUndefined();
    expect(a.getSnapshot().context.env).toBe(FAKE_ENV);
    a.stop();
  });

  it("hasEnv guard — scan 미통과면 install로 NEXT 차단", () => {
    const m = onboardingMachine.provide({
      actors: { scan: pendingActor<EnvironmentReport>() },
    });
    const a = createActor(m).start();
    a.send({ type: "NEXT" }); // language → scan
    a.send({ type: "NEXT" }); // running 중 NEXT 시도 — guard fail
    expect(a.getSnapshot().value).toEqual({ scan: "running" });
    a.stop();
  });
});

// ── 3. install substates ──────────────────────────────────────────────

describe("install 서브상태", () => {
  /** scan을 통과시켜 install 진입까지 완료한 actor 생성. */
  const reachInstall = async (
    env: EnvironmentReport = FAKE_ENV,
    installActor = fromPromise<ActionOutcome, InstallActorInput>(
      () => new Promise<ActionOutcome>(() => {}),
    ),
  ) => {
    const m = onboardingMachine.provide({
      actors: {
        scan: fromPromise(async () => env),
        install: installActor,
      },
    });
    const a = createActor(m).start();
    a.send({ type: "NEXT" });
    await vi.waitFor(() =>
      expect(a.getSnapshot().value).toEqual({ scan: "done" }),
    );
    a.send({ type: "NEXT" });
    return a;
  };

  it("decide → idle (런타임 미실행)", async () => {
    const a = await reachInstall();
    expect(a.getSnapshot().value).toEqual({ install: "idle" });
    a.stop();
  });

  it("decide → skip → done (Ollama running, 1.2초 후)", async () => {
    vi.useFakeTimers();
    try {
      const a = await reachInstall(FAKE_ENV_OLLAMA_RUNNING);
      expect(a.getSnapshot().value).toEqual({ install: "skip" });
      await vi.advanceTimersByTimeAsync(1200);
      expect(a.getSnapshot().status).toBe("done");
      a.stop();
    } finally {
      vi.useRealTimers();
    }
  });

  it("idle → SELECT_MODEL → running (modelId 설정)", async () => {
    const a = await reachInstall();
    a.send({ type: "SELECT_MODEL", id: "ollama" });
    expect(a.getSnapshot().value).toEqual({ install: "running" });
    expect(a.getSnapshot().context.modelId).toBe("ollama");
    a.stop();
  });

  it("running → success outcome → done (Issue A 가드 미적용 분기)", async () => {
    const a = await reachInstall(
      FAKE_ENV,
      fromPromise<ActionOutcome, InstallActorInput>(async () => SUCCESS_OUTCOME),
    );
    a.send({ type: "SELECT_MODEL", id: "ollama" });
    await vi.waitFor(() => expect(a.getSnapshot().status).toBe("done"));
    expect(a.getSnapshot().context.installOutcome?.kind).toBe("success");
    a.stop();
  });

  it("Issue A — running → opened-url outcome → openedUrl substate (자동 done 차단)", async () => {
    const a = await reachInstall(
      FAKE_ENV,
      fromPromise<ActionOutcome, InstallActorInput>(async () => OPENED_URL_OUTCOME),
    );
    a.send({ type: "SELECT_MODEL", id: "lm-studio" });
    await vi.waitFor(() =>
      expect(a.getSnapshot().value).toEqual({ install: "openedUrl" }),
    );
    expect(a.getSnapshot().status).not.toBe("done");
    expect(a.getSnapshot().context.installOutcome?.kind).toBe("opened-url");

    // Manual NEXT → done.
    a.send({ type: "NEXT" });
    expect(a.getSnapshot().status).toBe("done");
    a.stop();
  });

  it("Issue A — openedUrl에서 BACK → idle (clearInstallState)", async () => {
    const a = await reachInstall(
      FAKE_ENV,
      fromPromise<ActionOutcome, InstallActorInput>(async () => OPENED_URL_OUTCOME),
    );
    a.send({ type: "SELECT_MODEL", id: "lm-studio" });
    await vi.waitFor(() =>
      expect(a.getSnapshot().value).toEqual({ install: "openedUrl" }),
    );
    a.send({ type: "BACK" });
    expect(a.getSnapshot().value).toEqual({ install: "idle" });
    expect(a.getSnapshot().context.installOutcome).toBeUndefined();
    a.stop();
  });

  it("running → BACK → scan (clearInstallState 휘발 정리)", async () => {
    const a = await reachInstall();
    a.send({ type: "SELECT_MODEL", id: "ollama" });
    a.send({ type: "BACK" });
    expect(a.getSnapshot().value).toEqual({ scan: "done" });
    expect(a.getSnapshot().context.installLog).toBeUndefined();
    expect(a.getSnapshot().context.installProgress).toBeUndefined();
    a.stop();
  });

  it("Issue B — failed → RETRY 시 stale outcome/error 정리 (running.entry: clearInstallState)", async () => {
    let calls = 0;
    const m = onboardingMachine.provide({
      actors: {
        scan: fromPromise(async () => FAKE_ENV),
        install: fromPromise<ActionOutcome, InstallActorInput>(async () => {
          calls++;
          if (calls === 1) {
            // InstallApiError.runner JSON 형태.
            throw new Error(
              JSON.stringify({
                kind: "runner",
                code: "DiskFull",
                message: "no space",
              }),
            );
          }
          return SUCCESS_OUTCOME;
        }),
      },
    });
    const a = createActor(m).start();
    a.send({ type: "NEXT" });
    await vi.waitFor(() =>
      expect(a.getSnapshot().value).toEqual({ scan: "done" }),
    );
    a.send({ type: "NEXT" });
    a.send({ type: "SELECT_MODEL", id: "ollama" });
    await vi.waitFor(() =>
      expect(a.getSnapshot().value).toEqual({ install: "failed" }),
    );
    // failed 상태에서 installError가 정의됐어야 함 (정확한 code 파싱은 setInstallError 책임).
    expect(a.getSnapshot().context.installError).toBeDefined();

    // RETRY: reenter:true → running entry → clearInstallState 즉시 발화.
    a.send({ type: "RETRY" });
    // 핵심 — Issue B fix: stale outcome/error가 즉시 정리됐는지.
    expect(a.getSnapshot().context.installError).toBeUndefined();
    expect(a.getSnapshot().context.installOutcome).toBeUndefined();

    await vi.waitFor(() => expect(a.getSnapshot().status).toBe("done"));
    expect(a.getSnapshot().context.installOutcome?.kind).toBe("success");
    a.stop();
  });
});

// ── 4. INSTALL_EVENT applyInstallEvent ─────────────────────────────────

describe("INSTALL_EVENT 누적", () => {
  it("download.progress → installProgress 갱신", () => {
    const a = createActor(onboardingMachine).start();
    a.send({
      type: "INSTALL_EVENT",
      event: {
        kind: "download",
        download: {
          kind: "progress",
          downloaded: 500,
          total: 1000,
          speed_bps: 100,
        },
      },
    });
    expect(a.getSnapshot().context.installProgress).toEqual({
      downloaded: 500,
      total: 1000,
      speed_bps: 100,
    });
    expect(a.getSnapshot().context.installLog).toHaveLength(1);
    a.stop();
  });

  it("download.started → resume_from으로 progress 초기화 + retryAttempt 리셋", () => {
    const a = createActor(onboardingMachine).start();
    a.send({
      type: "INSTALL_EVENT",
      event: {
        kind: "download",
        download: { kind: "retrying", attempt: 2, delay_ms: 500, reason: "x" },
      },
    });
    expect(a.getSnapshot().context.retryAttempt).toBe(2);
    a.send({
      type: "INSTALL_EVENT",
      event: {
        kind: "download",
        download: { kind: "started", url: "u", total: 1000, resume_from: 250 },
      },
    });
    expect(a.getSnapshot().context.installProgress).toEqual({
      downloaded: 250,
      total: 1000,
      speed_bps: 0,
    });
    expect(a.getSnapshot().context.retryAttempt).toBeUndefined();
    a.stop();
  });

  it("log 최대 10건 유지", () => {
    const a = createActor(onboardingMachine).start();
    for (let i = 0; i < 15; i++) {
      a.send({
        type: "INSTALL_EVENT",
        event: { kind: "post-check", status: "pending" },
      });
    }
    expect(a.getSnapshot().context.installLog).toHaveLength(10);
    a.stop();
  });

  it("RESET_INSTALL → 모든 install 컨텍스트 초기화", () => {
    const a = createActor(onboardingMachine).start();
    a.send({
      type: "INSTALL_EVENT",
      event: {
        kind: "download",
        download: { kind: "progress", downloaded: 1, total: 1, speed_bps: 1 },
      },
    });
    expect(a.getSnapshot().context.installProgress).toBeDefined();
    a.send({ type: "RESET_INSTALL" });
    expect(a.getSnapshot().context.installProgress).toBeUndefined();
    expect(a.getSnapshot().context.installLog).toBeUndefined();
    a.stop();
  });
});

// ── 5. sanitizeSnapshotForPersist ──────────────────────────────────────

describe("sanitizeSnapshotForPersist", () => {
  it("install 서브상태 → idle 정규화", () => {
    const out = sanitizeSnapshotForPersist({
      value: { install: "running" },
      context: { lang: "ko" } as never,
    } as never) as { value: unknown };
    expect(out.value).toEqual({ install: "idle" });
  });

  it("install.openedUrl → idle (휘발 — Issue A 추가 substate도 동일)", () => {
    const out = sanitizeSnapshotForPersist({
      value: { install: "openedUrl" },
      context: { lang: "ko" } as never,
    } as never) as { value: unknown };
    expect(out.value).toEqual({ install: "idle" });
  });

  it("scan 서브상태 → 'scan' string으로 정규화", () => {
    const out = sanitizeSnapshotForPersist({
      value: { scan: "failed" },
      context: { lang: "ko" } as never,
    } as never) as { value: unknown };
    expect(out.value).toBe("scan");
  });

  it("env / scanError / install* 컨텍스트 제거", () => {
    const out = sanitizeSnapshotForPersist({
      value: "scan",
      context: {
        lang: "ko",
        env: FAKE_ENV,
        scanError: "x",
        installLog: [{}],
        installError: { code: "x", message: "y" },
        installOutcome: SUCCESS_OUTCOME,
        installProgress: { downloaded: 1, total: 1, speed_bps: 1 },
        retryAttempt: 3,
      } as never,
    } as never) as { context: Record<string, unknown> };
    expect(out.context.env).toBeUndefined();
    expect(out.context.scanError).toBeUndefined();
    expect(out.context.installLog).toBeUndefined();
    expect(out.context.installError).toBeUndefined();
    expect(out.context.installOutcome).toBeUndefined();
    expect(out.context.installProgress).toBeUndefined();
    expect(out.context.retryAttempt).toBeUndefined();
    expect(out.context.lang).toBe("ko");
  });
});

// 마지막 정리 — leak 방지.
afterEach(() => {
  vi.useRealTimers();
});
beforeEach(() => {
  // 필요 시 본 파일 단위 reset 자리. setup.ts가 글로벌 처리.
});

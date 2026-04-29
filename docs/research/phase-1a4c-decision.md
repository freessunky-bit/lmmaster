# Phase 1A.4.c — Step 3 첫 모델/런타임 설치 결정 노트

> 보강 리서치 (2026-04-27) 종합. xstate v5 + Tauri Channel<InstallEvent> + InstallProgress UX.

## 1. 핵심 결정

| 항목 | 결정 | 근거 |
|---|---|---|
| Sub-phase 범위 | **런타임 설치 (Ollama/LM Studio) 전용** — 모델 큐레이션은 Phase 2'로 분리 | Step 3에서 모델까지 받으려면 카탈로그·매니페스트 schema 확장 필요 → 1A.4.c 폭발. 현재는 Ollama 설치만 검증, 모델은 카탈로그에서 |
| xstate actor type | **`fromPromise<ActionOutcome, { id: string }>`** | Promise lifecycle = state lifecycle. `fromCallback`은 output 없음 → `onDone.output` 못 씀 |
| Cancel 패턴 | `signal.addEventListener("abort", onAbort, { once: true })` 안에서 `cancelInstall(id)` | xstate state 종료 → actor 정지 → signal abort → cancelInstall. 단일 진실원 |
| Cancel 시 Promise 처리 | `try/finally`로 listener 해제 + `signal.aborted`면 Promise rejection 조용히 swallow | xstate 이미 detach — onError는 안 발화. console 폴루션만 회피 |
| Channel<InstallEvent> 브리지 | **module-scope `let installEventBridge` 변수 + Provider 안 useEffect로 bind/unbind** | Pattern 2 (research §2). 다른 옵션(context에 callback 저장)은 영속화 깨짐. 단일 wizard라 singleton OK |
| 명령 트리거 | 카드 클릭 → `actorRef.send({ type: "SELECT_MODEL", id })` → `install.idle → install.running` | machine 진입은 explicit, auto-invoke 안 함 (StrictMode race 회피) |
| `install.running` 자식 actor | invoke `src: "install"` + `input: ({ context }) => ({ id: context.modelId! })` | input은 context에서만 derive — onEvent는 module-scope bridge가 forward |
| 늦게 도착하는 Cancelled 이벤트 | `onEvent` 안 `if (signal.aborted) return;`로 드롭 | actor 정지 후 INSTALL_EVENT는 무의미 |
| Install 완료 분기 | `OpenedUrl`이면 자동 NEXT 안 함 ("공식 사이트에서 설치 끝내고 와주세요" 안내) | 사용자가 실제 설치 완료를 확인해야 함 |
| Reboot-required | inline 패널 (모달 아님) — 2 버튼 ("지금 다시 시작" / "나중에 할게요") | 1A.4.c 스코프에선 "지금 다시 시작" 미구현 (별도 IPC 필요) → "나중에 할게요"만. 향후 Phase 6' OS reboot IPC 합류 |
| Failure 분기 | `failed` 서브상태 — RETRY = `target: "running"`, `reenter: true` | xstate v5 `reenter: true`로 actor 재invoke. Rust `InstallRegistry.try_start` 충돌 회피는 idempotent finish guard로 자동 처리 (이미 InstallGuard Drop). RETRY 버튼은 500ms debounce |
| Already-running SKIP | `install.decide` always 분기 — env에서 ollama|lm-studio가 running이면 `skip` 서브상태 후 1.2s 대기 → done | 사용자 confirmation 시간 |
| 영속화 | `sanitizeSnapshotForPersist`에서 install 서브상태 + install* context 모두 비움 | 휘발 상태 — 다음 부팅에 fresh 진입 |

## 2. Machine 확장 (요약)

```
on: { ..., INSTALL_EVENT: { actions: 'applyInstallEvent' } }   // 모든 step에서 수신 (running 외엔 무시)
context: { ..., installLatest?, installLog?[<=10], installProgress?, installOutcome?, installError?, retryAttempt? }

actors: {
  install: fromPromise<ActionOutcome, { id: string }>(...) // signal.abort → cancelInstall
}

guards: { anyRuntimeRunning }   // env.runtimes.some(r => (r.runtime==='ollama'||r.runtime==='lm-studio') && r.status==='running')

states.install:
  initial: 'decide'
  states:
    decide:  always: [{guard:anyRuntimeRunning, target:'skip'}, {target:'idle'}]
    skip:    after: { 1200: '#onboarding.done' }
    idle:    on: { SELECT_MODEL: { target:'running', actions:'setModel' } }
    running: invoke: { src:'install', input, onDone:{target:'#done', actions:'setOutcome'},
                       onError:{target:'failed', actions:'setInstallError'} }
             on: { BACK:'idle' }
    failed:  on: { RETRY: { target:'running', reenter:true } }
  on: { BACK: 'scan' }   // global back from any sub of install
```

## 3. Module-scope bridge 패턴

```ts
// context.tsx (or new bridge.ts)
import type { InstallEvent } from '../ipc/install-events';

let installEventBridge: ((e: InstallEvent) => void) | null = null;

export function setInstallEventBridge(fn: typeof installEventBridge) {
  installEventBridge = fn;
}
export function getInstallEventBridge() {
  return installEventBridge;
}

// In Provider — register actorRef.send wrapper at mount
function InstallEventBridge() {
  const actorRef = useActorRef();
  useEffect(() => {
    setInstallEventBridge((e) => actorRef.send({ type: "INSTALL_EVENT", event: e }));
    return () => setInstallEventBridge(null);
  }, [actorRef]);
  return null;
}

// In machine — fromPromise actor body
fromPromise<ActionOutcome, { id: string }>(async ({ input, signal }) => {
  const onAbort = () => { void cancelInstall(input.id); };
  if (signal.aborted) onAbort();
  else signal.addEventListener("abort", onAbort, { once: true });
  try {
    return await installApp(input.id, {
      onEvent: (e) => {
        if (signal.aborted) return;
        getInstallEventBridge()?.(e);
      },
    });
  } catch (e) {
    if (signal.aborted) {
      // sentinel — xstate detached, never delivered
      return { kind: 'opened-url', url: '' } as ActionOutcome;
    }
    throw e;
  } finally {
    signal.removeEventListener("abort", onAbort);
  }
})
```

## 4. InstallProgress UX

- 단계 라벨: "받고 있어요" / "압축 풀고 있어요" / "확인하고 있어요" / "거의 끝났어요"
- progress bar: `<progress value max>` + `aria-label="설치 진행"`
- 메타: speed (MB/s tabular-nums) + ETA ("약 12초 남았어요" / "약 1분 5초 남았어요")
- "자세히 보기" `<details>` — 마지막 10 이벤트
- 취소: BACK 이벤트 (signal abort가 cancelInstall trigger)
- Reboot 패널: inline. "지금 다시 시작" / "나중에 할게요"
- Failure: error code → 14건 한국어 매핑 + RETRY (500ms debounce) + 기술 message는 `<pre>`

## 5. 에러 코드 → 한국어 매핑 (i18n `onboarding.install.error.*`)

| code | 한국어 |
|---|---|
| download-failed | 받기에 실패했어요. 네트워크를 확인하고 다시 시도해 볼까요? |
| extract-failed | 압축 풀기에 실패했어요. 디스크 공간을 확인해 주세요. |
| io-error | 파일을 쓰지 못했어요. 권한을 확인해 주세요. |
| installer-exit-nonzero | 설치 프로그램이 정상 종료하지 않았어요. |
| installer-killed | 설치 프로그램이 도중에 멈췄어요. |
| installer-timeout | 시간이 너무 오래 걸렸어요. 다시 시도해 볼까요? |
| cancelled | 설치를 취소했어요. |
| open-url-failed | 브라우저를 여는 데 실패했어요. 직접 사이트를 열어주세요. |
| unsupported | 이 환경은 아직 지원하지 않아요. |
| invalid-spec | 매니페스트가 잘못됐어요. 개발팀에 알려주세요. |
| sink-closed | 진행 정보가 끊겼어요. 다시 시도해 볼까요? |
| no-install-section | 설치 정보가 없어요. |
| no-platform-branch | 이 OS에서는 설치할 수 없어요. |
| init-failed | 설치 준비 중 문제가 생겼어요. |
| (default) | 알 수 없는 오류가 났어요. 다시 시도해 볼까요? |

## 6. 카드 분기

| 조건 | 표시 | 동작 |
|---|---|---|
| Ollama not-installed | "Ollama" 카드 + "추천" 핀 + 자동 설치 안내 | SELECT_MODEL("ollama") → install.running |
| LM Studio not-installed | "LM Studio" 카드 + "EULA 안내" 핀 + open_url 안내 | SELECT_MODEL("lm-studio") → install.running → OpenedUrl outcome → 안내 + 수동 NEXT |
| Ollama running | 카드 muted + "이미 사용 중" 핀 | disabled (decide always가 자동 SKIP) |
| LM Studio running | 카드 muted + "이미 사용 중" 핀 | 동일 |
| 둘 다 running | 카드 미노출 → decide always → skip → 1.2s → done | "이미 사용 중이에요. 다음 단계로 갈게요" 안내 |
| RAM low / disk low | 카드 hint 노출 | clickable (서버 측 min_ram_mb로 차단) |

## 7. 비목표 (1A.4.c 외)

- 모델 큐레이션 (EXAONE/HCX-SEED 등) — Phase 2'
- "지금 다시 시작" 실제 reboot IPC — Phase 6'
- vitest + axe-core — Phase 1A.4.d
- LM Studio 설치 후 자동 모델 매칭 — Phase 2'
- Resume from .partial UI — 다음 시도에 자동 동작

## 8. 검증 체크리스트

- `pnpm exec tsc -b` 통과
- `pnpm run build` 통과 (Vite production)
- `cargo clippy --workspace --all-targets -- -D warnings` 통과
- `cargo test --workspace` 통과 (Rust 변경 없음 — 100건 유지)
- (사용자) `pnpm tauri:dev` 4단계 마법사 — Step 2 통과 후 Step 3에서 Ollama/LM Studio 카드 → "받을게요" → InstallProgress → 완료/실패/취소 모두 한국어. 이미 running이면 자동 SKIP.

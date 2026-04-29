# Phase 0 보강 리서치 — 종합 리포트

> 4개 영역 병렬 리서치 결과 종합. 각 영역은 별도 에이전트가 공식 문서·GitHub·베스트 프랙티스를 조사.
> 출처는 각 섹션 끝에 정리.

## 0. 적용 결정 요약 (5분 안에 보기)

| 영역 | 변경 | 근거 |
|---|---|---|
| Axum 버전 | 0.7 → **0.8.9** | 2026-04-14 릴리스. path syntax `/:id` → `/{id}` 호환만 주의(현재 path param 없음) |
| Tauri spawn | `tokio::spawn` 금지, **`tauri::async_runtime::spawn`** 사용 | Tauri 2가 자체 tokio 런타임을 소유. 잘못 spawn 시 무음 실패(tauri#11831) |
| 포트 선택 | **127.0.0.1:0 + `TcpListener::local_addr()`** | OS 할당 포트, 충돌 회피. `axum::serve`가 listener를 consume하므로 **bind 직후** local_addr 호출 |
| Shutdown | **`RunEvent::ExitRequested` + CancellationToken** | `WindowEvent::CloseRequested`만으로는 Alt+F4·taskkill·OS 셧다운에 동작 안 함(tauri#10555) |
| 401 응답 | **OpenAI envelope** `{error:{message,type,code}}` | 미준수 시 openai-python에서 `APIError`로 떠 디버깅 어려움 |
| SSE | **`KeepAlive::new().interval(15s)`** + **`[DONE]`** sentinel 필수 | 프록시가 idle SSE를 30~120초에 끊음. `[DONE]`은 LangChain·LiteLLM·vLLM 클라이언트가 기대 |
| CORS | auth layer **바깥**에 적용 | preflight OPTIONS가 401에 막히는 클래식 함정 |
| 타입 공유 | **specta v2 + tauri-specta v2** 도입 | shared-types와 SDK 타입의 단일 진실원, drift 방지. ADR-0015 추가 |
| 모노레포 target | **`CARGO_TARGET_DIR`** 또는 per-crate `build.target-dir` | Tauri 모바일 번들러가 `src-tauri/target` 하드코딩(tauri#5865) |
| 폰트 | Inter → **Pretendard Variable** 우선 | 한국어 width consistency, tabular-nums 한영 혼합 안정. Toss/Naver 표준 |
| 포커스 링 | 2-layer box-shadow + `:focus-visible` | WCAG C40, Linear/Raycast 표준 |
| 디자인 토큰 | Radix alpha 스케일(`*-a-1..6`) + `*-on` 의미 짝 추가 | hover/press wash 합성, 의미 색 위 텍스트 결정 |
| Tauri capability | `core:` prefix 사용 | 1.x 스타일 `app:default`는 silently no-op |
| Body 한도 | `RequestBodyLimitLayer::new(2MB)` | DoS 보호 |
| Timeout | `TimeoutLayer::new(600s)` | 긴 스트림 허용. 일반 JSON 라우트는 더 짧게 별도 레이어 가능 |
| 비주얼 회귀 | **Storybook+Chromatic** (디자인 시스템) + **Playwright `toHaveScreenshot`** (앱) | 2026 컨센서스 분리 |

새 ADR 1건: **ADR-0015 — Type sharing via specta + tauri-specta**.

기존 ADR은 모두 유지.

---

## 1. Tauri 2 + 임베디드 Axum (핵심 부팅 패턴)

### 채택할 패턴

**P1. setup hook + `tauri::async_runtime::spawn`**
```rust
.setup(|app| {
    let handle = app.handle().clone();
    tauri::async_runtime::spawn(async move { run_gateway(handle).await });
    Ok(())
})
```
- `#[tokio::main]`이나 `tokio::spawn` 사용 금지. `tauri-by-simon` 문서가 명시.

**P2. port=0 → bind → local_addr → serve**
```rust
let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
let port = listener.local_addr()?.port();      // serve 전에 읽어야 함
axum::serve(listener, router)
    .with_graceful_shutdown(async move { cancel.cancelled().await })
    .await?;
```

**P3. 포트를 frontend로 전달: `app.manage()` + `app.emit("gateway://ready", port)`**
- `app.manage(GatewayState { port, cancel })` → 동기적으로 Tauri command가 읽을 수 있음.
- `app.emit("gateway://ready", port)` → setup이 끝난 뒤(bind 완료 시점에) React가 `listen()`으로 받음.
- Tauri 2에선 `Emitter` trait import 필요.

**P4. shutdown은 `RunEvent::ExitRequested`**
```rust
.run(|handle, event| match event {
    tauri::RunEvent::ExitRequested { .. } => {
        if let Some(state) = handle.try_state::<GatewayState>() {
            state.cancel.cancel();
        }
    }
    _ => {}
});
```
- `WindowEvent::CloseRequested`는 Alt+F4·taskkill·OS 셧다운 누락(tauri#10555).

**P5. capability JSON으로 IPC 표면 최소화**
- `src-tauri/capabilities/main.json`에 허용된 command/permission만 명시.
- 매칭 안 되는 webview는 **default-deny**.

### 안티패턴

- `#[tokio::main] async fn main` + Tauri Builder → 런타임 충돌.
- 포트 1420/3000 하드코딩 → 다른 인스턴스/dev 서버와 충돌.
- `WindowEvent::CloseRequested`만으로 cleanup → 누락 케이스 다수.
- `local_addr` 호출 전에 listener를 `axum::serve`에 넘김 → consume됨.

### Tauri 2.x gotcha (2025 후반~2026 초반)

- 모든 코어 capability에 `core:` prefix 필수. `core:default`, `core:app:default`.
- `default-tls` feature가 `native-tls`로 리네임.
- `axum::Server`는 0.7에서 제거 — `axum::serve(listener, router)`만.
- `AppHandle::restart`는 `RunEvent::Exit`를 스킵할 수 있음(tauri#12310) — 영구 cleanup은 `ExitRequested`에서.

### 출처
- logankeenan/tauri-axum-htmx, jetli/rust-yew-axum-tauri-desktop
- tokio-rs/axum `examples/sse`, `examples/graceful-shutdown`
- tauri-apps/tauri discussions #11399, #11831; issues #10555, #14558
- Tauri 2 공식 문서: capabilities, calling-frontend
- tauri::async_runtime docs.rs

---

## 2. Axum 프로덕션 패턴 (SSE + 미들웨어 + shutdown)

### 채택할 의존성 버전

```toml
axum        = "0.8"
axum-extra  = "0.10"          # TypedHeader, Authorization<Bearer>
tower       = "0.5"
tower-http  = { version = "0.6", features = ["trace","cors","request-id","limit","timeout"] }
tokio       = { version = "1", features = ["full"] }
tokio-stream= "0.1"
tracing     = "0.1"
tracing-subscriber = "0.3"
```

### SSE 핸들러 (OpenAI-호환)

```rust
use axum::response::sse::{Event, KeepAlive, Sse};
use std::convert::Infallible;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};

let (tx, rx) = tokio::sync::mpsc::channel::<ChunkJson>(64);
tokio::spawn(state.engine.generate(req, tx));      // producer
let stream = ReceiverStream::new(rx)
    .map(|c| Ok::<_, Infallible>(Event::default().json_data(&c).unwrap()))
    .chain(tokio_stream::once(Ok(Event::default().data("[DONE]"))));
Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)).text(":kp"))
```
- **`Event::json_data(&v)`** 가 `Event::default().data(serde_json::to_string(&v)?)` 보다 1회 alloc 적음.
- `[DONE]` sentinel은 OpenAI 공식 docs에 명시 안 됐어도 모든 클라이언트가 기대 — 반드시 emit.

### Bearer auth (OpenAI 호환 401 envelope)

```rust
use axum_extra::{TypedHeader, headers::{Authorization, authorization::Bearer}};

pub async fn require_bearer(
    State(km): State<Arc<KeyManager>>,
    bearer: Option<TypedHeader<Authorization<Bearer>>>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let Some(TypedHeader(Authorization(b))) = bearer else { return unauth("missing_api_key"); };
    match km.verify(b.token()).await {
        Ok(p) => { req.extensions_mut().insert(p); next.run(req).await }
        Err(_) => unauth("invalid_api_key"),
    }
}

fn unauth(code: &str) -> Response {
    (StatusCode::UNAUTHORIZED,
     [("WWW-Authenticate", "Bearer realm=\"lmmaster\"")],
     Json(json!({"error":{"message":"Unauthorized","type":"invalid_request_error","code":code}})))
        .into_response()
}
```

### 크로스 플랫폼 graceful shutdown

```rust
async fn shutdown_signal() {
    let ctrl_c = async { let _ = tokio::signal::ctrl_c().await; };
    #[cfg(unix)]
    let term = async {
        let mut s = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate()).expect("sigterm");
        s.recv().await;
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
}
```
- 우리 케이스는 Tauri의 CancellationToken과 OR로 묶음.

### 미들웨어 스택 (외→내)

```
SetRequestIdLayer → TraceLayer → PropagateRequestIdLayer
   → TimeoutLayer(600s for stream routes; 30s for json)
   → RequestBodyLimitLayer(2MB)
   → CorsLayer (route별: /v1/* permissive, /_admin/* strict)
   → require_bearer (route_layer; CORS 안에 배치 — preflight 우회)
```

### Pitfall 모음

- **Sse::keep_alive 누락** → 프록시가 30~120초에 끊음.
- **CompressionLayer를 SSE에 적용** → gzip이 버퍼링하면서 스트림 깨짐. JSON 라우트에만.
- **Head-of-line blocking** — generation을 같은 future에서 await하지 말고 별도 spawn → mpsc.
- **Path syntax 0.8** — `/:id` 패닉. `/{id}` 사용.
- **TraceLayer span에 req_id 안 들어감** — `record()` 호출하는 작은 미들웨어 추가 필요.
- **CORS + auth 순서 바뀜** — preflight OPTIONS가 401에 막힘. CORS는 auth layer 바깥.

### 출처
- tokio-rs/axum: examples/sse, examples/graceful-shutdown, examples/jwt
- plabayo/tokio-graceful (참고만)
- LukeMathWalker/zero-to-production
- OpenAI streaming guide + cookbook
- llamastack/llama-stack issue #4744 ([DONE] sentinel)

---

## 3. Rust + TS 모노레포

### 결정 사항

- **CARGO_TARGET_DIR 설정** — Tauri 번들러가 `src-tauri/target`을 일부 경로에서 하드코딩. workspace 루트에 통합 target 두려면 환경변수.
- **`apps/desktop/src-tauri`를 workspace members에 명시 추가** (이미 함). 글롭 미지원.
- **버전 전략**:
  - Library crate: `version.workspace = true`로 통일.
  - 데스크톱 binary crate: 자기 버전(앱 릴리스 따라감).
  - `@lmmaster/design-system`, `@lmmaster/sdk`: 독립 semver.
- **타입 공유 전략**: ADR-0015로 별도 결정. **specta + tauri-specta v2 + 빌드타임 codegen + js-sdk 수작업 re-export**.
- **pnpm catalogs 채택** — react/react-dom/typescript/vite/vitest를 catalog에. `catalogMode: strict`.
- **CI 워크플로 분리** — `lint-rs.yml`, `lint-js.yml`, `test-crates.yml`, `test-packages.yml`, `build-desktop.yml`. paths filter + concurrency cancel.
- **Changesets + release-plz** 폴리글롯 패턴.

### 출처
- spacedriveapp/spacedrive (가장 유사한 구조)
- tauri-apps/tauri 자체 CI
- cloudflare/workers-sdk (catalog 채택 패턴)
- vercel/turborepo
- lapce/lapce
- tauri 이슈 #5865, #4232, #11859
- specta-rs/specta, specta-rs/tauri-specta
- pnpm catalogs docs

---

## 4. 디자인 시스템 보강

### 토큰 추가/변경 (즉시 반영)

추가:
```css
/* Radix-style alpha scales — surface 위 hover/press 합성 */
--white-a-1: rgba(255,255,255,0.03);
--white-a-2: rgba(255,255,255,0.06);
--white-a-3: rgba(255,255,255,0.09);
--white-a-4: rgba(255,255,255,0.13);
--white-a-5: rgba(255,255,255,0.18);
--black-a-3: rgba(0,0,0,0.30);
--black-a-6: rgba(0,0,0,0.60);
--primary-a-2: rgba(56,255,126,0.10);
--primary-a-3: rgba(56,255,126,0.16);
--primary-a-6: rgba(56,255,126,0.40);

/* 의미 색 위 텍스트 */
--info-on:   #04111f;
--warn-on:   #1a0e02;
--error-on:  #1f0509;
--accent-on: #0e0420;

/* shadcn parity */
--input-bg: var(--surface);
--ring:     var(--primary);

/* Overlay */
--overlay: var(--black-a-6);

/* 추가 motion */
--dur-instant: 80ms;
--ease-emphasized: cubic-bezier(0.16, 1, 0.3, 1);
```

변경:
- `--focus-ring`을 box-shadow 2-layer 형태로 재정의:
  ```css
  --focus-ring: 0 0 0 2px var(--bg), 0 0 0 4px var(--primary);
  ```
  `:focus-visible`에서만 적용. WCAG C40 충족.

- 폰트 스택 우선순위 변경(Pretendard 우선):
  ```css
  --font-body: "Pretendard Variable", Pretendard, -apple-system, BlinkMacSystemFont,
               "Apple SD Gothic Neo", "Segoe UI", Roboto, "Noto Sans KR", "Malgun Gothic",
               system-ui, sans-serif;
  --font-mono: "JetBrains Mono", "JetBrainsMonoHangul", "D2Coding",
               "Sarasa Mono K", "Apple SD Gothic Neo", ui-monospace, Menlo, Consolas, monospace;
  ```

- `prefers-reduced-motion` 블록 추가:
  ```css
  @media (prefers-reduced-motion: reduce) {
    :root { --dur-instant: 0ms; --dur-fast: 0ms; --dur-base: 0ms; --dur-slow: 0ms; }
  }
  ```

### 폰트 로딩 전략

- Pretendard Variable의 dynamic-subset CSS(jsDelivr)를 import. `font-display: swap`. 초기 ~30KB.
- Toss/Naver-class 앱들이 사용하는 표준 패턴.

### 비주얼 회귀

- `packages/design-system`: Storybook + Chromatic.
- `apps/desktop`: Playwright `toHaveScreenshot()` 시맨틱 사용.
- M4(D5 단계)에서 베이스라인.

### 출처
- workos/radix-ui-colors
- shadcn-ui/ui
- vercel/geist (font 패키지)
- orioncactus/pretendard
- Jhyub/JetBrainsMonoHangul
- Park UI semantic tokens
- WCAG C40

---

## 5. Phase 0 다음 단계 (이 보강에 따라 즉시 반영)

1. Cargo.toml 워크스페이스 deps 갱신 (axum 0.8, tower-http 0.6, axum-extra 0.10, tokio-util 0.7 추가).
2. ADR-0015 작성 (specta 타입 공유).
3. 디자인 토큰 위 추가/변경 적용. base.css의 폰트 스택, focus-visible 갱신.
4. apps/desktop/src-tauri/main.rs를 보강 패턴으로 재작성 (P1~P5 적용).
5. crates/core-gateway/src/lib.rs를 axum 0.8 기반 production 패턴으로 재작성: build_router, run_gateway 함수, shutdown signal.
6. apps/desktop/src-tauri/capabilities/main.json 추가.
7. crates/core-gateway/tests/health_test.rs — 통합 테스트.
8. apps/desktop/src/App.tsx에서 `gateway://ready` listen + 한국어 상태 표시.
9. RESUME.md 갱신.

검증은 fmt/clippy/build/test 통과 + dev 실행에서 한국어 UI에 gateway 포트가 표시되는지 확인.

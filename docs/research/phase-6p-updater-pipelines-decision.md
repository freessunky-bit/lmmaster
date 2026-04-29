# Phase 6'.a — Auto-Updater + Pipelines 결정 노트

> 작성일: 2026-04-28
> 상태: 확정 (scaffold + ADR-0025(Pipelines) / ADR-0026(Auto-Updater) 동반 발행)
> 선행: ADR-0013 (외부 통신 0), ADR-0016 (wrap-not-replace), ADR-0019 (always-latest hybrid bootstrap), ADR-0022 (gateway routing)
> 후행: Phase 6'.b (Pipelines 모듈 구현 + gateway 라우터 통합), Phase 6'.c (auto-updater Tauri IPC + JetBrains-style 토스트 UI)

> 본 결정 노트는 `docs/DECISION_NOTE_TEMPLATE.md` 6-섹션 의무 양식.
> §2 "기각된 대안 + 사유"는 다음 세션이 같은 함정에 빠지지 않게 하는 negative-space 보존 장치.

---

## §1 결정 요약 (8가지)

1. **Auto-Updater = `UpdateSource` trait + `GitHubReleasesSource` 1순위 + `MockSource` 테스트용** — 소스 추상화로 자체 호스팅/CDN 미러로 v1.x 확장 여지를 보존.
2. **6시간 폴 간격 (DEFAULT_INTERVAL = 21,600s)** — JetBrains Toolbox / VS Code 6h 패턴과 일치. 사용자 설정으로 1h~24h 조정 가능 (Phase 6'.c에서 Settings UI 노출).
3. **semver `is_outdated(current, latest)` — pre-release < release, build metadata 무시, leading 'v'/'V' strip** — GitHub release tag 관행(`v1.2.3`)을 lenient하게 흡수.
4. **사용자 동의 후만 다운로드. silent install 절대 금지** — ADR-0016 wrap-not-replace + LM Studio EULA 정책 준수. 본체(LMmaster) 본인 업데이트도 사용자 컨펌 후 (JetBrains-style "다음 실행 때 적용").
5. **Pipelines = gateway-side filter modules** — 외부 LLM filter 거부 (외부 통신 0 위반), sidecar 거부 (디버깅 부담). Rust trait + ordered chain + per-route activation + audit log.
6. **3종 v1 Pipeline: prompt sanitize / response trim / token quota check** — 한국어-aware 정규화 + 빈 응답 strip + scope.token_budget 도달 시 422 reject. v1.x에서 PII redact / 응답 redaction / observability 확장.
7. **CancellationToken 협력 cancel** — Tauri app exit 시 Poller가 현재 polling 끝나면 즉시 종료. ADR-0023 §Decision 1과 동일한 cooperative cancel 패턴.
8. **dedup invariant: 같은 outdated 버전은 1번만 emit** — 6h마다 같은 알림 토스트가 떠서 사용자 피로 누적되는 것 방지. `Option<String> last_notified` 보유 + up-to-date 복귀 시 클리어.

## §2 기각된 대안 + 사유 (Negative space — 의무 섹션)

### Auto-Updater 영역

#### 2.1 자체 다운로드 + 자동 silent install (in-app patch streaming)

- **시도 / 검토 내용**: GitHub release asset을 백그라운드에서 자동 다운로드 + sha256 검증 + `runtimes/` 자동 swap (Squirrel.Windows in-app updater 패턴).
- **거부 이유**:
  - **사용자 동의 침해** — 사용자가 명시적으로 "받을게요" 누르지 않았는데 디스크 쓰기 + 실행 환경 변경. ADR-0019 always-latest는 *마법사 설치*이지 *백그라운드 자동 적용*이 아님.
  - **EULA 위반 가능성** — LM Studio처럼 EULA 동의가 필요한 외부 런타임에 적용 시 사용자가 EULA 못 읽고 자동 설치되는 모순. ADR-0016 wrap-not-replace는 외부 런타임 lifecycle 존중이 본질.
  - **무결성 검증 시점 분리** — 다운로드 ≠ 적용. silent install은 사용자가 검증할 기회 자체가 없음.
- **재검토 트리거**: 본체 자기 업데이트(LMmaster.exe 본인) 한정으로 "사용자 토글 ON + 마지막 동의" 경로 제공 시 (v1.x).

#### 2.2 패키지 매니저 동기화 (winget / choco / brew / apt)

- **시도 / 검토 내용**: Windows winget / macOS Homebrew / Linux apt 패키지로 LMmaster 배포 + 외부 런타임도 패키지 매니저로 위임.
- **거부 이유**:
  - **포터블 정신 위반** — Phase 3' ADR-0009 portable workspace + USB SSD 운영이 핵심 wedge. 패키지 매니저는 시스템 전역 설치 (per-user portable과 충돌).
  - **권한 부담** — winget/apt는 UAC 또는 sudo 필요. 첫 실행 마법사 4단계 흐름이 깨짐.
  - **글로벌 정책 의존** — 사내 환경에서 winget 차단 등 변수 많음.
- **재검토 트리거**: 사내 IT 표준 배포 채널이 패키지 매니저로 강제될 때 — v1.x에서 별도 `LMmaster-Enterprise` 채널 검토.

#### 2.3 in-app patch streaming (delta update)

- **시도 / 검토 내용**: bsdiff 또는 Courgette delta patch로 50MB 풀 다운로드 대신 1~2MB diff만 전송.
- **거부 이유**:
  - **무결성 검증 어려움** — 부분 적용 도중 실패 시 binary 깨짐 + roll-back 복잡. portable workspace에서 다른 PC로 옮겼을 때 base binary 다르면 patch 적용 실패.
  - **소형 네트워크 절약 vs 복잡도 trade-off** — 6시간 폴이라 데일리 50MB 풀 다운로드도 무리 없음.
  - **검증 인력** — bsdiff format edge case가 너무 많아 v1 scope 확대 위험.
- **재검토 트리거**: 사용자가 satellite/저속망 환경에서 50MB가 부담된다는 dogfood 신호 발생 시 (v1.x).

#### 2.4 자체 호스팅 CDN 우선 (CloudFront / Vercel)

- **시도 / 검토 내용**: GitHub Releases 대신 자체 CDN을 1순위 source로.
- **거부 이유**:
  - **운영 부담** — 인증서 / 도메인 / 비용 지속 발생.
  - **GitHub Releases는 무료 + 신뢰도 높음** — sha256 자동 발행 + GitHub Actions 빌드 trail.
  - **ETag / If-Modified-Since** — registry-fetcher와 같은 패턴으로 충분.
- **재검토 트리거**: GitHub API rate limit 초과가 정상적인 사용 패턴에서 발생할 때 (60 req/h unauthenticated이라 6h 폴 + 단일 사용자에선 여유).

### Pipelines 영역

#### 2.5 외부 LLM filter (Guardrails AI / NeMo Guardrails)

- **시도 / 검토 내용**: Guardrails AI Python lib 또는 NeMo Guardrails로 prompt/response 검증.
- **거부 이유**:
  - **외부 통신 0 정책 (ADR-0013) 위반** — Guardrails AI는 OpenAI moderation API 호출이 default 경로. 한국어 요약은 모두 로컬 LLM only가 우리 약속.
  - **Python 의존성** — Tauri bundle 크기 + 첫 실행 마법사 복잡도 폭증. ML Workbench Python sidecar(ADR-0012)는 *훈련*에 한정 — 매 request마다 IPC는 latency 부담.
  - **한국어 필터 약함** — Guardrails는 영어 전제 패턴 매칭 + 한자 mixed-script 미지원.
- **재검토 트리거**: 한국어 LLM judge 안정성이 검증되고 사용자 명시 토글이 있을 때 (v1.x advanced).

#### 2.6 Gateway 외부 sidecar 프로세스

- **시도 / 검토 내용**: Open WebUI Pipelines처럼 별도 sidecar 프로세스에서 filter 실행 + IPC.
- **거부 이유**:
  - **IPC latency** — 매 request마다 부가 직렬화 + 프로세스 경계 비용. Phase 3' ADR-0022 byte-perfect SSE relay 정신과 충돌.
  - **디버깅 어려움** — 별도 프로세스 stack trace + log 분리 → tracing collation 부담.
  - **lifecycle 복잡도** — sidecar crash 시 supervisor + 재시작 + health check. Tauri 단일 프로세스 모델의 단순함을 잃음.
- **재검토 트리거**: 사용자 정의 Pipeline (Python으로 작성)을 허용하는 marketplace 도입 시 (v2+).

#### 2.7 모델 fine-tune으로 안전성 보장

- **시도 / 검토 내용**: SFT + RLHF로 모델 자체에 PII redaction / 안전성을 학습.
- **거부 이유**:
  - **모델 교체마다 재학습** — 카탈로그 5개 모델 × 분기 신규 출시 → 매번 재학습 + 검증 부담.
  - **deterministic 보장 X** — fine-tune은 확률적 출력. PII가 가끔 새는 케이스를 차단할 수 없음.
  - **carrier 손상** — 한국어 능력 + 도메인 정확도가 안전성 학습으로 떨어질 위험. ADR-0010 Korean-first와 충돌.
- **재검토 트리거**: 우리가 직접 발행하는 LMmaster-curated 모델이 출시될 때 (v2+).

## §3 보강 리서치 인용 (5+ sources)

- **Sparkle (macOS)** — appcast.xml feed + EdDSA 서명 + delta updates. 우리 v1은 단일 platform-agnostic JSON (GitHub release JSON 그대로) + sha256만, 서명은 v1.x.
  <https://sparkle-project.org/>
- **Squirrel (Windows / Electron)** — in-app installer 패턴. 명시적 `Install` step 없이 백그라운드 적용 → *자동 적용* 아닌 "다음 실행 때 적용" UX 패턴 (JetBrains와 동일)을 채택해 Squirrel-style 위험 회피.
  <https://github.com/Squirrel/Squirrel.Windows>
- **Tauri 2 updater plugin** (`tauri-plugin-updater`) — 본체 자기 업데이트는 v1.x 통합 후보. v1 auto-updater는 *외부 런타임 + 카탈로그 + 모델*만 추적, Tauri plugin은 본체용으로 명시 분리.
  <https://v2.tauri.app/plugin/updater/>
- **`semver` crate (rust-lang/semver)** — `Version::parse` + `Ord` 표준 비교. pre-release 우선순위 + build metadata 무시 spec 준수.
  <https://docs.rs/semver/latest/semver/>
- **OWASP filter chain pattern** — input validation → authentication → authorization → encoding → logging. Pipelines가 gateway 진입에 동일 chain을 적용 (sanitize → quota → audit).
  <https://cheatsheetseries.owasp.org/cheatsheets/Input_Validation_Cheat_Sheet.html>
- **Llama Guard (Meta)** — LLM-based safety classifier. v1 거부 (외부 통신 0 위반 잠재), v2+에서 로컬 LLM judge로 검토.
  <https://github.com/facebookresearch/PurpleLlama>
- **NeMo Guardrails (NVIDIA)** — Colang DSL based dialog safety. v1 거부 (Python 의존성), v1.x advanced 토글 후보.
  <https://github.com/NVIDIA/NeMo-Guardrails>
- **Open WebUI Pipelines** — UI plugin marketplace. 우리는 *gateway*에 적용 — 차별화 thesis #7과 일치.
  <https://github.com/open-webui/pipelines>
- **JetBrains Toolbox 3.3 update flow** — "Available · Update · Apply on next launch" 3-state. 우리 v1 토스트가 정확히 같은 패턴.
  <https://www.jetbrains.com/toolbox-app/>

## §4 구현 contract (verbatim Rust)

### 4.1 `UpdateSource` trait + `ReleaseInfo`

```rust
#[async_trait]
pub trait UpdateSource: Send + Sync {
    async fn latest_version(&self) -> Result<ReleaseInfo, UpdaterError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub version: String,                    // "1.2.3" or "v1.2.3"
    #[serde(with = "time::serde::rfc3339")]
    pub published_at: time::OffsetDateTime,
    pub url: String,
    pub notes: Option<String>,
}
```

### 4.2 `Poller` 구조

```rust
pub struct Poller {
    source: Arc<dyn UpdateSource>,
    current_version: String,
    interval: Duration,                     // default = 6h
    last_notified: Arc<Mutex<Option<String>>>,  // dedup
}

impl Poller {
    pub async fn check_once(&self) -> Result<Option<ReleaseInfo>, UpdaterError>;
    pub async fn run<F>(&self, on_update: F, cancel: CancellationToken)
    where F: Fn(ReleaseInfo) + Send + Sync;
}
```

### 4.3 `Pipeline` trait (Phase 6'.b 구현 예정 — 본 결정 노트는 contract만 확정)

```rust
#[async_trait]
pub trait Pipeline: Send + Sync {
    /// 식별자 (audit log + per-route activation 키).
    fn id(&self) -> &str;

    /// request 진입 시점에 호출. body 변경/거부 가능.
    async fn pre_request(&self, ctx: &mut PipelineContext, body: &mut serde_json::Value)
        -> Result<(), PipelineError>;

    /// response (full or chunk) 송출 직전에 호출. body 변경/거부 가능.
    async fn post_response(&self, ctx: &mut PipelineContext, body: &mut serde_json::Value)
        -> Result<(), PipelineError>;
}

pub struct PipelineContext {
    pub request_id: String,
    pub api_key_id: String,
    pub route: String,
    pub model: String,
    /// 누적 토큰 (token quota check Pipeline이 mutate).
    pub tokens_consumed: u64,
}

pub struct PipelineChain {
    pub pipelines: Vec<Arc<dyn Pipeline>>,
}
```

### 4.4 v1 3종 Pipeline 시드

- **PromptSanitize** — 한국어 NFC 정규화 + 공백 normalize + 제어 문자 strip.
- **ResponseTrim** — leading/trailing whitespace strip + 빈 응답이면 422 reject.
- **TokenQuotaCheck** — `scope.token_budget` 초과 시 즉시 reject (Phase 3' ADR-0022 §5 scope 확장).

## §5 검증 시나리오 (테스트 invariant)

> 본 sub-phase가 깨면 안 되는 동작들. 다음 세션 리팩토링 시 우연히 깨도 빨간불.

### Auto-Updater

- **6h poll cancel** — 6h sleep 도중 `cancel.cancel()` 호출 시 1초 안에 polling 루프 종료. `tokio::select!`로 sleep + cancelled 동시 listen.
- **outdated detection** — `is_outdated("1.0.0", "1.0.1") == Ok(true)`, `is_outdated("1.0.1", "1.0.0") == Ok(false)`, equal version → false.
- **pre-release ordering** — `1.0.0-beta < 1.0.0`, `1.0.0-alpha < 1.0.0-beta`.
- **build metadata 무시** — `1.0.0+build1 == 1.0.0+build2`.
- **leading 'v' strip** — `is_outdated("v1.0.0", "v1.0.1") == Ok(true)` (GitHub tag 관행).
- **invalid version error** — 잘못된 semver는 `UpdaterError::InvalidVersion(원본)` 반환 (deterministic).
- **source failure log + continue** — `latest_version()` Err 발생 시 polling 루프 멈추지 않음. `tracing::warn!`로 로그 + 다음 cycle.
- **dedup** — 같은 outdated 버전은 polling 횟수와 관계없이 정확히 1번 emit. up-to-date 복귀 시 last_notified 클리어.
- **NoReleases vs SourceFailure** — GitHub 404는 `NoReleases`, 그 외 4xx/5xx는 `SourceFailure(HTTP {status})`.
- **MockSource set_release runtime mutate** — 같은 인스턴스에서 release를 갈아끼울 수 있어야 Poller dedup 테스트 가능.

### Pipelines (Phase 6'.b 검증 invariant — 본 결정 노트는 contract만)

- **order preservation** — `PipelineChain::pre_request`는 `pipelines[0]` → `pipelines[1]` 순. `post_response`는 역순 (LIFO — middleware standard).
- **early termination** — `pre_request`가 `Err` 반환 시 chain 중단 + upstream 호출 안 함.
- **idempotency** — 같은 ctx + body로 두 번 호출해도 동일 결과 (PromptSanitize NFC × 2 = NFC × 1).
- **per-route activation** — `/v1/chat/completions`에는 적용, `/health`에는 적용 안 함.
- **audit log** — 매 Pipeline 실행은 `tracing::info!` 1줄 (request_id + pipeline_id + duration).

## §6 미해결 / 후순위 이월

| 항목 | 이월 사유 | 진입 조건 / 페이즈 |
|---|---|---|
| 자동 다운로드 동의 UI (Phase 6'.c) | Tauri IPC + React 토스트 컴포넌트 + 사용자 prefs persist 설계 필요. | Phase 6'.c |
| Pipeline 사용자 정의 UI (Settings → Pipelines) | per-key activation + drag-and-drop 순서 + 토글. | v1.x |
| signing key rotation | EdDSA 서명 + key rotation은 self-distribution 채널 마련 후. | v1.x |
| Tauri updater plugin 통합 | 본체 자기 업데이트만 별도 채널 — `tauri-plugin-updater`와 본 crate 책임 분리. | v1.x |
| delta update (bsdiff) | dogfood 후 50MB 부담 신호 발생 시. | v2+ |
| Pipeline marketplace (Python 사용자 정의) | sidecar 프로세스 안정성 검증 + 검열 정책. | v2+ |
| Llama Guard 로컬 LLM judge | 한국어 안정성 데이터 충분히 확보 후. | v1.x advanced 토글 |
| ETag / If-Modified-Since cache | Phase 1' registry-fetcher 패턴 재사용 — auto-updater에도 적용. | Phase 6'.b |
| GitHub API rate limit (60 req/h unauth) | 6h × 1 user = 4 req/day라 v1 여유. 다중 source 추가 시 검토. | v1.x |
| 본체 로그 redact (PII Pipeline) | 사용자 채팅 본문이 tracing log에 들어가지 않도록 별도 redact filter. | v1.x |

## §7 다음 페이즈 인계 (Phase 6'.b/.c 진입 조건)

- **선행 의존성**:
  - Phase 3' ADR-0022 gateway scope (Pipeline은 scope.token_budget을 읽음).
  - Phase 1' registry-fetcher 패턴 (ETag / cache 4-tier — auto-updater에도 적용).
  - Phase 2'.c bench-harness `BenchErrorReport` (Pipeline 실패 시 동일 한국어 결로 사용자 노출).
- **이 페이즈 산출물**:
  - `crates/auto-updater/` (lib + error + version + source + poller + integration tests).
  - `docs/adr/0025-pipelines-architecture.md`.
  - `docs/adr/0026-auto-updater-source.md`.
  - 본 결정 노트.
- **다음 sub-phase 진입 조건**:
  - Phase 6'.b — `Pipeline` trait 구체 구현 + 3종 v1 시드 + gateway 라우터 통합.
  - Phase 6'.c — Tauri IPC `check_for_update` / `start_update_poller` / `cancel_update_poller` + React 토스트 + Settings → 자동 갱신 토글.
- **위험 노트** (next session 함정):
  - **auto-updater와 본체 자기 업데이트의 책임 분리** — 본 crate는 *외부 런타임 + 카탈로그 + 모델* 추적. 본체 LMmaster.exe 본인은 `tauri-plugin-updater` 별도 채널. 다음 세션이 두 책임을 합치려 들면 silent install 위험 재발.
  - **dedup invariant 깨지기 쉬움** — `last_notified`를 up-to-date 복귀 시 클리어해야 다음 outdated가 다시 알림. 누락 시 사용자가 1.0.0 → 1.1.0 → 1.0.0(rollback) → 1.1.0 시 두 번째 1.1.0 알림 못 받음.
  - **6h 기본값 vs Settings UI 토글** — Phase 6'.c에서 `Poller::with_interval`이 사용자 prefs를 읽어야 함. 기본값은 6h이지만 Settings 토글이 1h~24h 범위 강제 검증 필요 (음수/0 거부).
  - **Pipeline order vs SSE relay 충돌** — byte-perfect SSE relay (ADR-0022 §2)는 chunk 단위 transformation 어려움. v1 Pipeline은 *full response*에만 적용, *streaming chunk*는 v1.x.
  - **pre-release 버전 흐름** — v1.0.0-rc.1 → v1.0.0 시 사용자에겐 outdated. 하지만 v1.0.0 → v1.0.0-beta는 outdated가 아님 (downgrade). is_outdated 비교 정확성 매번 의식.
  - **GitHub API auth** — unauthenticated rate limit 60 req/h. 6h 폴 × 1 user = 4 req/day여서 OK이지만, source 다양화 (다중 repo) 시 PAT 인증 필요. 사용자에게 "GitHub PAT 입력해 주세요"는 UX 부담 — 자체 호스팅 mirror 검토.

## 참고

- 결정 노트: 본 문서.
- ADR-0025 (Pipelines architecture).
- ADR-0026 (Auto-Updater source + 6h poll).
- ADR-0024 (Knowledge Stack RAG — Phase 4.5'.a 동시 진행).
- 메모리: `tech_stack_defaults` (외부 통신 0 + Korean-first), `competitive_thesis` (Thesis #7 — Pipelines를 gateway에 적용).
- Phase 3' decision: `docs/research/phase-3p-gateway-decision.md` (scope.token_budget 정의).
- Phase 1' decision: `docs/research/phase-1p-registry-fetcher-decision.md` (ETag / TTL / 4-tier).

---

**문서 버전**: v1.0 (2026-04-28). Phase 6'.a scaffold 단계 — auto-updater crate 구체화 + Pipelines contract 확정. Phase 6'.b/.c에서 본격 구현.

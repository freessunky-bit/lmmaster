# ADR-0026: Auto-Updater — GitHub Releases 1순위 source + 6h poll

- Status: Accepted
- Date: 2026-04-28
- Related: ADR-0013 (외부 통신 0 — 단, GitHub api.github.com만 예외 허용), ADR-0016 (wrap-not-replace — silent install 금지), ADR-0019 (always-latest hybrid bootstrap), ADR-0022 (gateway routing — `tower-http::request-id`와 동일한 tracing 결)
- 결정 노트: `docs/research/phase-6p-updater-pipelines-decision.md`

## Context

Phase 6'.a 진입 — 사용자가 매번 LM Studio / Ollama / 카탈로그 모델 업데이트를 수동 확인하는 부담을 줄이고, 보안 패치를 빠르게 전파해야 한다. 다음 5 영역이 동시에 결정 필요:

1. **소스 1순위** — vendor API / GitHub Releases / 자체 CDN 어디?
2. **폴 간격** — 1h / 6h / 24h?
3. **다운로드 trigger** — 자동 / 사용자 동의 후?
4. **silent install 허용** — 본체? 외부 런타임?
5. **버전 비교** — semver / lexicographic / 자체 포맷?

기존 `crates/registry-fetcher` (Phase 1')는 *manifest 카탈로그*를 추적하지만, *바이너리 릴리스*는 별도 채널이 필요. 본 ADR이 그 채널을 정의한다.

## Decision

### 1. `UpdateSource` trait + `GitHubReleasesSource` 1순위

```rust
#[async_trait]
pub trait UpdateSource: Send + Sync {
    async fn latest_version(&self) -> Result<ReleaseInfo, UpdaterError>;
}

pub struct GitHubReleasesSource {
    repo: String,            // "owner/repo"
    client: reqwest::Client,
    base_url: String,        // default "https://api.github.com"
    timeout: Duration,       // default 8s
}
```

GitHub Releases `https://api.github.com/repos/{repo}/releases/latest` 단일 엔드포인트만 v1에 사용. ETag / If-Modified-Since는 v1.x (Phase 6'.b).

### 2. 6h 기본 폴 간격 (1h~24h 사용자 설정)

`DEFAULT_INTERVAL = Duration::from_secs(6 * 60 * 60)`. JetBrains Toolbox / VS Code와 일치. `Poller::with_interval(...)`로 사용자 설정 주입 — Phase 6'.c Settings UI에서 1h~24h 토글.

```rust
pub struct Poller {
    source: Arc<dyn UpdateSource>,
    current_version: String,
    interval: Duration,
    last_notified: Arc<Mutex<Option<String>>>,  // dedup
}
```

### 3. 사용자 동의 후만 다운로드. silent install 절대 금지

- 폴러는 *outdated 감지 + 토스트 emit*만. 다운로드/설치는 사용자가 명시적으로 "받을게요" 클릭한 후만.
- 본체(LMmaster) 본인 업데이트도 동일. JetBrains-style "다음 실행 때 적용" 패턴.
- LM Studio 같은 EULA 필요 외부 런타임은 *반드시 EULA 확인 후*만. ADR-0016 wrap-not-replace 정책 준수.
- Ollama (MIT) 만 silent install 허용 — 기존 Phase 1A.3 정책과 동일.

### 4. semver 비교 — pre-release/build metadata spec 준수 + leading 'v' strip

```rust
pub fn is_outdated(current: &str, latest: &str) -> Result<bool, UpdaterError>
```

- `semver::Version::parse` + `Ord` 비교.
- pre-release: `1.0.0-beta < 1.0.0`.
- build metadata: `1.0.0+build1 == 1.0.0+build2` (ordering 무시).
- leading 'v'/'V' 한 글자 strip (GitHub release tag 관행 `v1.2.3`).
- 둘 중 하나라도 파싱 실패 → `UpdaterError::InvalidVersion(원본)` (deterministic, fallback X).

### 5. dedup invariant — 같은 outdated 버전 1번만 emit

`Poller`는 `last_notified: Arc<Mutex<Option<String>>>`을 보유. 같은 outdated 버전 polling 시 dedup. up-to-date 복귀 시 클리어 → 다음에 outdated 다시 등장 시 재알림.

### 6. cancellation — `tokio_util::sync::CancellationToken` 협력 cancel

- 매 cycle 시작 시 `cancel.is_cancelled()` 체크.
- sleep 중에는 `tokio::select! { sleep / cancel.cancelled() }`로 동시 listen → cancel 즉시 깨어남.
- Tauri app exit 시 `gateway://shutdown`과 동일 결로 polling 종료.

### 7. error 매핑 — 6 variant thiserror enum

```rust
pub enum UpdaterError {
    Network(reqwest::Error),       // "업데이트 서버에 연결하지 못했어요"
    Parse(String),                 // "릴리스 정보를 해석하지 못했어요"
    InvalidVersion(String),        // "버전 형식이 올바르지 않아요"
    NoReleases,                    // "릴리스가 아직 없어요"  (GitHub 404)
    Cancelled,                     // "사용자가 업데이트 확인을 취소했어요"
    SourceFailure(String),         // "업데이트 소스가 응답하지 않아요"
}
```

- 사용자 향 노출은 `Display`만으로 충분 (한국어 해요체).
- 영어 fallback은 토스트 컴포넌트 i18n 키에서 처리 — error enum 본체는 한국어 1차.

### 8. test 표면 — `MockSource` + `Poller` 단위 + 통합 테스트 30+

- `MockSource::set_release(release)` 런타임 mutate로 Poller dedup / cancel 시나리오 검증.
- `tokio::test(start_paused = true)` + `time::advance`로 6h 폴 시뮬레이션 (실제 6h 안 기다림).

## Consequences

**긍정**:
- **사용자 동의 보존** — silent install 절대 없음 → ADR-0016 wrap-not-replace + LM Studio EULA 준수.
- **dedup으로 토스트 피로 0** — 6h마다 같은 알림 안 뜸. 사용자가 "다음에 할게요" 한 번 누르면 다음 outdated 버전까지 침묵.
- **GitHub 무료 + 신뢰도** — 자체 CDN 운영 부담 없음. sha256 자동 발행.
- **소스 추상화** — `UpdateSource` trait로 v1.x에서 자체 CDN / mirror 추가 여지.
- **deterministic semver** — invalid version 거부 (lenient parsing 안 함) → 카탈로그 일관성 보존.

**부정**:
- **GitHub API rate limit** — unauthenticated 60 req/h. 6h × 1 user = 4 req/day OK. 다중 source / 다중 repo 시 PAT 인증 부담 (v1.x).
- **본체 자기 업데이트 분리** — 본 crate는 *외부 런타임 + 카탈로그 + 모델*만. 본체 LMmaster.exe는 `tauri-plugin-updater` 별도 채널 (v1.x). 책임 경계 명확하지만 다음 세션이 합치려 들 위험.
- **streaming download progress 미지원** — v1은 metadata 추적만. 다운로드는 기존 `crates/installer` (Phase 1A.3 resumable downloader)가 담당. 통합은 Phase 6'.c.

**감내한 트레이드오프**:
- 6h 기본값 vs 즉시 알림 — 즉시 알림은 매 1분 폴 같은 부담. 6h가 보안 패치 전파 + 사용자 부담 균형.
- pre-release 우선순위 — `v1.0.0-rc.1 < v1.0.0` 룰을 따르므로 사용자가 rc 받았다가 정식 release 나오면 outdated 인식. 의도된 동작.
- `last_notified`는 메모리에만 — 앱 재시작 시 리셋. 사용자가 같은 toast를 한 번 더 볼 수 있음. v1.x에서 prefs persist.

## Alternatives considered (negative space — 결정 노트 §2.1–§2.4 미러)

### a. 자체 다운로드 + 자동 silent install (in-app patch streaming)

거부. 사용자 동의 침해 + EULA 위반 가능성 + 무결성 검증 시점 분리 어려움. ADR-0019 always-latest는 마법사 설치이지 백그라운드 자동 적용이 아님.

### b. 패키지 매니저 동기화 (winget / choco / brew / apt)

거부. portable workspace 정신 위반 + 권한 부담 (UAC/sudo) + 사내 환경 winget 차단 변수.

### c. in-app patch streaming (delta update — bsdiff / Courgette)

거부. 무결성 검증 어려움 + 부분 적용 도중 실패 시 binary 깨짐 + portable workspace에서 base binary 다르면 patch 적용 실패. 6h × 일일 50MB 풀 다운로드는 무리 없음.

### d. 자체 호스팅 CDN 1순위 (CloudFront / Vercel)

거부. 운영 부담 (인증서 / 도메인 / 비용) + GitHub Releases는 무료 + 신뢰도 높음. v1은 GitHub 1순위 + v1.x에서 자체 CDN을 fallback 추가 검토.

### e. 1h 폴 (즉시성 우선)

거부. GitHub API rate limit 부담 + 보안 패치 평균 30분~2시간 단위로 발행되지 않음 (대부분 24h 이상). 6h가 균형.

### f. 24h 폴 (네트워크 절약)

거부. 보안 패치 전파 너무 늦음. 6h가 보안 + 부담 균형. 사용자가 24h 선호하면 Settings에서 토글.

### g. lexicographic 비교 (semver 거부)

거부. `1.0.10 < 1.0.2` (lex) 같은 정렬 오류. semver는 표준이며 GitHub release tag도 대부분 따름. invalid version은 명시 거부 (lenient X).

### h. Tauri updater plugin만 사용 (자체 crate 거부)

거부. `tauri-plugin-updater`는 *본체 자기 업데이트*만. 외부 런타임 + 카탈로그 + 모델 추적은 별도 추상화 필요. 본 crate가 그 역할 + 본체 업데이트는 별도 채널.

## 검증 invariant

- **6h poll cancel** — 6h sleep 도중 cancel 시 1초 안에 polling 루프 종료.
- **outdated detection** — `is_outdated("1.0.0", "1.0.1") == Ok(true)`, equal version → false.
- **pre-release** — `1.0.0-beta < 1.0.0`, alpha < beta < rc.
- **build metadata** — `1.0.0+build1 == 1.0.0+build2`.
- **leading 'v' strip** — `is_outdated("v1.0.0", "v1.0.1") == Ok(true)`.
- **invalid version error** — 잘못된 semver는 `UpdaterError::InvalidVersion(원본)`.
- **source failure log + continue** — `latest_version()` Err 발생 시 polling 루프 멈추지 않음.
- **dedup** — 같은 outdated 버전 polling 횟수와 관계없이 1번 emit.
- **NoReleases** — GitHub 404 시 `UpdaterError::NoReleases` (다른 4xx/5xx는 `SourceFailure`).
- **MockSource set_release runtime mutate** — Poller dedup / re-emit 테스트 가능.
- **error 메시지 한국어** — 모든 6 variant `Display`가 해요체 ("...했어요", "...요").

## References

- 결정 노트: `docs/research/phase-6p-updater-pipelines-decision.md`
- ADR-0013 (외부 통신 0 — GitHub api.github.com 단일 호스트만 예외)
- ADR-0016 (wrap-not-replace — silent install 정책)
- ADR-0019 (always-latest hybrid bootstrap — *마법사 설치*이지 자동 적용 아님)
- Sparkle (macOS): <https://sparkle-project.org/>
- Squirrel.Windows: <https://github.com/Squirrel/Squirrel.Windows>
- Tauri 2 updater: <https://v2.tauri.app/plugin/updater/>
- semver crate: <https://docs.rs/semver/latest/semver/>
- JetBrains Toolbox 3.3: <https://www.jetbrains.com/toolbox-app/>

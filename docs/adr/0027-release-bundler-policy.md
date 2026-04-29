# ADR-0027: Release bundler / sign / EULA / telemetry policy

- Status: Accepted
- Date: 2026-04-28
- Related: ADR-0009 (portable workspace), ADR-0010 (Korean-first), ADR-0013 (외부 통신 0), ADR-0016 (wrap-not-replace), ADR-0026 (auto-updater 외부 런타임)
- 결정 노트: `docs/research/phase-7p-release-prep-reinforcement.md`

## Context

Phase 7' 진입 — v1 launch 직전. 다음 5가지 정책을 한 번에 결정해야 해요:

1. **Bundler 매트릭스** — Win / mac / Linux 어느 형식으로 배포?
2. **코드 사인** — Authenticode (Win), Apple Developer ID (mac) 비용·시점·평판 빌드 전략.
3. **EULA 표시 흐름** — 첫 실행 게이트, 버전 갱신 시 재동의 정책.
4. **텔레메트리** — opt-in 정책, endpoint 도구, 데이터 범위.
5. **자동 업데이트 책임 분리** — 본체 self-update vs ADR-0026 외부 런타임 추적.

각 결정의 *왜 다른 안을 거부했는지* (negative space)를 ADR에 박아 둬서 다음 세션이 같은 함정에 빠지지 않게 해요.

## Decision

### 1. Bundler 매트릭스 — NSIS perUser / dmg / AppImage

- **Windows**: NSIS, `installMode: "perUser"`, languages `["Korean", "English"]`. UAC 우회 + ADR-0009 portable workspace 정신 일관.
- **macOS**: dmg, `minimumSystemVersion: "11.0"` (Big Sur). entitlements.plist는 `network.client + network.server + disable-library-validation`. JIT는 거부(False)로 두고 llama.cpp는 외부 sidecar로 처리.
- **Linux**: AppImage portable single-file. deb/rpm은 v1.x.

### 2. 코드 사인 정책

- **Windows OV ($150~$300/년)**: SmartScreen 평판 빌드 ~6개월. EV는 2024년 변경으로 즉시 우회 X — 비용 5배 정당화 안 됨.
- **mac Developer ID Application + notarytool ($99/년)**: altool은 deprecated. Hardened Runtime + entitlements.plist + stapler staple 워크플로.
- **현재 상태 (Phase 7'.a)**: 두 인증서 모두 미발급. config는 placeholder TODO + `certificateThumbprint: null` / `signingIdentity: null`로 unsigned ship 가능. 사용자 향 SmartScreen / Gatekeeper 경고는 한국어 README + 다운로드 페이지에서 안내.

### 3. EULA — 첫 실행 클릭랩 + version-bound localStorage

- 컴포넌트: `apps/desktop/src/components/EulaGate.tsx`. localStorage 키 `lmmaster.eula.accepted.<version>`.
- 다크 패턴 회피: 사용자가 본문을 끝까지 스크롤해야 "동의할게요" 버튼 활성.
- 한국어 / English 토글. i18n.resolvedLanguage default.
- 거절 → 확인 dialog → window.close(). LM Studio EULA는 *우리가 다시 표시 X* (ADR-0016 wrap-not-replace).
- 갱신 정책: patch (1.0.0 → 1.0.1) 자동 동의, minor/major (1.1.0 / 2.0.0) 재동의.

### 4. 텔레메트리 — opt-in only + GlitchTip self-hosted (v1.x 연결)

- 기본 비활성. Settings → "익명 사용 통계" 토글로 opt-in.
- 첫 opt-in 시 backend가 anonymous UUID v4 + opted_in_at(RFC3339) 발급. PC 단위 식별자, 개인 식별 X.
- Phase 7'.a 산출물: `apps/desktop/src-tauri/src/telemetry.rs` config + UUID 관리 + 디스크 영속만. 실제 전송 endpoint 연결은 Phase 7'.b.
- Endpoint: GlitchTip self-hosted (~$10/월 VPS). Sentry SaaS는 거부 — ADR-0013 외부 통신 0 위반 + 사용자 신뢰 해칠 위험.

### 5. 자동 업데이트 책임 분리

- **본체 self-update**: `tauri-plugin-updater` v2. endpoint = `releases/latest/download/latest.json` GitHub Releases CDN. minisign signature 강제 검증.
- **외부 런타임 (Ollama / LM Studio / 카탈로그)**: ADR-0026 `crates/auto-updater`. endpoint = `api.github.com/repos/.../releases/latest`. 두 system은 *합치려는 충동을 거부* — schema가 달라요.
- 사용자 향 toast 컴포넌트는 통일 (`UpdateToast`), label만 분기.

## Consequences

### Positive

- v1 launch에 필요한 packaging / EULA / telemetry / updater scaffold가 모두 갖춰짐.
- 인증서 발급은 별도 경로(사용자 결정)로 분리 — 코드 변경 없이 thumbprint / identity 채우기만 하면 됨.
- 텔레메트리 opt-in / version-bound EULA / self-update minisign — 사용자 신뢰 빌드의 3 기둥이 코드에 박힘.
- 책임 분리(본체 vs 외부 런타임 updater) → 다음 세션이 endpoint를 잘못 합칠 위험 차단.

### Negative

- 인증서 미발급 상태에선 사용자 향 SmartScreen / Gatekeeper 경고 발생 → 한국 일반 사용자 이탈 위험. Phase 7'.b에서 OV / Developer ID 발급으로 해결 예정.
- minisign 비밀키 분실 / 노출 시 사용자 강제 재설치 필요 (disaster) — 키는 1Password / KeyHub vault 의무.
- GlitchTip self-hosted는 ~$10/월 VPS + 서버 SOC 부담. v1.x로 미룸.
- EULA 본문은 placeholder + TODO 마커. 법무 검토 후 채워야 v1 launch 가능.

## Alternatives considered

### A. WiX MSI vs NSIS (Win)

**거부 이유**: MSI는 enterprise GPO 배포 채널용. v1 타깃 (개인 사용자) 부합 X. VBSCRIPT 의존 + Win11 LTSC disable 가능 → 빌드 환경 부담. NSIS perUser가 portable 정신과 한국어 lang 지원 모두 만족.

### B. Sentry SaaS vs GlitchTip self-hosted

**거부 이유 (Sentry SaaS)**: ADR-0013 외부 통신 0 원칙 직접 위반. 첫 외부 의존 + 사용자 데이터가 3rd party에 도달 → 사용자 신뢰 해침. 무료 5K/월 quota는 매력적이지만 LMmaster의 privacy thesis와 정면 충돌.

**채택 (GlitchTip self-hosted)**: Sentry SDK 호환 + 모든 데이터 우리 서버만. 비용은 ~$10/월 VPS + 서버 운영 부담 trade-off.

### C. EULA always-show vs version-bound localStorage

**거부 이유 (always-show)**: 매 실행마다 EULA dialog → 사용자 피로 + "다 동의함" 클릭 습관 형성 → 동의 의미 잃음. 매 패치 갱신마다 재동의 → UX 부담.

**채택 (version-bound)**: 한 번 동의 = 같은 EULA 버전엔 다시 묻지 않음. patch 자동 동의 / minor·major 재동의. 사용자 피로 최소화 + 동의 명시성 보존.

### D. EV 인증서 vs OV 인증서 (Win)

**거부 이유 (EV)**: 2024년 Microsoft 변경으로 EV의 "즉시 SmartScreen 우회" 특혜 제거. OV와 평판 빌드 시간 차이 ~3개월 vs 6개월 — 비용 5배 정당화 X. v1.x에서 다운로드 50K+ 시점에 재검토.

### E. Mac App Store 제출

**거부 이유**: sandbox + Phase 5' python sidecar 충돌 (ADR-0012). receipt 등록 = system-wide install = portable workspace 정신 위반. v1.x에서 별도 ADR로 평가.

## References

- 결정 노트: `docs/research/phase-7p-release-prep-reinforcement.md` (492 LOC, 24 인용).
- Tauri 2 Windows installer: <https://v2.tauri.app/distribute/windows-installer/>
- Tauri 2 macOS code signing: <https://v2.tauri.app/distribute/sign/macos/>
- Tauri 2 updater plugin: <https://v2.tauri.app/plugin/updater/>
- 2024 SmartScreen 변경: <https://learn.microsoft.com/en-us/answers/questions/417016/reputation-with-ov-certificates-and-are-ev-certifi>
- GlitchTip self-hosted: <https://glitchtip.com/>
- ADR-0013 (외부 통신 0), ADR-0016 (wrap-not-replace), ADR-0026 (auto-updater 외부 런타임).

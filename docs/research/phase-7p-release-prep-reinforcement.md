# Phase 7' — v1 Release Prep 보강 리서치

> 작성일: 2026-04-28
> 상태: 보강 리서치 (구현 직전 — Phase 5'.e 완료 후 본 페이즈 진입)
> 선행: Phase 5'.e (HTTP 런타임 + Modelfile shell-out), Phase 6' (auto-updater + pipelines), 모든 Phase 1A~5'/4p5/6'.
> 후행: 첫 v1 공개 release (GitHub Releases). v1.x 패치 채널.
>
> 본 노트 범위:
> - Tauri 2 bundler 매트릭스 (Win NSIS / mac dmg / Linux AppImage).
> - 코드 서명 (Windows Authenticode, mac notarization).
> - `tauri-plugin-updater` 본체 자기 업데이트 vs ADR-0026 외부 런타임 업데이트 책임 분리.
> - 한국어 1차 EULA 노출 흐름.
> - 베타 채널 + 텔레메트리 opt-in (ADR-0013 외부 통신 0 일관).
> - OS-specific 설치 / 제거.

---

## §0. 결정 요약 (10가지)

1. **Win = NSIS perUser, mac = dmg + Developer ID, Linux = AppImage** — Tauri 2 권장 매트릭스. NSIS는 한국어 다국어 + UAC 회피(perUser)가 portable workspace 정신과 부합. mac dmg는 표준 배포 + Gatekeeper friendly. Linux는 AppImage portable 정신 일관 (deb/rpm은 v1.x).
2. **Windows OV 인증서 + 6개월 평판 빌드 = SmartScreen 1차 우회** — 2024년 변경으로 EV도 즉시 우회 X. OV($150~$300/년)이 비용/효용 균형. EV는 v1.x 후순위 (사용자 다운로드 50K+ 시점에 재검토).
3. **mac Developer ID Application ($99/년) + notarytool + dmg notarize** — altool은 2023-11 deprecated. notarytool 1순위. App Store 제출 X (sandbox + sidecar 충돌, ADR-0012 python sidecar).
4. **`tauri-plugin-updater` v2 = 본체 자기 업데이트 단일 책임** — endpoint = `latest.json` GitHub Releases CDN. minisign keypair (Tauri CLI 생성) + `pubkey` `tauri.conf.json`에 embedding. installMode: Windows `passive`(default), mac auto-restart, Linux AppImage in-place.
5. **ADR-0026 `crates/auto-updater` = 외부 런타임 + 카탈로그 + 모델 추적** — endpoint 분리: api.github.com/repos/ollama/ollama/releases/latest 등. tauri-plugin-updater는 *우리 본체*만, ADR-0026 crate는 *외부*만. 합치려는 충동 차단 (§7.4 negative space).
6. **첫 실행 EULA dialog = 한국어 1차 + 영어 fallback + clickwrap** — checkbox "동의해요" + "다음" 버튼 (해요체). 동의 안 한 사용자는 본 화면 진입 차단. 동의 영속 → workspace settings `eula.accepted_version: "1.0.0"`. EULA 버전 갱신 시 재동의 (semver minor 이상).
7. **베타 채널 = Settings 토글 ("베타 채널 참여할게요") + 별도 latest.json endpoint** — 기본 OFF. ON 시 endpoint url에 `/beta/` segment 삽입 → 별도 GitHub Release pre-release 채널. 정식 → 베타 한 방향 (베타 사용자는 정식 release 보일 때 별도 안내).
8. **텔레메트리 = opt-in only + 본체 crash report만 v1** — ADR-0013 외부 통신 0 + 사용자 명시 동의. 익명 사용 통계 / 모델 호출 통계 v1 X (privacy 우선). crash report는 *opt-in* + 사용자 향 "익명 크래시 보고서를 보낼게요" 토글, 기본 OFF.
9. **다국어 README = 한국어 1차 + 영어 동시 + ja/zh v1.x** — README.ko.md / README.md(en) 두 개. 외부 사용자 첫 마주침은 영어, 한국어 사용자는 link로 2클릭. 일본어 / 중국어는 v1.x 사용자 요청 시.
10. **GitHub Releases v1 1순위 + mac App Store + Microsoft Store v1.x** — Store 채널은 sandbox / 정책 / 심사 부담 → v1 X. v1.x에서 별도 ADR로 평가.

---

## §1. Tauri 2 bundler 엘리트 사례

### 1.1 Windows NSIS 채택 + MSI 거부

**채택 (`tauri.conf.json` 발췌)**: `bundle.targets: ["nsis", "dmg", "appimage"]`, `bundle.createUpdaterArtifacts: true`, `windows.nsis: { installerIcon, installMode: "perUser", languages: ["Korean", "English"], displayLanguageSelector: false, compressionLevel: 0 }`.

**근거 6가지**:
1. NSIS = setup.exe (한국 사용자 친숙, 게임 패턴), MSI는 enterprise GPO 배포용 v1 타깃 X.
2. **perUser 모드 = UAC 우회** — `%LOCALAPPDATA%\LMmaster\` 설치, 관리자 권한 X. ADR-0009 portable workspace 정신 일관.
3. NSIS는 다국어 단일 binary. `languages: ["Korean", "English"]`로 두 언어 OS 자동 감지.
4. NSIS hooks (`NSIS_HOOK_PREINSTALL` / `POSTINSTALL` 등 4개) v1.x 확장 여지.
5. MSI는 VBSCRIPT 의존 (Windows 11 LTSC에서 disable 가능 → 빌드 환경 부담).
6. `compressionLevel: 0` = LZMA 최강. 본체 ~30MB → 5~10MB.

**기각**: WiX MSI v1 거부. v1.x enterprise 채널 후순위 (별도 ADR).

### 1.2 macOS dmg 채택 + pkg 거부

**채택 (`macOS` 섹션)**: `minimumSystemVersion: "11.0"` (Big Sur, Apple Silicon 지원), `entitlements: "entitlements.plist"` (§1.4), `signingIdentity: "Developer ID Application: ..."`, `dmg: { background, windowSize, appPosition, applicationFolderPosition }`.

**근거**: dmg = mac 표준 (Homebrew Cask도 dmg unpack), Big Sur 5년 지원. **기각**: pkg는 Mac App Store 제출 시 필수지만 v1은 GitHub Releases만 — pkg system-wide install + receipt 등록은 portable workspace 정신 위반.

### 1.3 Linux AppImage 채택 + deb/rpm 거부

**채택**: `linux.appimage: { bundleMediaFramework: false }`, deb/rpm null.

**근거**: AppImage = single-file portable, 권한 X, 어떤 distro든 동작. `bundleMediaFramework: false` = GStreamer 미포함 (LMmaster는 LLM only). **기각**: deb/rpm은 distro별 빌드 + apt/yum repo 운영 부담 → v1.x 사용자 요청 시.

### 1.4 entitlements.plist (mac)

`com.apple.security.cs.allow-unsigned-executable-memory` (llama.cpp JIT — notarization 거절 회피), `com.apple.security.cs.disable-library-validation` (Phase 5' python sidecar 로드, ADR-0012), `com.apple.security.network.client` + `network.server` (localhost HTTP + gateway listen, ADR-0006), `com.apple.security.files.user-selected.read-write` (workspace import).

### 1.5 production Tauri 2 앱 인용

- **Cap (CapSoftware/cap)**: 18.3K stars, Tauri 2 + Rust + SolidStart. 75 releases, v0.4.84 (2026-04-15). Win + mac. <https://github.com/CapSoftware/cap>. 본 프로젝트 패턴 차용 가능.
- **Tauri awesome list**: <https://github.com/tauri-apps/awesome-tauri> — 다른 production 앱 (Pot 번역기, Quadrant Minecraft, Teyvat Guide) 모두 NSIS+dmg+AppImage 매트릭스.

### 1.6 인용

- Tauri 2 Windows installer: <https://v2.tauri.app/distribute/windows-installer/>
- Tauri 2 DMG: <https://v2.tauri.app/distribute/dmg/>
- Tauri 2 AppImage: <https://v2.tauri.app/distribute/appimage/>
- Tauri 2 reference config: <https://v2.tauri.app/reference/config/>
- Cap (Tauri 2 production app): <https://github.com/CapSoftware/cap>
- "Ship Your Tauri v2 App Like a Pro" 가이드: <https://dev.to/tomtomdu73/ship-your-tauri-v2-app-like-a-pro-github-actions-and-release-automation-part-22-2ef7>

---

## §2. 코드 사인 정책

### 2.1 Windows OV 인증서 채택 + EV 거부 (v1)

**2024년 변경 사항**: Microsoft가 EV 인증서의 "즉시 SmartScreen 우회" 특혜를 제거. EV도 OV처럼 reputation building 거침. EV 비용($400+/년)이 더 이상 정당화되지 않음.

**채택 (v1)**:
- **OV (Organization Validation) 인증서 — DigiCert / SSL.com / Sectigo / GlobalSign 중 1**.
  - 비용: $150~$300/년 (1~3년).
  - 발급: 회사 등록증 / 등기부등본 검증 (1~2주).
  - 저장: 로컬 .pfx (개인 PC) 또는 Azure Key Vault (CI 친화).
- **2026 새 규정**: 2026-03-01부터 publicly trusted code signing 인증서 max validity 458일. SSL.com 2026-02-27부터 적용. v1 발급 시 1년 단위 갱신 패턴.

**SmartScreen 평판 빌드 패턴**:
1. v1.0.0 첫 release → SmartScreen "Unknown publisher" 경고 (사용자 "More info" → "Run anyway" 클릭 필요).
2. 다운로드 ~5K+ 누적 → 평판 자동 부여 → 경고 사라짐 (보통 3~6개월).
3. v1.0.x 패치마다 동일 인증서로 사인 → 평판 누적.

**기각**: EV 인증서 v1 X. 평판 시간 차이 = OV ~6개월 / EV ~3개월 (2024년 이후 체감 차이 작음). 비용 차이 5배.

**기각**: 비서명 release v1 X. SmartScreen 경고에 한국 일반 사용자 대거 이탈 위험. 비용은 v1 launch 비용 흡수.

### 2.2 mac Developer ID Application ($99/년) + notarytool

**채택**:
- **Apple Developer Program $99/년** 가입 (회사 명의).
- **Developer ID Application 인증서** (Mac App Store 아님 — App Store는 v1 X).
- **notarytool submit** — altool 2023-11 deprecated.
- **stapler staple** — notarization 결과를 .dmg에 영구 부착 (오프라인 검증 가능).

**워크플로 (CI)**: `codesign --deep --force --options runtime --entitlements entitlements.plist --sign "Developer ID Application: ..."` → `xcrun notarytool submit ... --wait` (5~30분 동기 대기) → `xcrun stapler staple` (오프라인 Gatekeeper pass).

`--options runtime`은 Hardened Runtime 활성화 (notarization 필수). 첫 빌드 거절 흔한 원인: entitlements 부족 ("main executable failed strict validation"), ExternalBin 미사인 ("code object not signed at all" — tauri issue #11992), timestamp 누락 (codesign 자동 처리).

### 2.3 Azure Trusted Signing — v1.x 검토

**고려 (v1.x)**: Azure Trusted Signing은 인증서 X + Microsoft가 매번 동적 발급. 비용 $9.99/월 (~$120/년). 키 관리 부담 0 + EV 수준 평판.

**v1 거부 사유**: Azure 계정 + 신원 검증(KYC) 1~2주. v1 launch 시점 초과. v1.x에서 GA 시점에 OV → Azure 전환 검토.

```text
"signCommand": "trusted-signing-cli -e https://wus2.codesigning.azure.net -a MyAccount -c MyProfile %1"
```

(Tauri 2 docs Windows code signing 섹션, signCommand 형식)

### 2.4 인용

- Tauri 2 Windows code signing: <https://v2.tauri.app/distribute/sign/windows/>
- Tauri 2 macOS code signing: <https://v2.tauri.app/distribute/sign/macos/>
- SSL.com OV vs EV: <https://www.ssl.com/faqs/which-code-signing-certificate-do-i-need-ev-ov/>
- 2024 EV SmartScreen 변경: <https://learn.microsoft.com/en-us/answers/questions/417016/reputation-with-ov-certificates-and-are-ev-certifi>
- 2026-03-01 458일 cap: <https://www.sematicon.com/en/ev-code-signing-windows/>
- Apple notarization: <https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution>
- Apple notarytool migration: <https://github.com/electron/notarize/issues/189>
- tauri-action: <https://github.com/tauri-apps/tauri-action>
- Tauri 2 production mac: <https://dev.to/0xmassi/shipping-a-production-macos-app-with-tauri-20-code-signing-notarization-and-homebrew-mc3>

---

## §3. 자동 업데이트 통합

### 3.1 `tauri-plugin-updater` v2 = 본체 자기 업데이트

**구성 (`tauri.conf.json`)**: `bundle.createUpdaterArtifacts: true`, `plugins.updater: { active: true, pubkey: "<minisign 공개 키 base64>", endpoints: ["https://github.com/joycity/lmmaster/releases/latest/download/latest.json"], windows: { installMode: "passive" } }`.

**`latest.json` 형식 (GitHub Releases asset)**: `{version, notes, pub_date, platforms: {"windows-x86_64": {signature, url}, "darwin-x86_64": {...}, "darwin-aarch64": {...}, "linux-x86_64": {...}}}`. `signature`는 minisign base64, `url`은 GitHub Releases asset URL.

**minisign 키 생성**: `pnpm exec tauri signer generate -- -w ~/.tauri/lmmaster.key`. 비밀 키는 절대 commit X (CI `TAURI_SIGNING_PRIVATE_KEY` env로 inject), public key는 `tauri.conf.json` `plugins.updater.pubkey`에 embed (commit OK).

**플랫폼 동작**: Windows = `installMode: "passive"` (NSIS 자동 실행, 진행률 보임, 자동 재시작), macOS = `.app.tar.gz` 다운로드 후 /Applications/ 교체 + 재시작, Linux = AppImage in-place 교체.

### 3.2 ADR-0026 `crates/auto-updater` = 외부 런타임 + 카탈로그 + 모델 추적

**역할 분리** (negative space — §7.4):

| 컴포넌트 | 책임 | endpoint | 채택 도구 |
|---|---|---|---|
| `tauri-plugin-updater` | LMmaster.exe / .app / .AppImage 자기 업데이트 | `github.com/joycity/lmmaster/releases/.../latest.json` | tauri 공식 |
| `crates/auto-updater` | Ollama / LM Studio 외부 런타임 + 카탈로그 manifest + 모델 GGUF | `api.github.com/repos/ollama/ollama/releases/latest` 등 | 자체 (ADR-0026) |

**책임 경계 — 사용자 향 표시 통일**:
- 두 시스템 모두 toast emit 시 동일 컴포넌트 (`UpdateToast`) 사용. user-facing label만 다름:
  - 본체 update: "LMmaster 새 버전이 나왔어요"
  - 외부 update: "Ollama 새 버전이 나왔어요" / "Llama-3.1 모델이 갱신됐어요"
- Settings → "업데이트 채널" 단일 화면에 두 source 모두 표시. 사용자 입장에서는 통합된 경험.

### 3.3 endpoint 충돌 위험 (negative space)

**위험**: `tauri-plugin-updater` endpoint를 잘못 설정해서 `api.github.com/repos/.../releases/latest` 같은 ADR-0026 패턴으로 가리키면:
- tauri-plugin-updater는 latest.json 형식만 이해 → JSON parse fail.
- ADR-0026 crate의 `GitHubReleasesSource`는 GitHub Releases API JSON (다른 schema).

**채택 분리 정책**:
1. `tauri-plugin-updater` endpoint는 **항상 `latest.json`로 끝나는 URL**. (`/releases/latest/download/latest.json` 패턴)
2. `crates/auto-updater`는 **항상 `api.github.com/repos/...` 패턴**.
3. 두 endpoint 변수는 **소스 코드에서 같은 module에 있지 않게** 분리 (`apps/desktop/src-tauri/tauri.conf.json` vs `crates/auto-updater/src/source/github.rs`).
4. 결정 노트 §7.4에 "합치려는 충동 차단" 명시.

### 3.4 첫 출시 테스트 시나리오

- v1.0.0 release → `latest.json` v1.0.0 명시.
- v1.0.1 release → `latest.json` v1.0.1로 갱신 (CI tauri-action이 자동 처리).
- 사용자 앱이 v1.0.0이면 6h poll(우리 plugin 설정 기본값) 후 toast: "1.0.1 새 버전이 나왔어요. 받을까요?".
- 사용자 "받을게요" → 다운로드 → minisign 검증 → installMode 동작.

### 3.5 인용

- Tauri 2 updater plugin: <https://v2.tauri.app/plugin/updater/>
- Tauri 2 GitHub pipeline: <https://v2.tauri.app/distribute/pipelines/github/>
- 자동 업데이트 가이드: <https://thatgurjot.com/til/tauri-auto-updater/>
- Ratul 블로그: <https://ratulmaharaj.com/posts/tauri-automatic-updates/>
- Crab Nebula auto-update: <https://docs.crabnebula.dev/cloud/guides/auto-updates-tauri/>
- Tauri tauri-docs updater (v2): <https://github.com/tauri-apps/tauri-docs/blob/v2/src/content/docs/plugin/updater.mdx>

---

## §4. EULA 노출 흐름

### 4.1 첫 실행 EULA dialog 채택

**시점**: 첫 실행 시 + EULA 버전 갱신 시. 두 경우 모두 본 화면 진입 차단.

**구성**:
1. **언어 자동 감지** — OS locale ko-KR → 한국어 EULA 1차. 그 외 → 영어.
2. **언어 토글** — dialog 우상단 "한국어 / English" 토글 버튼.
3. **EULA 본문** — scroll 영역. 약관 + 라이선스 + 데이터 처리 정책. ~3~5분 읽기 분량.
4. **clickwrap checkbox**: "EULA를 읽었고 동의해요" (초기 unchecked).
5. **버튼**:
   - "동의해요" — checkbox 체크 시 활성. 클릭 시 본 화면 진입.
   - "거절할래요" — 앱 종료. 종료 전 한 번 더 확인 dialog ("정말 거절할까요? 동의하지 않으면 앱을 사용할 수 없어요").
6. **영속**: 동의 시 workspace settings `eula.accepted_version: "1.0.0"`, `eula.accepted_at: "2026-04-28T..."`. 다음 실행 시 동일 버전이면 dialog skip.
7. **EULA 버전 갱신**: semver minor 이상 변경 시 (`1.0.0` → `1.1.0`) 재동의. patch (`1.0.0` → `1.0.1`)는 skip.

### 4.2 LM Studio EULA vs LMmaster 본체 EULA — 분리

| 시점 | 누가 표시 | 우리 책임 |
|---|---|---|
| LM Studio 설치 (외부) | LM Studio installer 자체 | 0 (사용자 직접 동의) |
| LMmaster 첫 실행 | 우리 dialog | 100% |
| `ollama create` 등록 | Phase 5'.e §5 동의 dialog | 100% (Ollama EULA 아닌 *우리 동작 동의*) |

ADR-0016 wrap-not-replace 일관 — LM Studio EULA를 *우리가 다시 표시*하지 않음 (사용자 혼란 + 법적 모호성).

### 4.3 한국어 EULA UX 패턴

**채택 톤** (CLAUDE.md §4.1 매뉴얼 일관):
- "이 앱을 쓰려면 약관에 동의해 주세요" (해요체).
- "EULA를 읽었고 동의해요" (clickwrap label).
- "동의해요" / "거절할래요" (버튼).
- "정말 거절할까요? 동의하지 않으면 앱을 사용할 수 없어요" (확인 dialog).

**금지**:
- "I Accept" / "Decline" 영어 그대로.
- "동의하시겠습니까?" 공식체.
- "약관 동의" 명사구 버튼.

### 4.4 인용

- TermsFeed clickwrap: <https://www.termsfeed.com/blog/eula-installation/>
- EULA template: <https://www.eulatemplate.com/>
- 한국어 데스크톱 앱 UX 참고:
  - 카카오톡 PC 첫 실행 — 한국어 1차 + clickwrap. 약관 보기 link.
  - 네이버 웨일 — OS 언어 자동 감지 + 토글.

---

## §5. 베타 채널 + 텔레메트리

### 5.1 베타 채널 = Settings 토글 + 별도 endpoint

**채택**: Settings → "업데이트" 섹션 토글 "베타 채널 참여할게요" + 부연 "정식 출시 전 새 기능을 미리 써 볼 수 있어요. 안정성이 떨어질 수 있어요". OFF(기본) → endpoint `release/latest/download/latest.json`. ON → `latest-beta.json` (별도 pre-release 채널).

**구현 노트**: tauri-plugin-updater endpoint 동적 변경 미지원 → 두 endpoint를 array로 등록 + toggle 시 plugin restart. v1은 단순 array, v1.x에서 무중단 toggle. **베타 → 정식 한 방향**: 정식 v1.1.0 release 시 베타 사용자(v1.1.0-beta.5)에게 "정식 v1.1.0이 나왔어요. 베타 채널을 끄고 옮길까요?" 안내.

### 5.2 텔레메트리 = opt-in only + 본체 crash report만 v1

**ADR-0013 외부 통신 0 일관**. 기본 OFF. Settings → "개인정보" 토글 "익명 크래시 보고서 보낼게요" + 부연 "사용 데이터는 포함되지 않아요". ON 시 panic / unhandled error만 전송.

**v1 메트릭 = 0 (crash report만)**: 사용 통계 X, 모델 ID X, 시스템 정보는 GPU 모델 / VRAM / OS major version만 (개인 식별 X). v1.1+에서 사용자 자발적 "제품 개선에 도움 될게요" 토글로 익명 사용 통계 opt-in 추가 검토.

### 5.3 텔레메트리 채택 도구

| 도구 | 비용 | self-hosted | Rust SDK |
|---|---|---|---|
| Sentry SaaS | 무료 5K/월, $26+/월 | X | sentry crate |
| GlitchTip self-hosted | ~$10/월 VPS | O | sentry SDK 호환 |
| 자체 endpoint | 0 | O | 직접 구현 |

**채택 (v1)**: GlitchTip self-hosted. Sentry SDK 호환 + 모든 데이터 우리 서버만. **기각**: Sentry SaaS (첫 외부 의존, 사용자 신뢰 해칠 위험), 자체 endpoint (stack trace 분석 도구 구현 부담 — v1.x 후순위).

### 5.4 인용

- Sentry alternatives 2026: <https://vemetric.com/blog/best-sentry-alternatives>
- GlitchTip self-hosted: <https://glitchtip.com/>
- Sentry Rust SDK: <https://docs.sentry.io/platforms/rust/>
- ADR-0013 외부 통신 0 — 사용자 opt-in 시점에 단일 endpoint 예외 허용.

---

## §6. 다국어 / 출시 자료

### 6.1 README 매트릭스

**채택**:
- `README.md` — 영어 1차 (외부 사용자 첫 접근).
- `README.ko.md` — 한국어 1차 (한국 사용자 친화). 영어 README 상단에 link.
- 일본어 / 중국어 — v1.x (사용자 요청 시).

**한국어 README 톤** (CLAUDE.md §4.1):
- "LMmaster는 LM Studio와 Ollama를 한국어로 감싸는 데스크톱 앱이에요."
- "5분 안에 LLM을 PC에서 돌려 볼 수 있어요."

**영어 README 톤**:
- "LMmaster: A Korean-first desktop companion for LM Studio and Ollama."

### 6.2 GitHub Releases v1 1순위

**v1**: GitHub Releases (.exe / .dmg / .AppImage 직접 다운로드).
- 무료 + 신뢰도 + 익숙함.
- mac App Store / Microsoft Store / Snap / Flatpak 모두 v1.x.

**v1.x 검토**:
- mac App Store: sandbox + sidecar 충돌 (Phase 5' python sidecar). 우회 가능하지만 별도 ADR.
- Microsoft Store: portable workspace 정신과 일부 충돌 (Store는 system-wide install). v1.x.

### 6.3 인용

- GitHub Releases CDN 패턴 (Tauri pipelines): <https://v2.tauri.app/distribute/pipelines/github/>
- mac App Store 제한 (sandbox): Apple Developer docs 일반 가이드.

---

## §7. 위험 노트 (next session 함정)

### 7.1 Windows SmartScreen "Unknown Publisher" 경고 — 사용자 대거 이탈

**위험**: 비서명 release 시 Windows Defender SmartScreen이 "Unknown Publisher" 경고 → "More info" → "Run anyway" 2단계 클릭 필요. 한국 일반 사용자 ~50% 이탈 추정 (게임 업계 ~30~70% 변동).

**완화**:
1. v1 OV 인증서 사인 — 6개월 평판 빌드 동안 경고 가능.
2. README + 다운로드 페이지에 "처음 받을 때 경고가 나올 수 있어요" 한국어 안내.
3. 실행 중 LMmaster 본체에서 다음 release update는 minisign 자동 검증으로 매끄러움.

**다음 세션 함정**: "v1은 비서명도 OK"라며 인증서 미발급 → 위 시나리오 직격. v1 launch 전 OV 발급 1주 lead time 확보 필수.

### 7.2 mac notarization 첫 빌드 거절 — entitlements 부족

**위험**: notarization 첫 시도 시 거절 빈번. 가장 흔한 원인:
1. Hardened Runtime 누락 (`--options runtime` 명시 X).
2. entitlements.plist 부족 (`allow-unsigned-executable-memory` 없으면 llama.cpp JIT 거절).
3. ExternalBin (sidecar) 미사인 (tauri issue #11992).

**완화**:
1. 본 노트 §1.4 entitlements.plist를 v1 release pre-flight에 commit.
2. `xcrun notarytool log <submission-id>` JSON으로 거절 사유 파싱 → CI에 포함.
3. **로컬 검증**: `spctl -a -vv LMmaster.app` — Gatekeeper assess. release 전 100% pass 보장.

### 7.3 자동 업데이트 무결성 검증 안 하면 supply-chain 위험

**위험**: minisign signature 검증 안 하면 GitHub 계정 탈취 시 악성 페이로드 배포 가능. 사용자 PC에 임의 코드 실행.

**완화**:
1. `tauri-plugin-updater`는 **항상 minisign 검증** (default ON, 비활성화 옵션 없음). 우리는 `pubkey` 명시.
2. private key는 CI secret에만 (`TAURI_SIGNING_PRIVATE_KEY`). repo / commit / 로컬 PC 평문 저장 금지.
3. 키 분실 / 노출 시 **모든 사용자 강제 재설치 필요** (signature 변경) — disaster scenario.
4. 키 백업: 1Password / KeyHub 같은 secrets vault. 회사 SOC 정책 일관.

### 7.4 endpoint 충돌 — `tauri-plugin-updater` vs ADR-0026

**위험**: 두 시스템 모두 GitHub Releases를 source로 쓰면 endpoint URL이 비슷해 보임 → 다음 세션이 "통합" 시도 → tauri-plugin-updater가 ADR-0026 endpoint를 가리키면 schema mismatch (latest.json vs GitHub Releases API JSON).

**완화 (negative space — §3.3 일관)**:
1. **명확한 책임 분리** — 본 노트 §3.2 매트릭스.
2. **endpoint 패턴 분리**:
   - tauri-plugin-updater: 항상 `releases/latest/download/latest.json`.
   - ADR-0026: 항상 `api.github.com/repos/.../releases/latest`.
3. **결정 노트 + ADR에 negative space 명시** — "두 시스템을 합치려는 충동 차단".
4. **테스트 invariant**: tauri-plugin-updater mock + ADR-0026 mock이 *서로 다른 schema*를 처리한다는 invariant 보존.

### 7.5 EULA 버전 갱신 vs 사용자 재동의 부담

**위험**: EULA를 매 패치마다 갱신 → 사용자가 매 업데이트마다 재동의 → 피로 누적 → "다 동의함" 클릭 습관 → 의미 잃음.

**완화**:
1. EULA semver 정책 명시 (§4.1):
   - patch (1.0.0 → 1.0.1) → skip 재동의 (오타 / 작은 명확화).
   - minor (1.0.0 → 1.1.0) → 재동의 (기능 변화 / 데이터 처리 변경).
   - major (1.0.0 → 2.0.0) → 강제 재동의.
2. 사용자 향 갱신 사유 표시 — "EULA가 갱신됐어요. 변경 사항: [요약]".
3. 갱신 빈도 자체를 줄임 — v1 EULA를 robust하게 작성 (§4.2의 LM Studio + Ollama 분리).

### 7.6 베타 채널 데이터 파편화

**위험**: 베타 사용자의 workspace data를 정식 사용자가 받으면 schema 불일치 → 마이그레이션 실패.

**완화**:
1. workspace SQLite schema_version 명시 (이미 ADR-0008 sqlite-storage).
2. beta-only schema 변경은 *upgrade path만* 허용 (downgrade X).
3. 정식 → 베타 toggle on은 데이터 영향 X (read-only). 베타 → 정식 toggle off는 schema 호환 시만.

### 7.7 텔레메트리 opt-in이 v1.x에서 default ON으로 바뀔 위험

**위험**: 사용자 신뢰 빌드 후 v1.5+ 시점에 PM이 "사용 통계가 필요해요"라며 opt-in default를 ON으로 변경 → 사용자 신뢰 파괴.

**완화 (negative space)**:
1. **메모리에 영속**: `competitive_thesis` + 본 노트에 "텔레메트리 opt-in 원칙은 ADR-0013과 동등 의무"라 못 박음.
2. v1.x 변경은 ADR 신설 + 사용자 명시 동의 필수.
3. 메모리 + ADR 양쪽에 negative space 보존 — "PM이 default ON 요청해도 거부".

---

## §8. 참고 (선행 페이즈 + ADR)

### 8.1 ADR-0026 (auto-updater 외부 런타임 추적) 인용

본 노트 §3.2의 책임 분리 = ADR-0026 Consequences "본체 자기 업데이트 분리" 일관. 두 시스템 endpoint 분리 + 사용자 향 표시 통일.

### 8.2 Phase 1A.3.b.2 dual zip-slip 방어 인용

Phase 1A.3 installer는 zip-slip / tar-slip 방어 (악성 archive 차단). `tauri-plugin-updater`는 .app.tar.gz 다운로드 후 unpack — 동일 방어 필요. 본 노트 § 7.3 minisign 검증과 별도, archive 무결성도 검증.

**채택**: tauri-plugin-updater v2는 internal로 `tar` crate + 표준 path validation (Tauri 2.0+ default). 우리는 별도 구현 X.

### 8.3 ADR-0013 외부 통신 0 + opt-in 예외 일관

ADR-0013은 "외부 통신 0" 원칙. 본 노트 §5.2 텔레메트리는 사용자 명시 opt-in으로 단일 endpoint 예외. ADR-0013 본문 "단, 사용자 동의 + 단일 도메인은 예외 허용".

### 8.4 다음 세션 인계 — Phase 7'.a (bundler config + sign 환경 셋업)

**진입 조건**:
- 회사 Apple Developer Program $99 가입 완료.
- OV 인증서 발급 1~2주 lead time 시작.
- minisign keypair 생성 + private key vault 저장.

**Phase 7'.a 산출물**:
- `tauri.conf.json` bundle 매트릭스 완성 (본 노트 §1).
- entitlements.plist 작성 (§1.4).
- GitHub Actions release workflow (.github/workflows/release.yml).
- minisign keypair 생성 + pubkey embed.

**Phase 7'.b 산출물**:
- 첫 실행 EULA dialog 컴포넌트 (한국어 1차).
- 베타 채널 + 텔레메트리 토글 Settings UI.
- README.md / README.ko.md.

**Phase 7'.c 산출물**:
- v1.0.0-rc.1 release (베타 채널만).
- v1.0.0 정식 release (3주 베타 후).

---

## §9. 결정 노트 6-섹션 매핑 (CLAUDE.md §4.5)

| 섹션 | 본 노트 위치 |
|---|---|
| §1 결정 요약 | §0 (10가지) |
| §2 채택안 | §1 / §2 / §3 / §4 / §5 / §6 |
| §3 기각안 + 이유 | §1.1 (MSI 거부), §1.2 (pkg 거부), §1.3 (deb/rpm 거부), §2.1 (EV v1 거부 + 비서명 거부), §5.3 (Sentry SaaS / 자체 endpoint 거부), §6.2 (App Store v1 거부) |
| §4 미정 / 후순위 | §1.1 (MSI v1.x), §2.3 (Azure Trusted Signing v1.x), §5.1 (베타 채널 무중단 toggle v1.x), §5.2 (사용 통계 opt-in v1.x), §6.1 (ja/zh README v1.x) |
| §5 테스트 invariant | §3.4 (자동 업데이트 시나리오), §7 위험 완화 invariant 매핑 |
| §6 다음 페이즈 인계 | §8.4 (Phase 7'.a / 7'.b / 7'.c) |

---

## §10. 테스트 invariant (sub-phase DoD)

### 10.1 bundler 산출물 GitHub Actions 자동 빌드 + 사인

`.github/workflows/release.yml` — `on: push: tags: ["v*"]` 트리거. matrix: `{windows-latest: nsis, macos-latest: dmg, ubuntu-latest: appimage}`. `tauri-apps/tauri-action@v0` 사용. CI secrets: `TAURI_SIGNING_PRIVATE_KEY` (minisign), `APPLE_ID` / `APPLE_TEAM_ID` / `APPLE_APP_SPECIFIC_PASSWORD` / `APPLE_CERTIFICATE` (mac), `WINDOWS_CERTIFICATE` / `WINDOWS_CERTIFICATE_PASSWORD` (Win OV pfx).

| Invariant | 시나리오 |
|---|---|
| Win build success | tag push → NSIS .exe + signed + uploaded to Release |
| mac build success | tag push → .dmg + signed + notarized + stapled + uploaded |
| Linux build success | tag push → .AppImage + signed + uploaded |
| latest.json generated | tauri-action이 latest.json asset 자동 생성 |
| minisign signature verifies | release asset의 .sig 파일이 pubkey로 검증 통과 |

### 10.2 자동 업데이터 payload 무결성 (minisign signature)

wiremock으로 latest.json mock + signature 변조 → `UpdaterError::SignatureVerificationFailed` 단언.

| Invariant | 시나리오 |
|---|---|
| valid signature passes | 정확한 signature → install 진행 |
| invalid signature fails | 잘못된 signature → 즉시 reject + 사용자 향 한국어 에러 |
| missing signature fails | signature 필드 없음 → reject |
| corrupt download fails | url에서 받은 binary가 sha256 mismatch → reject |
| pubkey mismatch fails | latest.json 서명이 다른 키로 됐으면 → reject |

### 10.3 EULA 동의 안 한 사용자 차단

`TestApp::new_first_run()` → 모든 IPC 호출 `EulaError::NotAccepted`. `app.accept_eula("1.0.0")` 후 IPC 정상.

| Invariant | 시나리오 |
|---|---|
| 동의 안 함 → 모든 IPC 차단 | EulaError::NotAccepted |
| 동의 후 → 모든 IPC 허용 | 정상 |
| EULA 버전 minor 갱신 → 재동의 강제 | 1.0.0 동의자가 1.1.0 EULA 환경에서 차단 |
| EULA 버전 patch 갱신 → 재동의 skip | 1.0.0 동의자가 1.0.1 EULA 환경에서 통과 |
| EULA 거절 → 앱 종료 | "거절할래요" 클릭 → process exit 0 |

### 10.4 한국어 톤 invariant

EULA + 베타 채널 + 텔레메트리 모든 사용자 향 문구가 §4.1 카피 톤 매뉴얼 일관:
- 해요체 / 명사구 라벨 / 영어 노출 0 / loanword(VRAM 등)만 허용.
- 자동 grep test: `grep -E "(I Accept|Decline|Install|Loading)" apps/desktop/src/locales/ko.json` → match 0건.

---

**버전**: v1.0 (2026-04-28). Phase 7' 진입 직전. 다음 갱신: 인증서 발급 + GitHub Actions release.yml 작성 후 검증 결과 추가.

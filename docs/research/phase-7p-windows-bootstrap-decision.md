# Phase 7'.x — Windows 배포 부트스트랩 호환성 응급 픽스 결정 노트

> 작성일: 2026-05-05
> 상태: 구현 완료 (`cargo check` 통과). 재빌드 + 다른 PC 검증 + 서명 결정 standby.
> 트리거: 사용자가 v0.0.1 NSIS installer를 다른 PC로 복사 → 실행 시 권한/실행 차단. 어제 차단됐다가 오늘 해제되는 비결정성 관찰됨 → SmartScreen reputation 비결정성 + 환경 의존 버그 동시 의심.
> 선행: Phase 7' 보강 리서치 (`docs/research/phase-7p-release-prep-reinforcement.md`).
> 후행: Phase 7'.b (코드 서명 도입 + README 매트릭스 + EULA dialog).

---

## §1. 결정 요약

1. **`+crt-static` 활성화** — `apps/desktop/src-tauri/.cargo/config.toml` 신설. windows-msvc 타깃에 정적 CRT 강제. VC++ Redistributable 미설치 PC에서도 영구 실행 보장.
2. **WebView2 `embedBootstrapper`** — `tauri.conf.json` `bundle.windows.webviewInstallMode` 명시. 인터넷/사내망 정책에 좌우되지 않는 자동 설치. installer +1.8MB 수용.
3. **v0.0.1 unsigned 유지 + SmartScreen 안내 카피로 brigding** — 서명 결정(#6 task)은 사업자등록 여부에 따라 Azure Trusted Signing vs OV vs unsigned 분기. 분기 결정 전에는 한국어 SmartScreen 우회 안내(§2.3)로 사용자 가이드.

---

## §2. 채택안

### §2.1 `+crt-static` (정적 CRT 링크)

**파일**: `apps/desktop/src-tauri/.cargo/config.toml` (신규)

```toml
[target.'cfg(all(windows, target_env = "msvc"))']
rustflags = ["-C", "target-feature=+crt-static"]
```

**효과**:
- `VCRUNTIME140.dll` / `MSVCP140.dll` 의존 제거. VC++ Redist 미설치 PC (Win10 일부 클린 이미지, 회사 표준 이미지 일부)에서도 즉시 실행.
- binary size 약 +200KB (LMmaster 본체 ~30MB 대비 무시할 수준).
- LTO / 다른 컴파일 플래그와 충돌 없음 (host build에는 영향 0 — `target.'cfg(...)'.rustflags`가 target build에만 적용).

**범위 제어**:
- 이 config.toml은 `apps/desktop/src-tauri/` 하위에만 영향. 워크스페이스 루트 (`cargo test --workspace`)는 검색되지 않음. 다른 crate의 dev test 인프라 영향 0.
- 의도와 정확히 일치 — 최종 배포 exe는 src-tauri에서 빌드, 다른 crate들은 동적 CRT로 둠 (테스트 빌드 속도 우선).

### §2.2 WebView2 `embedBootstrapper` + silent

**파일**: `apps/desktop/src-tauri/tauri.conf.json` (변경)

```json
"webviewInstallMode": {
  "type": "embedBootstrapper",
  "silent": true
},
"allowDowngrades": false,
```

**효과**:
- WebView2 누락 PC에서 installer가 임베드된 부트스트래퍼로 자동 설치. 인터넷/사내망 정책 의존 0.
- `silent: true` — 사용자에게 별도 dialog 없이 자동 진행 (installer UX 일관).
- `allowDowngrades: false` — 자동 업데이트로 다운그레이드 방지 (sql/schema 호환성 보장).

### §2.3 SmartScreen 한국어 우회 안내 카피 (해요체)

**용도**: v0.0.1 unsigned 유지 시 사용자 향 안내. 다음 위치 모두 동일 톤:
- 다운로드 페이지 (GitHub Releases 본문)
- 설치 가이드 (Phase 7'.b README.ko.md 작성 시)
- (옵션) 첫 실행 후 도움말 화면

**카피** (CLAUDE.md §4.1 매뉴얼 일관):

> ### 처음 실행할 때 "Windows에서 PC를 보호했습니다" 화면이 떠요
>
> LMmaster는 아직 Microsoft 평판이 쌓이지 않은 신생 앱이라 그래요. 코드 자체는 안전해요 — 아래 해시로 직접 확인해 볼래요?
>
> 1. 다운로드 폴더에서 `LMmaster_*.exe` 우클릭 → 속성 → 일반 탭 하단 **"차단 해제"** 체크 → 확인.
> 2. installer 다시 실행.
> 3. 그래도 SmartScreen이 뜨면 **"추가 정보" → "실행"** 클릭.
>
> ### 안전한 파일이 맞는지 확인하고 싶어요
>
> PowerShell에서:
>
> ```powershell
> Get-FileHash .\LMmaster_0.0.1_x64-setup.exe -Algorithm SHA256
> ```
>
> 결과가 GitHub Release notes의 SHA256과 같으면 무결성 OK예요.

**금지 (CLAUDE.md §4.1)**:
- 영어 뱅크 문구 그대로 노출 ("Click 'More Info'", "Run anyway") — *대신* 한국어 표시명 + 영어 병기.
- "설치하시겠습니까?" 공식체.
- 의문문 호명 ("어떻게 설치할까요?").

---

## §3. 기각안 + 이유

### §3.1 `downloadBootstrapper` (Tauri 2 기본값) 유지 — 거부

**거부 이유**:
- 사내망 / 공공기관 / 학교망 등 일부 정책으로 `EdgeUpdate.exe` 다운로드 차단. installer가 WebView2 단계에서 무한 대기.
- 인터넷이 없는 PC에서 즉시 실패.
- 한국 환경 (사내망 + 공공망 비중 높음) 부적합.

### §3.2 `offlineInstaller` (+127MB) — 거부

**거부 이유**:
- installer 크기 ~127MB 증가. CDN 비용 + 다운로드 UX 저하.
- 대부분 사용자는 이미 WebView2 설치되어 있음 — 임베드 부트스트래퍼가 빠른 path 자동 선택.
- v1.x에서 "오프라인 빌드" 별도 채널로만 검토 (사용자 요청 시).

### §3.3 `fixedVersion` WebView2 — 거부

**거부 이유**:
- WebView2 자동 업데이트 OFF → 보안 패치 누락 위험. Tauri docs도 "특정 chromium feature lock 필요할 때만" 권장.
- LMmaster는 latest WebView2와 호환 OK (특수 chromium feature 의존 없음). lock 불필요.

### §3.4 dynamic CRT (Rust MSVC default) 유지 — 거부

**거부 이유**:
- VC++ Redistributable 미설치 PC에서 `VCRUNTIME140.dll` missing → 실행 자체 불가. 사용자가 "권한 문제"로 오인하는 한국어 윈도우 에러 메시지.
- Tauri 이슈 #719에서 "auto-magical 처리" 요청 중이지만 아직 미구현 → 수동 명시 필요.
- 정적 CRT 비용 (binary +200KB)이 사용자 가치 (배포 호환성) 대비 무시할 수준.

### §3.5 self-signed cert — 거부

**거부 이유**:
- 받는 PC가 자체 root store에 자체 cert 추가해야 trust — 일반 사용자 비현실적.
- 보안적으로도 위험 신호 (malware 패턴과 동일).
- SmartScreen reputation에도 도움 안 됨 (Microsoft 신뢰 chain 외부).

### §3.6 v0.0.1 즉시 EV 인증서 발급 — 거부

**거부 이유**:
- HSM USB 토큰 별송 1주 + 비용 $300~700/년 + 키 분실 시 disaster scenario.
- 2024년 변경으로 EV의 SmartScreen 즉시 우회 특혜 제거됨 → OV와 reputation 누적 시간 차이 미미.
- 같은 효과를 Azure Trusted Signing (#6 task)이 $9.99/월 + ID 검증으로 제공.
- v0.0.x는 unsigned + 사용자 안내로 brigding, 사업자등록 확인 후 Trusted Signing 도입이 정통.

---

## §4. 미정 / 후순위

### §4.1 코드 서명 도입 (#6 task) — 사용자 결정 사항

**분기**:

| 옵션 | 비용 | 즉시성 | 조건 |
|---|---|---|---|
| Azure Trusted Signing (Basic) | $9.99/월 | reputation 즉시 (MS Identity Verification) | 사업자등록 또는 정부 발급 ID + Microsoft Partner Network 가입 |
| 표준 OV (DigiCert/Sectigo/SSL.com) | $200~400/년 | reputation 누적 수개월 (수천 회 설치 필요) | HSM USB 토큰 또는 Azure Key Vault |
| unsigned + 안내 (현재 v0.0.x) | $0 | reputation 자연 누적 (수년 가능) | README + 다운로드 안내 |

**추천 (사업자등록 보유 시)**: Azure Trusted Signing. 가성비 압도적 + Tauri 2 `signCommand` + `trusted-signing-cli`로 1줄 통합. **단**, MS ID 검증에서 indie 거절 사례(GitHub 이슈 #9578) 확인 후 결정.

**Phase 7'.b 인계**:
- `tauri.conf.json` `bundle.windows.signCommand` + `digestAlgorithm` + `timestampUrl` + `tsp: true` 4줄 추가.
- GitHub Actions release.yml에 `AZURE_TENANT_ID` / `AZURE_CLIENT_ID` / `AZURE_CLIENT_SECRET` 3 secret 주입.
- `trusted-signing-cli` cargo install step 추가.

### §4.2 README 매트릭스 (Phase 7'.b)

`docs/research/phase-7p-release-prep-reinforcement.md` §6.1 결정 — `README.md`(영어 1차) + `README.ko.md`(한국어 1차). §2.3 SmartScreen 안내는 `README.ko.md`의 "설치" 섹션에 흡수.

### §4.3 GitHub Release SHA256 + minisign 자동 게시

`tauri-action`은 minisign 자동 처리. SHA256은 GitHub Actions step 추가 (1 line).

```yaml
- name: SHA256 hashes
  run: Get-FileHash .\target\release\bundle\nsis\*.exe -Algorithm SHA256 | Format-Table -AutoSize
```

Release notes 본문에 자동 첨부. Phase 7'.b 인계.

---

## §5. 테스트 invariant (DoD)

### §5.1 빌드 sanity

| Invariant | 검증 |
|---|---|
| `cargo check -p lmmaster-desktop --target x86_64-pc-windows-msvc` 통과 | ✅ 2m 10s, exit 0 (2026-05-05 첫 검증) |
| `tauri.conf.json` schema validation 통과 | ✅ tauri-build이 빌드 시점 검증 (cargo check 일관) |
| `apps/desktop/src-tauri/.cargo/config.toml` 워크스페이스 다른 crate 영향 없음 | `cargo test --workspace`는 워크스페이스 root에서 실행 → 검색 안 됨. host build 영향 0. |

### §5.2 배포 호환성 (수동 검증 — 사용자 standby)

| Invariant | 검증 시나리오 |
|---|---|
| Win10 22H2 + WebView2 미설치 + VC++ Redist 미설치 PC | NSIS installer 더블클릭 → SmartScreen 통과 (또는 안내 우회) → WebView2 자동 설치 → 본체 실행 → 첫 화면 도달 |
| Win11 + 사내망 (proxy / 인터넷 제한) PC | 동일 시나리오 + WebView2 다운로드 단계에서 실패 안 함 (embedBootstrapper) |
| 다른 한국어 사용자 PC (사용자 가족/지인 PC) | "권한 문제" 메시지 0 + SmartScreen 안내 따라 실행 |

### §5.3 SHA256 무결성 (자동 + 수동)

| Invariant | 검증 |
|---|---|
| 빌드된 NSIS .exe SHA256이 Release notes의 해시와 일치 | `Get-FileHash` 결과 == Release body 해시 |
| 사용자가 PowerShell에서 `Get-FileHash` 명령 따라 검증 가능 | §2.3 카피 그대로 동작 |

---

## §6. 다음 페이즈 인계

### §6.1 즉시 다음 standby

1. **재빌드** — `pnpm tauri build` (Windows). NSIS installer 생성. ~30~60분 소요.
2. **다른 PC 검증** — 사용자가 어제 차단됐던 PC 또는 새 PC에서 §5.2 invariant 확인.
3. **#6 서명 결정** — 사업자등록 여부 + Azure 구독 의향 확인. Trusted Signing 신청 OR unsigned 유지 결정.

### §6.2 Phase 7'.b 진입 조건

- v0.0.1 → v0.0.2 bump (이번 변경 반영).
- 결정 노트 + RESUME 갱신.
- 사용자가 다른 PC 검증 OK 확인.

### §6.3 Phase 7'.b 산출물

- `tauri.conf.json` 코드 서명 4줄 (signCommand / digestAlgorithm / timestampUrl / tsp).
- **`bundle.createUpdaterArtifacts: false → true` 복구** (2026-05-05 빌드 검증 시 minisign private key 부재로 임시 false. Phase 7'.b에서 `TAURI_SIGNING_PRIVATE_KEY` CI secret 주입 후 다시 true).
- GitHub Actions release.yml 서명 통합.
- `README.md` (영어) + `README.ko.md` (한국어) — 본 §2.3 안내 카피 흡수.
- `latest.json` GitHub Release asset 자동 생성 + minisign 자동 서명 (이미 conf에 pubkey embed 됨, private key는 CI secret).

### §6.4 위험 노트 (next session 함정)

- **`+crt-static`은 windows-msvc만 적용** — macOS / Linux 빌드 영향 0. `cfg(all(windows, target_env = "msvc"))` guard 필수. 다음 세션에서 이 guard를 풀면 비-Windows 빌드 깨짐.
- **`embedBootstrapper`는 installer 크기 +1.8MB만 추가** — 의도적. 사용자가 "왜 installer가 커졌어?" 의문 가져도 §3.2 (`offlineInstaller` 거부) 기각 사유와 함께 유지.
- **Azure Trusted Signing은 ID 검증 거절 위험 존재** — GitHub 이슈 #9578에 indie 거절 사례 누적. 사업자등록 + Microsoft Partner Network 가입 후에만 실패율 낮음. unsigned fallback 경로 유지.
- **EULA dialog (Phase 7'.b §4)와 본 §2.3 안내는 별개** — EULA는 본체 실행 후 첫 화면, §2.3은 installer 실행 *전* 다운로드 페이지 안내. 한 번에 합치려는 충동 차단 (negative space).

---

## §7. 결정 노트 6-섹션 매핑 (CLAUDE.md §4.5)

| 섹션 | 본 노트 위치 |
|---|---|
| §1 결정 요약 | §1 (3가지) |
| §2 채택안 | §2.1 / §2.2 / §2.3 |
| §3 기각안 + 이유 | §3.1 / §3.2 / §3.3 / §3.4 / §3.5 / §3.6 |
| §4 미정 / 후순위 | §4.1 / §4.2 / §4.3 |
| §5 테스트 invariant | §5.1 / §5.2 / §5.3 |
| §6 다음 페이즈 인계 | §6.1 / §6.2 / §6.3 / §6.4 |

---

## §8. 출처

- Tauri 2 Windows installer (webviewInstallMode 5종 매트릭스): <https://v2.tauri.app/distribute/windows-installer/>
- Tauri 2 Windows code signing (signCommand / Trusted Signing): <https://v2.tauri.app/distribute/sign/windows/>
- Tauri 이슈 #719 (`+crt-static` auto-magical 미지원): <https://github.com/tauri-apps/tauri/issues/719>
- Tauri 이슈 #9578 (Azure Trusted Signing 통합 + indie ID 거절 사례): <https://github.com/tauri-apps/tauri/issues/9578>
- Rust users forum (windows-binaries-vcruntime140-dll-not-found): <https://users.rust-lang.org/t/windows-binaries-vcruntime140-dll-not-found-unless-crt-static/94517>
- Microsoft Learn (Trusted Signing / Artifact Signing 2026-01 리브랜드): <https://learn.microsoft.com/en-us/azure/trusted-signing/overview>
- Hendrik Erz — Code Signing With Azure Trusted Signing on GitHub Actions: <https://www.hendrik-erz.de/post/code-signing-with-azure-trusted-signing-on-github-actions>
- Microsoft Q&A (SmartScreen reputation 메커니즘): <https://learn.microsoft.com/en-us/answers/questions/192721/>
- Phase 7' release prep 보강 리서치 (선행 노트): `docs/research/phase-7p-release-prep-reinforcement.md`

---

**버전**: v1.0 (2026-05-05). 다음 갱신: 다른 PC 검증 결과 + 서명 결정 후.

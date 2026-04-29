# GitHub Secrets 설정 가이드 (Phase 7'.b)

> 본 문서는 LMmaster의 릴리스 자동화에 필요한 GitHub Secrets를 등록하는 절차를 한국어 해요체로 안내해요.
>
> 대상: 저장소 관리자 (소유자) — `freessunky-bit/lmmaster`.

---

## 0. 사전 준비

- GitHub CLI (`gh`)가 설치돼 있어야 해요. [공식 설치 가이드](https://cli.github.com/).
- `gh auth login`을 한 번 실행해 두어야 해요.
- 본 저장소 위치(`./LMmaster`)에서 명령을 실행해 주세요.

---

## 1. Tauri minisign 비밀키 등록 (필수)

자동 업데이터(`tauri-plugin-updater`)가 릴리스 자산에 minisign 서명을 부착하려면 비밀키가 필요해요.

### 1.1 비밀키 파일 등록

```bash
# 사전: 비밀키는 ~/.tauri/lmmaster.key (CLAUDE 세션에서 이미 생성).
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/lmmaster.key
```

`<` 리다이렉트를 쓰면 키가 GitHub Actions secret 저장소로 직접 들어가서 로컬 echo / 클립보드 복사 단계를 생략할 수 있어요.

### 1.2 비밀키 패스워드 등록

```bash
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --body "여기에-비밀키-패스워드"
```

> 패스워드를 잊어버리면 비밀키 자체를 폐기하고 새로 만들어야 해요. 1Password / KeyHub 같은 사내 vault에 한 번 더 보관해 두는 걸 권장해요.

---

## 2. GlitchTip telemetry DSN (옵션)

자체 운영하는 GlitchTip 서버를 두고 텔레메트리를 받고 싶을 때만 등록해요. 미등록 시 LMmaster는 이벤트를 큐에만 적재하고 외부 통신을 하지 않아요(ADR-0013 외부 통신 0 일관).

```bash
# DSN 형식: https://<key>@<host>[:port]/<project_id>
gh secret set LMMASTER_GLITCHTIP_DSN --body "https://abcdef123456@telemetry.example.com/1"
```

> DSN을 등록하면 `release.yml`이 빌드 시점에 환경변수로 주입되지 않아요. 본 secret은 *런타임* 사용을 위한 placeholder예요. 실제 LMmaster 사용자 PC에 DSN을 주입하려면 v1.x에서 별도 채널 (서명된 매니페스트 / 사용자 동의 dialog)이 필요해요.

---

## 3. (옵션 v1.x) Authenticode / Apple Developer 인증서

현재 Phase 7'.b는 인증서 미보유로 unsigned ship이에요. 추후 인증서를 발급받으면 다음을 등록하고 `release.yml`의 주석 처리된 환경변수를 활성화해 주세요.

### 3.1 Windows OV 인증서 (추후)

```bash
# pfx 파일을 base64로 인코딩한 뒤 등록.
base64 -i lmmaster-codesign.pfx -o lmmaster-codesign.pfx.b64
gh secret set WINDOWS_CERTIFICATE < lmmaster-codesign.pfx.b64
gh secret set WINDOWS_CERTIFICATE_PASSWORD --body "pfx-password-here"
```

### 3.2 Apple Developer ID (추후)

```bash
gh secret set APPLE_CERTIFICATE < lmmaster-developer-id.p12.b64
gh secret set APPLE_CERTIFICATE_PASSWORD --body "p12-password"
gh secret set APPLE_SIGNING_IDENTITY --body "Developer ID Application: 회사명 (TEAM_ID)"
gh secret set APPLE_ID --body "developer@example.com"
gh secret set APPLE_PASSWORD --body "app-specific-password"
gh secret set APPLE_TEAM_ID --body "ABCDE12345"
```

> Apple notarytool은 `APPLE_PASSWORD`에 *앱 전용 패스워드* (App-Specific Password)를 받아요. 일반 Apple ID 패스워드가 아니에요.

---

## 4. 등록된 secret 확인

```bash
gh secret list
```

`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 두 줄이 보이면 첫 릴리스 빌드를 시도할 수 있어요.

---

## 5. 키 분실 / 노출 시 재발급 절차

### 5.1 minisign 키 재발급

```bash
# 새 키 생성 (기존 키와 별도 경로 권장).
pnpm exec tauri signer generate -- -w ~/.tauri/lmmaster.key.new

# tauri.conf.json plugins.updater.pubkey를 새 base64 값으로 교체.
# secret 갱신 — 모든 사용자가 다음 업데이트에서 새 서명을 검증하게 돼요.
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/lmmaster.key.new
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --body "새-패스워드"
```

> 키 교체 후 첫 릴리스를 받은 사용자는 자동 업데이트가 통하지만, 그 이전 버전 사용자는 수동 재설치가 필요할 수 있어요. README에 안내 문구를 추가해 주세요.

### 5.2 인증서 재발급

- **Windows OV**: 발급 기관(DigiCert / SSL.com 등)에서 revoke + 재발급. 평판 빌드는 처음부터 다시 시작해요.
- **Apple Developer ID**: developer.apple.com 콘솔에서 revoke + 재발급. notarization 이력은 그대로 유지돼요.

---

## 6. 한 번에 끝내는 명령 (정상 케이스)

```bash
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/lmmaster.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --body "real-password"
# 추후 (선택): GlitchTip DSN.
# gh secret set LMMASTER_GLITCHTIP_DSN --body "https://key@host/1"
```

이후 `git tag v0.1.0 && git push --tags` 하면 `release.yml`이 자동으로 빌드 + GitHub Release(draft)를 작성해요. 사용자가 release 페이지에서 publish 버튼만 누르면 정식 공개돼요.

---

> **주의**: secret 값은 `gh secret list`로도 노출되지 않아요. 잊어버리면 재등록만 가능해요. CI 로그에도 마스킹 처리(`***`)돼요. 만약 실수로 로그에 출력하면 *secret을 즉시 폐기*해 주세요.

# LMmaster — GitHub 삭제 스탠바이 노트

> 작성: 2026-05-10 | GitHub 저장소(`freessunky-bit/lmmaster`) 삭제 전 기록.
> 재활성화 시 본 문서를 참고해 단계별로 복원하면 돼요.

---

## 삭제 후 오프되는 기능

| 기능 | 의존 | 앱 동작 영향 |
|---|---|---|
| **자동 업데이트 알림** | `api.github.com/repos/freessunky-bit/lmmaster/releases/latest` | 새 버전 알림 안 옴 (기존 기능 100% 정상) |
| **릴리즈 설치파일 배포** | GitHub Releases (exe/dmg/AppImage) | 새 버전 배포 불가 |
| **카탈로그 갱신** | `cdn.jsdelivr.net` → `manifests/apps/catalog.json` | 새 모델 추가 안 됨 (기존 설치 모델 정상) |
| **카탈로그 서명 검증** | `sign-catalog.yml` GHA + minisig | 서명 파일 자동 갱신 중단 |
| **빌드 자동화** | GitHub Actions (Release + CI workflow) | 로컬 빌드로 대체해야 함 |

**핵심**: 이미 설치된 앱의 **채팅·모델 실행·원격 연결·API 키 등 모든 로컬 기능은 영향 없음.**

---

## 재활성화 체크리스트

GitHub에 새 저장소를 만들고 push하면 아래 순서로 재활성화돼요.

### Step 1 — 저장소 재생성
```bash
# 로컬 폴더는 그대로 보존됨. remote만 새로 연결.
git remote set-url origin https://github.com/<새-계정>/lmmaster.git
git push -u origin main
```

### Step 2 — GitHub Actions Secrets 재등록
CI/CD 워크플로우가 필요로 하는 시크릿 확인:
- `.github/workflows/` 하위 yml 파일에서 `secrets.` 키워드 검색
- 필요 시 새 저장소 Settings > Secrets에 재등록

### Step 3 — 카탈로그 CDN 경로 업데이트
`apps/desktop/src-tauri/src/commands.rs` 또는 `crates/registry-fetcher/` 에서
현재 jsdelivr URL이 `freessunky-bit/lmmaster` 레포를 가리키는 부분 →
새 레포 주소로 교체 후 재빌드.

### Step 4 — 자동 업데이트 repo 주소 업데이트
`apps/desktop/src/pages/Settings.tsx` 상단:
```ts
const UPDATE_REPO = "freessunky-bit/lmmaster";  // ← 새 레포로 교체
```

### Step 5 — 첫 릴리즈 태그
```bash
git tag v0.7.9   # 또는 다음 버전
git push origin v0.7.9
# GitHub Actions가 자동으로 exe/dmg/AppImage 빌드 후 릴리즈 업로드
```

---

## 현재 배포 버전 현황 (삭제 시점)

| 버전 | 주요 내용 |
|---|---|
| v0.7.9 | HF 토큰 인증, HCX-SEED 401 수정, RP 시스템 프롬프트, 버전 표시 수정 |
| v0.7.8 | RP 모델 시스템 프롬프트 템플릿 구체화 |
| v0.7.7 | 채팅 시스템 프롬프트 입력창, Stheno NSFW 안내 개선 |
| v0.7.5 | LAN 원격 연결 기능 |
| v0.7.4 | 회수 키 삭제, API 연결 가이드, Stheno 드로어 안내 |

---

## 로컬 빌드 방법 (GitHub 없이)

```powershell
# 프론트엔드 빌드
cd apps/desktop
pnpm install
pnpm run build

# Tauri 앱 빌드 (Windows exe 생성)
pnpm run tauri build
# 결과물: apps/desktop/src-tauri/target/release/bundle/
```

---

*재활성화 시 이 문서를 삭제하거나 완료 항목에 체크 표시 하세요.*

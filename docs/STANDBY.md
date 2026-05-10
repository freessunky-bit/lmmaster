# LMmaster — GitHub 삭제 스탠바이 노트

> 작성: 2026-05-10 | 저장소 `freessunky-bit/lmmaster` 삭제 전 전수 조사 기록.
> 로컬 폴더(`C:\Users\wind.WIND-PC\Desktop\VVCODE\LMmaster`)는 보존됨.

---

## 삭제 후 오프되는 기능

| 기능 | 의존 경로 | 앱 영향 |
|---|---|---|
| **자동 업데이트 알림** | `api.github.com/repos/freessunky-bit/lmmaster/releases` | 새 버전 알림 안 옴 (기존 기능 100% 정상) |
| **릴리즈 설치파일 배포** | GitHub Releases (exe / dmg / AppImage) | 새 버전 배포 불가 |
| **카탈로그 갱신** | `cdn.jsdelivr.net/gh/freessunky-bit/lmmaster@…` | 새 모델 추가 안 됨 (기존 설치 모델 정상) |
| **카탈로그 서명 검증** | sign-catalog GitHub Actions + minisig | 서명 갱신 중단 |
| **큐레이션 Issue 자동 생성** | hf_search.rs / trending-watcher → GitHub Issue | HF 모델 등록 요청 링크 깨짐 |
| **빌드 자동화 (CI/CD)** | `.github/workflows/` Release + CI | 로컬 빌드로 대체 필요 |

**핵심**: 설치된 앱의 채팅·모델·원격 연결·API 키 등 **모든 로컬 기능은 영향 없음**.

---

## 재활성화 시 수정해야 할 파일 목록

새 저장소 주소를 `<새계정>/<새레포>`로 교체.

### 1. 자동 업데이트 설정 (2곳)

**`apps/desktop/src/pages/Settings.tsx`**
```ts
// 라인 92
const UPDATE_REPO = "freessunky-bit/lmmaster";        // ← 교체
// 라인 94
const UPDATE_REPO_BETA = "freessunky-bit/lmmaster-beta"; // ← 교체
// 라인 100
const RELEASES_URL = "https://github.com/freessunky-bit/lmmaster/releases/latest"; // ← 교체
```

**`apps/desktop/src-tauri/tauri.conf.json`**
```json
// 라인 118-119
"endpoints": [
  "https://github.com/freessunky-bit/lmmaster/releases/latest/download/latest.json",       // ← 교체
  "https://github.com/freessunky-bit/lmmaster/releases/download/beta/latest-beta.json"     // ← 교체
]
```

### 2. 카탈로그 CDN 소스 (핵심)

**`crates/registry-fetcher/src/source.rs`**
```rust
// 라인 112 — jsdelivr URL
"https://cdn.jsdelivr.net/gh/freessunky-bit/lmmaster@{jsdelivr_ref}/manifests/apps/{id}.json"
// ↑ freessunky-bit/lmmaster → <새계정>/<새레포>

// 라인 119 — GitHub Releases fallback URL
"https://github.com/freessunky-bit/lmmaster/releases/download/manifests-{github_tag}/{id}.json"
// ↑ 동일하게 교체
```
> 교체 후 `cargo check --workspace` 실행 필수.

### 3. HuggingFace 모델 큐레이션 요청 링크 (2곳)

**`apps/desktop/src/ipc/hf_search.ts`** (라인 69)
```ts
`https://github.com/freessunky-bit/lmmaster/issues/new?${params}`
// ↑ 교체
```

**`apps/desktop/src-tauri/src/hf_search.rs`** (라인 182)
```rust
"https://github.com/freessunky-bit/lmmaster/issues/new?template=curation-request.yml&..."
// ↑ 교체
```

### 4. GitHub Actions Workflows

**`.github/workflows/`** 하위 파일들을 점검:
- `release.yml` — 릴리즈 빌드 (변경 불필요, 자동으로 현재 repo 인식)
- `sign-catalog.yml` — 카탈로그 서명
- `trending-watcher.yml` — deprecated (코멘트 참고)
- `trends-bundle-curator.yml` — 트렌드 큐레이션

**Secrets 재등록 필요** (저장소 Settings > Secrets > Actions):

| Secret 이름 | 용도 | 확인 방법 |
|---|---|---|
| `MINISIGN_SECRET_KEY` | 카탈로그 서명 | `sign-catalog.yml` 참조 |
| `MINISIGN_PASSWORD` | 서명 키 비밀번호 | 동일 |
| (기타) | CI/CD | `.github/workflows/*.yml`에서 `secrets.` 키워드 검색 |

### 5. 큐레이터 담당자 (GitHub username 교체)

**`crates/trending-watcher/src/report.rs`** (라인 28)
```rust
"assignees: freessunky-bit\n"  // ← 새 GitHub 계정으로 교체
```

**`crates/trends-bundle-curator/src/main.rs`** (라인 81)
```rust
"assignees: freessunky-bit\n"  // ← 동일
```

### 6. capabilities 화이트리스트 (변경 불필요)

`apps/desktop/src-tauri/capabilities/main.json`의 `shell:allow-open` URL 목록:
- `https://github.com/**` ← 이미 와일드카드라 repo 교체해도 자동 적용됨

---

## 재활성화 절차 (순서)

```bash
# 1. 새 GitHub 저장소 생성 후 remote 교체
git remote set-url origin https://github.com/<새계정>/<새레포>.git
git push -u origin main --tags

# 2. 위 파일들 일괄 치환 (예: VS Code 전체 파일 검색/교체)
#    "freessunky-bit/lmmaster" → "<새계정>/<새레포>"
#    "freessunky-bit/lmmaster-beta" → "<새계정>/<새레포>-beta"  (있으면)

# 3. 빌드 검증
cargo check --workspace
pnpm exec tsc -b

# 4. 커밋 + 태그 → CI가 자동으로 릴리즈 빌드
git add -A
git commit -m "chore: 새 저장소 주소로 URL 교체"
git tag v0.7.9   # 또는 다음 버전
git push origin main v0.7.9

# 5. GitHub Actions Secrets 재등록 후 sign-catalog.yml 실행 확인
```

---

## 로컬 빌드 (GitHub 없이)

```powershell
# Windows exe 빌드
cd apps/desktop
pnpm install
pnpm run tauri build
# 결과: apps/desktop/src-tauri/target/release/bundle/msi/ 또는 /nsis/
```

---

## 현재 배포 버전 현황 (삭제 시점: 2026-05-10)

| 버전 | 주요 내용 |
|---|---|
| **v0.7.9** | HF 토큰 인증 지원, HCX-SEED 401 수정, RP 시스템 프롬프트 |
| v0.7.8 | RP 모델 시스템 프롬프트 템플릿 구체화 |
| v0.7.7 | 채팅 시스템 프롬프트 입력창, Stheno 안내 개선 |
| v0.7.6 | 버전 표시(0.1.0 → 실제 버전), 가이드 섹션 CRLF 수정 |
| v0.7.5 | LAN 원격 연결 기능 전체 구현 |
| v0.7.4 | 회수 키 삭제, API 연결 JS 예시, Stheno 드로어 안내 |
| v0.7.3 | 가이드 UX, llama-server 인라인 설치, 채팅 비활성 수정 |

---

*재활성화 완료 시 본 문서 삭제 또는 `## 완료` 섹션으로 이동.*

# ADR-0017: Pinokio-style manifest + tauri-plugin-shell 기반 외부 앱 자동 설치

- Status: Accepted
- Date: 2026-04-26

## Context
ADR-0016이 LM Studio + Ollama를 1순위 백엔드로 결정했다. 사용자 요구는 "거의 자동에 가까운 한국어 셋업". 따라서 우리는 (1) 외부 앱 detect (2) 미설치 시 silent/안내 install (3) 주기적 update detect를 책임진다. 보강 리서치(§2)는 Pinokio가 declarative install 매니페스트의 ergonomics를, Tauri 2 `tauri-plugin-shell`이 타협 없는 권한 모델을 제공함을 확인했다.

## Decision

### 1. 매니페스트 포맷
`manifests/apps/<app>.json`을 Pinokio-style declarative로 작성한다. 각 앱은 4가지 액션 정의:

```json
{
  "id": "ollama",
  "display_name": "Ollama",
  "license": "MIT",
  "redistribution_allowed": true,
  "detect": [
    { "method": "http.get", "url": "http://127.0.0.1:11434/api/version", "timeout_ms": 1500 },
    { "method": "shell.which", "cmd": "ollama" }
  ],
  "install": {
    "windows": {
      "method": "download_and_run",
      "url": "https://github.com/ollama/ollama/releases/latest/download/OllamaSetup.exe",
      "sha256_url": "https://github.com/ollama/ollama/releases/latest/download/OllamaSetup.exe.sha256",
      "args": ["/SILENT"]
    },
    "macos": { "method": "download_and_run", "url": "...", "args": [] },
    "linux": { "method": "shell.run", "cmd": "curl -fsSL https://ollama.com/install.sh | sh" }
  },
  "update": {
    "source": { "type": "github_release", "repo": "ollama/ollama" },
    "trigger": { "method": "open_url", "url": "https://ollama.com/download" }
  }
}
```

LM Studio 매니페스트는 `redistribution_allowed: false`로 표시하고, install 액션은 **공식 다운로드 페이지로 한국어 안내**만 한다 — silent install 금지.

### 2. Rust crate 구조
- `crates/runtime-detector` — `detect`. HTTP probe + 레지스트리/plist/dpkg fallback.
- `crates/installer` — `install`/`update`. `tauri-plugin-http`로 `app_cache_dir`에 다운로드 → SHA256 검증 → `tauri-plugin-shell`로 silent flag spawn. 재배포 금지 앱은 `open_url`로 안내.
- `crates/updater` — 6~24h 폴러: GitHub releases (Ollama), LM Studio `/api/latest-version`, 우리 모델 manifest. 비차단 한국어 토스트.

### 3. Tauri capability ACL
`src-tauri/capabilities/main.json`에 명시 추가:
```json
"permissions": [
  "core:default",
  { "identifier": "shell:allow-execute",
    "allow": [
      { "name": "ollama-installer", "cmd": "OllamaSetup.exe", "args": ["/SILENT"], "sidecar": false },
      { "name": "lms-cli", "cmd": "lms", "args": [{"validator":"^[\\w\\-:= /\\.]+$"}], "sidecar": false }
    ]
  },
  "http:default",
  "updater:default"
]
```
와일드카드 exec 금지. 인자 정규식 validator 강제. shell:allow-execute는 매니페스트에 등록된 명령만.

### 4. 한국어 UX
- 첫실행 마법사: 4단계 stepper (감지 / Ollama 설치 / LM Studio 안내 / 추천 모델).
- 업데이트 토스트: "새 버전 LM Studio 0.4.x가 출시되었어요. 설치하시겠어요? [설치] [나중에]".
- 사인된 third-party installer는 OS UAC가 그쪽 cert로 뜨므로 우리가 재사인 금지.

### 5. 코드 사인
- 우리 본체: EV 코드 사인 (Win), Apple Developer ID + notarization (mac).
- `tauri-plugin-updater`: `tauri signer generate` 키페어, 공개키 `tauri.conf.json`, 비공개키 CI secret.
- 외부 vendor installer는 그쪽 사인 그대로 호출.

## Consequences
- 매니페스트 추가만으로 새 외부 앱(향후 GPT4All, AnythingLLM 등) 통합 가능.
- 권한 ACL이 strict — 임의 명령 실행 차단.
- LM Studio EULA 위배 위험 0(자동 install 안 함).
- Tauri 2의 admin elevation 미지원(issue #7173) → 시스템 전역 설치 강제 시 vendor installer가 자체 UAC trigger. 우리는 per-user 우선.

## Alternatives considered
- **자체 Rust 설치 로직 하드코딩**: declarative 보다 유지보수 비용 ↑. 거부.
- **Pinokio 그대로 사용**: per-app venv 디스크 폭증, portability 0. 거부 — 패턴만 차용.
- **Ansible/Chocolatey/Homebrew wrapping**: 사용자에게 추가 도구 강요. 거부.

## References
- `docs/research/pivot-reinforcement.md` §2
- pinokiocomputer/pinokio, docs.pinokio.computer/api/datastructure
- v2.tauri.app/plugin/shell, /plugin/updater, /plugin/http
- v2.tauri.app/distribute/sign/windows
- ADR-0016 (Wrap-not-replace)

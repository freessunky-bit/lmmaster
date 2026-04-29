# manifests/snapshot

> Bundled fallback fixture — `crates/registry-fetcher`의 4-tier fallback 마지막 단계 (Tier: Bundled).
> 사용자 PC가 오프라인 + 캐시 24h 초과 + 네트워크 미러 모두 실패 시 본 디렉터리에서 매니페스트를 읽어 마지막 안전망으로 사용해요.

## 정책 (ADR-0017)

- 본 디렉터리는 **manifests/apps/*.json의 stale-but-known-good snapshot**.
- CI가 manifests/apps에서 자동 갱신 — 본 README와 함께 동일한 조 단위로 commit.
- **Tauri bundle.resources에 포함**되어 빌드 시 앱과 함께 배포 (`apps/desktop/src-tauri/tauri.conf.json`).
- v1: ollama.json + lm-studio.json + `models/{agents,coding,roleplay,slm,sound-stt}/*.json` (Phase 2'.a 시드 8종).
- 모델 매니페스트의 `quantization_options[].sha256`은 v1 시드에선 placeholder (64 zeros). Phase 6' 자동 갱신에서 실제 hash로 교체.

## 갱신 절차

```powershell
# 수동 — 새 매니페스트 추가/수정 시.
Copy-Item -Path manifests\apps\*.json -Destination manifests\snapshot -Force
git add manifests/snapshot
git commit -m "chore(manifests): refresh snapshot from manifests/apps"
```

CI 자동 갱신은 별도 워크플로(`.github/workflows/manifests-snapshot.yml`, 후순위)에서.

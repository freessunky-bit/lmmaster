# ADR-0009: Portable workspace는 manifest 기반 (단일폴더 환상 금지)

- Status: Accepted
- Date: 2026-04-26

## Context
사용자는 "USB에 꽂아서 어디서나" 같은 portable 환상을 자주 가진다. 그러나 실제로는 (1) 다른 OS(Win→mac), (2) 다른 GPU/드라이버, (3) 다른 RAM/디스크에서 동일 폴더가 즉시 동작할 수 없다.

## Decision
Workspace는 **manifest 파일**(`workspace/manifest.json`)로 정의한다. 모든 경로는 워크스페이스 루트 기준 상대경로. 다음을 manifest에 기록:
- workspace_id (uuid)
- host_fingerprint: { os, arch, gpu, vram, ram, cpu }
- runtimes_installed: [{ id, version, build_target }]
- models_installed: [{ id, runtime_id, quant, file_rel_path, sha256, size }]
- ports: { gateway, ml_worker? }
- created_at, last_repaired_at

표준 디렉터리:
```
workspace/
  app/         # 앱 바이너리(설치 시 채움, 사용자 수정 금지)
  data/        # SQLite, 사용자 설정
  models/      # 모델 파일
  cache/       # 임시/재생성 가능
  runtimes/    # llama.cpp, koboldcpp 등 OS·target별 prebuilt
  manifests/   # registry cache, model card 캐시
  logs/        # 회전 로그
  projects/    # 프로젝트 바인딩 메타
  sdk/         # SDK 예제 / 자동 생성 키 정보 viewer
  docs/        # 한국어 가이드 사본
  exports/     # 향후 양자화/변환 결과
```

이동 정책:
- **같은 OS/아키텍처/GPU 계열**: 그대로 동작.
- **fingerprint mismatch**: 첫 실행 시 자동 감지 → "환경이 변경되었습니다" 화면 → repair flow(필요 시 런타임 재다운로드, 모델 호환성 점검) 진입.
- **cross-OS**: 명시적 경고 + 재설치 가이드. 모델 파일은 그대로 사용 가능(GGUF는 OS-agnostic).

## Consequences
- 설치 위치 자유, 다른 PC로 옮겼을 때 가이드된 복구.
- SQLite 파일은 OS-agnostic이라 그대로 이동 가능.
- 런타임 바이너리는 host_fingerprint에 따라 재다운로드 필요할 수 있음.
- 사용자 멘탈모델: "프로젝트 폴더처럼 다루세요. 환경이 바뀌면 한 번 점검해드립니다."

## Alternatives considered
- **단일 폴더에 모든 OS 바이너리 동봉**: 디스크 폭증, 라이선스/서명 복잡. 거부.
- **OS 표준 위치만 사용 (AppData/Application Support)**: portable 요구와 충돌. 거부.

## References
- ADR-0008 (SQLite)

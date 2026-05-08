# Phase 13'.h.2.c.2 — LlamaCpp 모델 자동 다운로드 결정 노트

> 작성: 2026-05-08 (v0.4.0 직후, v0.4.2 진입). Phase 13'.h.2.d Round 1~4 다음 sub-phase.

## 1. 결정 요약

LlamaCpp 사용자도 카탈로그 "모델 받기" 한 번 클릭으로 **GGUF + mmproj가 cache_dir에 자동 배치**되도록 wiring한다. 신규 IPC 없이 기존 `start_model_pull`에 `RuntimeKind::LlamaCpp` 분기를 추가하고, `installer::Downloader` (sha256 + atomic + backon + .partial resume)를 그대로 재사용한다.

## 2. 채택안

- **IPC 진입점**: `start_model_pull(model_id, runtime_kind=LlamaCpp, channel)` 분기 추가 — Ollama 분기와 frontend 동등 인터페이스. `ModelPullEvent` 그대로 emit (Status/Progress/Completed/Cancelled/Failed).
- **download stack 재사용**: `installer::Downloader` 인스턴스화 → `download(DownloadRequest, cancel, ProgressSink)`. `.no_proxy()`, sha256 stream 검증, .partial resume, backon retry 모두 자동.
- **cache_dir**: `app_local_data_dir().join("models")`. `create_dir_all`로 idempotent mkdir.
- **메인 GGUF URL**: `ModelEntry::source` + 선택된 `QuantOption`로 derive.
  - `HuggingFace { repo, file }` + `quant.file_path` → `https://huggingface.co/{repo}/resolve/main/{file_path}`.
  - `DirectUrl { url }` → 그대로.
- **메인 파일명**: 다운로드 URL의 basename. `chat::llama_cpp::build_server_spec`도 동일 함수로 path 결정해 자동 다운로드 ↔ chat 진입 일관.
- **mmproj 다운로드**: `entry.mmproj.is_some()`이면 메인 다운로드 후 추가 다운로드. `derive_mmproj_filename` (URL basename + `<id>-mmproj.gguf` fallback)는 v0.4.0과 동일.
- **quant 선택 기본 정책**: 사용자가 명시 quant 미지정 시 `entry.quantization_options.first()` (보통 Q4_K_M). 명시 quant은 v0.5.x frontend wizard에서 (Phase 13'.h.2.e).
- **DownloadEvent → ModelPullEvent 변환**:
  - `DownloadEvent::Started { url, total, .. }` → `Status { status: "받기 시작: {basename} ({size}MB)" }`.
  - `DownloadEvent::Progress { downloaded, total, speed_bps }` → `Progress { completed_bytes, total_bytes, speed_bps, eta_secs }`.
  - `DownloadEvent::Verified { .. }` → `Status { status: "sha256 검증 완료" }`.
  - `DownloadEvent::Finished { .. }` → `Status { status: "저장 완료" }`.
  - `DownloadEvent::Retrying { .. }` → `Status { status: "재시도 중 — {reason}" }`.

## 3. 기각안 + 이유

- **별도 `start_llama_pull` IPC 신설**: frontend가 runtime별 분기 추가 부담 + IPC 표면 증식. 기각 — `start_model_pull`의 runtime branch가 Ollama와 동질 패턴.
- **Ollama hub 통과 LlamaCpp 활용** (Ollama가 이미 받은 blob 재사용): Ollama가 GGUF를 *내부 hash 형식*으로 저장 (manifest layer + sha256 directory). 사용자 GGUF 경로 추출은 Ollama 버전마다 변경 위험 + 경로 수동 매핑 부담. 기각.
- **Range request resumable + UI resume 버튼**: `Downloader`가 이미 `.partial` resume 지원. UI 측 명시 resume 버튼은 v1.x 사용자 피드백 후 도입. 기각 (현재 v1).
- **Multi-quant 동시 다운로드** (Q4_K_M + Q5_K_M 둘 다): 디스크 / 대역폭 부담 + 사용자 의도 모호. 기각 — 사용자 명시 선택만.
- **`<id>.gguf` 강제 통일**: build_server_spec 시 단순하지만 *동일 모델 multi-quant*를 한 cache_dir에 못 둠 (덮어쓰기). 기각 — basename derivation으로 유연.

## 4. 미정 / 후순위

- **Quant 선택 UI** (frontend): Phase 13'.h.2.e 마법사가 다룸. v0.4.2은 default first quant.
- **Catalog hot-reload 시 cache 정리**: 새 catalog가 동일 model_id의 quant URL을 변경하면 옛 파일 stale. v1.x — 사용자 cache 관리 UI.
- **mmproj sha256 누락 entry**: 현재 catalog의 mmproj.sha256은 일부 None. None일 때 검증 skip + 사용자 한국어 경고는 v1 미적용 (커뮤니티 sha256 백필 후 강제).
- **국가별 mirror / 서버 selector**: HF 외 추가 mirror는 정책 결정 없음. v1.x.

## 5. 테스트 invariant

- `derive_main_url(entry, quant)` — HuggingFace `{repo}/resolve/main/{file_path}` + DirectUrl 그대로 + 빈 file_path fallback.
- `derive_main_filename(entry, quant)` — URL basename + 빈 basename fallback `<id>.gguf`.
- `build_server_spec`의 model_path가 `derive_main_filename`과 일치 (chat 진입 ↔ 자동 다운로드 round-trip).
- mmproj `None`일 때 다운로드 skip 검증.
- `parse_sha256_hex` — 64-hex → `[u8; 32]`, 잘못된 길이 / 비-hex → 한국어 에러.
- `ProgressSink` 변환 closure: `DownloadEvent::Progress` → `ModelPullEvent::Progress` 필드 매핑.
- cancel cascade: chat IPC R-E.6 패턴과 동일 — Channel close → cancel.

## 6. 다음 페이즈 인계

- **Phase 13'.h.2.e** (LlamaCppSetupWizard): env 등록 UI + quant 선택 UI + 다운로드 진행 표시.
- **Phase 13'.h.5** (known_issues 카탈로그 마커): vision_support=true & mmproj=None 모델은 카탈로그 UI에 한국어 경고.

진입 조건:
- 본 sub-phase 종결 = `start_model_pull(LlamaCpp, ...)` 호출 시 GGUF + mmproj 자동 다운로드 + cache_dir 배치.
- 후속 마법사가 `start_model_pull` 호출 + progress 표시만 하면 됨.

위험 노트:
- HF resolve URL은 redirect chain 발생 가능 — `reqwest::Client::redirect(Policy::limited(20))`이 기본. 현 client는 reqwest default (10회) — 일반적으로 충분.
- 7B Q4_K_M ≈ 4.5GB, 30B Q4_K_M ≈ 18GB. 디스크 사전 체크는 v0.4.2 미적용 — Downloader는 free space 부족 시 OS 에러 한국어 매핑.

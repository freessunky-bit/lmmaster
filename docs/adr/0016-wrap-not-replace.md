# ADR-0016: Wrap-not-replace — LM Studio + Ollama을 v1 primary backend로 채택

- Status: Accepted
- Date: 2026-04-26
- Supersedes: ADR-0005 (llama.cpp primary)

## Context
GPT 외부 검토와 4영역 보강 리서치(`docs/research/pivot-reinforcement.md`) 결과:
- 사용자가 "로컬 LLM을 쓰기만" 하려는 영역에선 LM Studio · Ollama가 이미 충분히 성숙.
- 우리의 객관적 우위 지점은 **자체 런타임이 아니라 제품화된 오케스트레이션 레이어**(설치 자동화 + 한국어 UX + 포터블 + 카테고리 큐레이션 + 워크벤치 + 자동 갱신).
- LM Studio는 free-for-work 라이선스로 상업/사내 사용 자유, REST/CLI 풍부, 단 installer 재배포 금지.
- Ollama는 MIT, HTTP API 완전 통제, silent install 가능.
- 자체 llama.cpp 자식 프로세스를 default로 두는 것은 **이중 작업 + QA 부담**.

## Decision
v1 primary backend는 **LM Studio + Ollama 어댑터(HTTP attach)** 두 개로 한다.

- **OllamaAdapter (1순위)**: `http://127.0.0.1:11434`. 미설치 시 우리가 silent install (MIT, 재배포 가능).
- **LMStudioAdapter (1순위)**: `http://127.0.0.1:1234` + 보조 `lms` CLI. 미설치 시 사용자에게 공식 설치 페이지를 한국어로 안내(EULA 상 자동 재배포 금지). `lms daemon up` 헤드리스 실행을 우리가 트리거 가능.
- **LlamaCppAdapter**: v1.x로 격하. 둘 다 설치하기 어려운 환경의 zero-config 옵션. 자체 spawn은 워크벤치 quantization 경로(`llama-quantize`)에서만 v1 활용.
- **KoboldCppAdapter / VllmAdapter**: v2 이후로 미룸.

Gateway는 기존 OpenAI-compatible REST(ADR-0006) 그대로 노출하고, 내부적으로 모델 → 백엔드 라우팅을 한다.

## Consequences
- **개발 리소스 절감**: 자체 런타임 빌드/사인/플랫폼 매트릭스 부담 제거. 대신 (1) 자동 설치 (2) 큐레이션 (3) 워크벤치에 집중.
- **백엔드 직렬화 필요**: GPU contention 회피 위해 한 번에 한 백엔드만 active. `keep_alive`(Ollama) + idle TTL(LM Studio)을 사용자에게 노출.
- **저장소 중복 가능성**: 같은 GGUF가 LM Studio path와 Ollama blob store에 양립. SHA256 detect + 사용자 경고. 자동 symlink 금지(Ollama blob-store invariant).
- **EULA 준수**: LM Studio installer 재배포 절대 금지. 공식 사이트 링크만.
- **업데이트 책임 축소**: LM Studio/Ollama는 자체 updater 보유 — 우리는 detect + 알림 + 사용자 동의 후 trigger만.
- **포터블 워크스페이스의 의미 변화**: 백엔드는 OS-level 설치 → 우리 워크스페이스 폴더에 포함되지 않음. 다른 PC로 이동 시 백엔드 detect → 미설치면 재설치 마법사. 모델 파일은 우리 manifest에 sha256으로 기록 → 재다운로드 가능.

## Alternatives considered
- **llama.cpp 자체 spawn 유지(원래 ADR-0005)**: 거부 — 이중 비용, 사용자 가치 낮음.
- **Ollama 하나로 충분**: 거부 — LM Studio가 GUI 사용자에게 더 친숙, 두 진영 모두 커버 시 시장 적합도 ↑.
- **자체 추론 엔진 작성**: 절대 거부 (ADR-α 시점부터 일관).

## References
- `docs/PIVOT.md`
- `docs/research/pivot-reinforcement.md`
- LM Studio docs: lmstudio.ai/docs/{cli,app/api/endpoints/rest,app-terms}
- Ollama: github.com/ollama/ollama/blob/main/docs/api.md
- ADR-0001 (Companion 구조 — 유지)
- ADR-0004 (Adapter pattern — 유지, 우선순위만 재배치)
- ADR-0017 (Manifest+Installer)
- ADR-0018 (Workbench v1 core)

# ADR-0019: Always-latest hybrid bootstrap + 자동 업그레이드 정책

- Status: Accepted
- Date: 2026-04-26

## Context
사용자가 명시한 v1 약속:
- 아무것도 모르고 앱을 시작해도 **최신 시점**의 모델/프로그램/권장 환경/설치 옵션이 즉시 적용.
- 이후 주기 자가스캔으로 신모델/신버전 발견 → 가능한 항목은 자동 업그레이드.

리서치(Phase 1' §2)로 확인된 사실:
- VS Code, GitHub Desktop, Cursor, JetBrains Toolbox 모두 **hybrid 패턴**(번들된 snapshot + 비차단 원격 fetch + cache 폴백)을 사용.
- Discord의 pure-remote 차단 패턴은 한국 기업망(KT/SKB 간헐, 사내 proxy)에서 사용자 좌절 야기.
- GitHub Releases는 anonymous 60/h 한도지만 ETag 기반 304는 cap에 포함되지 않음 — 6h 폴링이면 충분.
- HuggingFace `/api/models?sort=trending_score`는 anon 500/5min, 카테고리 필터 가능.
- tauri-plugin-updater는 ETag native 미지원 → 수동 헤더 추가 필요.

## Decision

### 1. 첫 실행 부팅 (Hybrid Pattern C)
빌드 시 `manifests/snapshot/` 디렉터리에 모든 외부 앱·모델 카탈로그의 **stale-but-known-good snapshot**을 번들. 첫 실행 시:

```
firstRun cascade:
  1. cache <= TTL(1h)?           → use cache. async background refresh.
  2. remote 4-tier 병렬 폴백:
       P1: vendor API           timeout 2s   (e.g. ollama.com, lmstudio.ai/api/latest-version, HF Hub)
       P2: GitHub Releases       timeout 3s   (anon + ETag)
       P3: jsdelivr CDN mirror   timeout 3s   (rate-limit 회피)
       P4: bundled snapshot      always succeeds
     → 첫 성공 결과 사용 + cache+ETag 저장. 2s soft deadline.
  3. cache (stale)?              → use stale + UI 뱃지 "확인 중…"
  4. bundled snapshot            → use + UI 뱃지 "오프라인 표시 중"
```

UI는 어느 단계에서도 차단 안 됨. snapshot은 빌드 파이프라인에서 자동 갱신.

### 2. 멀티-소스 버전 lookup
각 외부 앱(LM Studio / Ollama / 본체)에 대해 위 4-tier 동시 시도. 우선순위:
- LM Studio: lmstudio.ai/api/latest-version (P1) ‖ github lmstudio-ai/lms releases (P2) → jsdelivr (P3) → bundled.
- Ollama: ollama.com (P1) ‖ github ollama/ollama releases (P2) → jsdelivr (P3) → bundled.
- 본체: tauri-plugin-updater 다중 endpoint(우리 CDN ‖ GitHub releases) → bundled.

### 3. 폴링 cadence + ETag
- on-launch + 6h interval (`tokio-cron-scheduler`).
- 모든 GET에 `If-None-Match: <stored-etag>` + `If-Modified-Since` 강제. 304는 free.
- HF trending: 1h cache + ETag.
- OpenRouter / Ollama library scrape: 1h cache.
- 403 rate-limit 시 자동으로 jsdelivr fallback.

### 4. 자동 업그레이드 UX (JetBrains Toolbox 모델)
- 자동 체크 (default ON, 설정 토글 가능).
- 다운로드: 백그라운드 (사용자 작업 차단 0).
- 적용: **다음 실행 시** (한국어 토스트 "새 버전 준비됨 · 다음 실행 때 적용돼요"). 보조 버튼 "지금 재시작".
- LM Studio / Ollama: 자체 updater 보유 → 우리는 detect + 한국어 알림 + 사용자 동의 후 본인 installer trigger.
- 우리 본체: tauri-plugin-updater (Ed25519 sign verification, 다중 endpoint failover).

### 5. Trending 모델 발견 (자가스캔의 한 axis)
- HuggingFace `/api/models?sort=trending_score&limit=N&pipeline_tag=<tag>` — 카테고리(text-generation/conversational/etc.)별 trending top-N.
- OpenRouter `/api/v1/models` — 개발자 인기 신호.
- Ollama library: scraping 위험 → GitHub `ollama/ollama` registry mirror 우선.
- 모두 1h cache + 1차 신뢰는 우리 curated `manifests/models/index.json`.

### 6. 빌드 파이프라인 책임
CI에서:
- 매일 1회 manifest snapshot 생성 (latest LM Studio/Ollama 버전 + curated 모델 카탈로그) → repo `manifests/snapshot/`에 PR.
- 릴리스 빌드는 마지막 snapshot 포함.

## Consequences
- **오프라인에서도 첫 실행 가능** (bundled snapshot).
- **온라인 첫 실행은 거의 항상 최신** (4-tier remote fallback).
- **Korean 기업망 친화** (proxy/CDN 차단 시에도 jsdelivr 또는 bundled).
- **GitHub rate-limit 안전** (ETag + 6h cadence + jsdelivr).
- **UI 차단 0** — 모든 fetch는 비동기.
- snapshot bundle 크기 ~수십~수백 KB, 무시 가능.
- bundled snapshot stale 위험 → CI 자동 갱신 필수.

## Alternatives considered
- **Pure remote (Discord 모델)**: Korean 기업망에 부적합. 거부.
- **Pure bundled (Cursor 일부 패턴)**: 사용자 약속 "최신 시점" 위배. 거부.
- **PAT 동봉 GitHub polling**: 보안 위험 + ToS 위반. 거부 — anonymous + ETag로 충분.

## References
- `docs/research/phase-1-reinforcement.md` §2
- VS Code, GitHub Desktop, Cursor, JetBrains Toolbox 2.0 update model
- v2.tauri.app/plugin/updater
- docs.github.com/rest/using-the-rest-api/{rate-limits,best-practices}
- huggingface.co/docs/hub/{rate-limits,api}
- ADR-0014 (curated registry — 보강)
- ADR-0020 (자가스캔)

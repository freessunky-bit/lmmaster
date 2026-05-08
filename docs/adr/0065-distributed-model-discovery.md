# ADR-0065: 사용자 측 분산 모델 발견 (운영자 큐레이션 정책 변경)

> **상태**: 채택 (2026-05-09).
> **Supersedes**: ADR-0044 라이브 카탈로그 갱신 정책의 *운영자 중앙 큐레이션* 부분.
> **Phase**: 22'.h.

## 1. 결정 요약

운영자(실장님) 중앙 큐레이션 부담을 폐기하고, **각 사용자 PC에서 HF Trending API 자동 fetch + 사용자 자율 1-click 추가** 구조로 전환한다. 신규 모델 발견은 *발견자 = 사용자*. 운영자는 *기본 카탈로그* (현재 manifests/snapshot/)만 minimal 유지.

## 2. 채택안

### 2.1 사용자 측 자율 발견 흐름

```
[자동, 사용자 PC, 24h cron]
  HF Trending API fetch (top N + downloads / likes / created_at 메트릭)
    ↓
[자동]
  자동 라벨 생성:
    - 라이선스 카테고리 (Apache-2 / MIT / CC-BY-NC / Llama Community / 기타)
    - 한국어 키워드 detect (model card에 "한국어" / "Korean" 포함 여부)
    - NSFW marker (model id / tags / description heuristic)
    - VRAM 추정 (model size + quant 추정)
    - 신뢰 점수 (downloads * likes * 작성자 verified_by_hf)
    ↓
[자동]
  현재 사용자 PC catalog (snapshot + user_local_manifest)에 없는 후보만 노출
    ↓
[GUI]
  Catalog 페이지에 "신규 발견" 별도 탭 또는 섹션
  카드 그리드 + 자동 라벨 표시 + "추가/거부" 1-click
    ↓
[사용자]
  추가 → user_local_manifest.json 신규 entry
  거부 → 7일 dismiss 캐시
    ↓
[자동]
  user_local_manifest는 main catalog와 합쳐 user view 생성
```

### 2.2 user_local_manifest 분리 정책

- 위치: `%LOCALAPPDATA%/com.lmmaster.desktop/user_local_manifest.json`.
- 스키마: `manifests/snapshot/models/<...>.json` 동일 구조 (`schema_version: 1`).
- 사용자 직접 추가한 model entries만 보관.
- 메인 catalog (snapshot + jsdelivr fetch) ↔ user_local_manifest 충돌 시 **메인 우선**.
- 사용자가 user_local_manifest entry 삭제 가능 (별도 button).

### 2.3 GUI 구조 (디자인 토큰만)

- **Catalog 페이지에 신규 탭** "신규 발견" 또는 별도 sub-section.
- 카드 그리드:
  - lucide icon (`Sparkles` / `Flame` / `ShieldAlert` 등 — 카테고리별)
  - 모델 이름 + repo + 다운로드 수 + 작성자
  - 자동 라벨 chip (라이선스 / 한국어 / 위험 / VRAM)
  - 두 button: **"내 카탈로그에 추가할게요"** primary / **"이번엔 안 할게요"** secondary (7일 dismiss)
- a11y: focus-visible + role="listitem" + 한국어 aria-label.
- 해요체 카피 (CLAUDE.md §4.1).

### 2.4 자동 라벨 정의

| 라벨 | 자동 결정 기준 |
|---|---|
| **라이선스 안전** | Apache-2 / MIT / BSD / CC-BY → green chip |
| **상업 제한** | CC-BY-NC / Llama Community / EXAONE Custom → yellow chip |
| **라이선스 주의** | 미기재 / unknown → gray chip + "확인 필요" |
| **한국어 가능** | Model card에 "한국어/Korean/한글" 포함 → KR chip |
| **NSFW 가능** | model id / tags에 "rp/uncensored/nsfw/erotic" 포함 → red chip + 성인 토글 켜져야 노출 |
| **VRAM 추정** | model size GB × 0.55 → 추정 VRAM (Q4_K_M 기준) |

## 3. 기각안 + 이유

- **운영자 중앙 큐레이션** (현재 정책): 운영자 매주 1-2시간 부담 + 신규 모델 누락 위험. 사용자 명시 거부 (2026-05-09).
- **하이브리드** (운영자 큐레이션 + 사용자 자율 둘 다): 두 manifest 채널 동시 운영 + 사용자 화면에서 *어떤 게 검증된 건지* 혼란 + 코드 복잡도 큼. 기각.
- **HF Trending 100% 자동 추가** (review 없음): 위험 모델 (NSFW / 라이선스 함정) 자동 카탈로그 진입. 사용자 신뢰 깨짐. 기각.
- **별도 발견 페이지 신규**: Catalog와 분리하면 사용자 진입점 분산. Catalog 안 *신규 발견 탭*으로 통합이 단일 진입점. 채택.

## 4. 미정 / 후순위

- HF Trending API rate limit (anonymous 60/h, authenticated 1000/h) — 사용자별 fetch 시 60/h 충분. 그러나 *동시 다수 사용자*는 무관 (각 PC별 독립).
- 라이선스 자동 라벨 정확도 — heuristic 기반. *false positive/negative 발생*. v1.x 사용자 피드백 후 reinforce.
- *사용자가 추가한 model의 자동 다운로드 + 사용*: 메인 catalog 모델과 동일 흐름 (start_model_pull 등). 별도 처리 X.
- *user_local_manifest 휴대성*: portable workspace export/import에 포함 여부. v1.1.

## 5. 테스트 invariant

- HF Trending API JSON parse + 실패 시 graceful degradation (빈 리스트).
- 자동 라벨 생성 함수 unit tests (라이선스 분류 / 한국어 detect / NSFW heuristic).
- 거부 7일 dismiss cache (동일 모델 7일간 안 보임).
- user_local_manifest 추가/삭제 round-trip + JSON schema validation.
- 메인 catalog 충돌 시 우선순위 (메인 > user_local).
- 후보 fetch 외부 통신 화이트리스트: `huggingface.co` API only.

## 6. 다음 페이즈 인계

본 ADR 다음 세션 본격 구현. 작업 분할:

### Phase 22'.h.1 (~3-4일) — backend
- `crates/hf-trending` (또는 `crates/discovery`) 신규 — HF Trending API client.
- `apps/desktop/src-tauri/src/discovery/` 모듈 — 24h cron + 캐싱 + IPC 4개 (`list_trending_candidates` / `approve_candidate` / `reject_candidate` / `clear_dismiss_cache`).
- `user_local_manifest.rs` — load/save/validate.
- 자동 라벨 함수 + unit tests.

### Phase 22'.h.2 (~3-4일) — frontend
- Catalog 페이지에 "신규 발견" sub-section.
- 카드 컴포넌트 + 자동 라벨 chip + button.
- ko/en i18n.

### Phase 22'.h.3 (~2일) — 정합성
- 메인 catalog ↔ user_local_manifest 합치기 logic.
- 충돌 우선순위.
- portable export 정책 (v1.1).

### Phase 22'.h.4 (~1일) — 검증 + 출시
- 사용자 e2e 검증 + 한국어 카피 점검.
- v0.8.0 minor bump.

총 9-11일. 별도 큰 sub-phase로 다음 세션 (또는 별 일정).

진입 조건: 본 ADR 채택 + 메모리 갱신 (현 세션 완료).

위험 노트:
- HF Trending API endpoint이 *변경 또는 deprecated* 시점에 fallback 필요. v1.x 사용자 피드백 후 health check.
- 자동 라벨 *false negative* (NSFW 미감지 등) 시 사용자 *bug 보고*가 유일한 reinforce 채널. 보수적 라벨 (의심 시 red chip 우선).
- user_local_manifest의 사용자 GGUF 다운로드 시 sha256 미기재 — `parse_optional_sha256` (placeholder skip)로 호환.

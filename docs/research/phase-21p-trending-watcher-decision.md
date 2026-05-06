# Phase 21' — Trending Watcher 결정 노트 (deterministic 필터 + human review queue)

> **작성일**: 2026-05-06
> **선행 문서**: `docs/adr/0059-trending-watcher.md`, `docs/research/phase-21p-trending-watcher-reinforcement.md`
> **트리거**: 사용자 명시 승인 (옵션 1) — "수동만으로는 가치 떨어진다, 약속 이행 필요".

---

## 1. 결정 요약

- **A1**: 별도 repo `lmmaster-trending-watcher` (public, MIT) + GHA cron `0 */6 * * *`. LMmaster 본 repo는 PR 받는 측.
- **A2**: 데이터 소스 4종 — HF Trending API + Open LLM Leaderboard 2 + Arena 미러 + KMMLU 정규식. OpenRouter / Open Ko-LLM / LMSYS 공식 거부.
- **A3**: Deterministic 가중치 매트릭스 (0.35·Open_LLM + 0.20·log10(downloads) + 0.20·korean_signal + 0.15·license + 0.10·gguf_present). LLM judge 0.
- **A4**: `JasonEtco/create-an-issue` (dedupe 우월). 라벨 `auto-curate` + `trending-watcher` + `needs-review`. 큐레이터 1인 assignee.
- **A5**: 큐레이터 검토 후 manifest PR (LMmaster 본 repo). **자동 PR 거부** — 사람 검토가 큐레이션 정체성.

상세는 ADR-0059 §1~7 참조.

---

## 2. 채택안

ADR-0059 §결정 1~7에 모두 명시. 본 결정 노트는 cross-reference만:

- **§1 호스팅** — 별도 repo (ADR-0059 §1).
- **§2 화이트리스트 보존** — `huggingface.co` + `github.com`만 (ADR-0059 §2).
- **§3 데이터 소스 매트릭스** — 4종 (ADR-0059 §3).
- **§4 가중치 + 임계** — score 공식 + license whitelist + Korean signal + 사이즈 게이트 (ADR-0059 §4).
- **§5 GHA action** — `JasonEtco/create-an-issue` (ADR-0059 §5).
- **§6 검토 흐름** — Issue → 큐레이터 → manifest PR (ADR-0059 §6).
- **§7 본 repo 영향 0** — PR 받는 측만 (ADR-0059 §7).

---

## 3. 기각안 + 이유 (negative space)

ADR-0059 "거부된 대안" 12건 인용 — 핵심 5건만 결정 노트에 명시:

| 기각안 | 거부 이유 |
|---|---|
| **완전 자동 manifest PR** | 큐레이션 정체성 와해. chat template 깨짐 / 라이선스 함정 / 한국어 자연스러움 미검증 위험. 인간 검토 의도적 게이트 |
| **LMmaster 데스크톱 내장** | 사용자 PC 켜져 있어야 + secrets 관리 부담 + 사용자별 배포본 추적 부담 |
| **OpenRouter 화이트리스트 추가** | `openrouter.ai` 신규 도메인 = 정체성 훼손. `hugging_face_id` 신호 가치는 인정, v1.x 후순위 이월 |
| **Open Ko-LLM Season 2 primary** | Space 504 timeout + 마지막 commit 2024-03 → 신뢰성 낮음 |
| **LLM judge 가중** | deterministic only 정체성 위반. score 100% 코드 상수 |

전체 12건은 ADR-0059 "거부된 대안" 참조.

---

## 4. 미정 / 후순위 이월

- **OpenRouter `hugging_face_id` 신호 활용** — v1.x 후순위. 화이트리스트 추가 시 별도 ADR + 사용자 결정.
- **Open Ko-LLM Season 2 안정화 fallback** — Season 2가 504 timeout 해소 + 정기 갱신 재개 시 KMMLU + Open Ko-LLM 듀얼 source 검토.
- **자동 벤치마크 실행 (LMmaster가 사용자 PC에서 KMMLU 등 돌림)** — ADR-0048 거부 그대로. v2+ 검토.
- **Korean leaderboard primary 후보 확장** — KoCommonGen, HAERAE, KoBEST 등. v1.x 후순위.
- **HF Token 사용** — 익명 한도 충분, token 누출 위험 + 사용자 보안 부담. 후순위.
- **EULA 갱신** — 자동 발견 / 큐레이터 알림 흐름 명시. 본 페이즈 진입 시 갱신.

---

## 5. 테스트 invariant

ADR-0059 §테스트 invariant 8건 그대로:

1. **Deterministic** — 동일 JSON snapshot → 동일 score 100회 반복.
2. **License whitelist** — `other` license = score 0 + 큐 제외.
3. **사이즈 게이트** — `< 3B` or `> 14B` info-only 큐로 분리.
4. **GGUF gate** — `library_name != gguf` + 미러 hit 0이면 큐 제외.
5. **Issue dedupe** — 동일 `hub_id` 두 번 발견 시 issue 1개 (update_existing 동작).
6. **Korean signal boundary** — 정규식 0 hit 0.0, 3+ hit 1.0 cap.
7. **Empty fetch** — HF 응답 빈 배열 → graceful skip + warning.
8. **Rate limit 429** — `RateLimit-Retry-After` 헤더 honor.

---

## 6. 다음 페이즈 인계 — sub-phase 분할

### 진입 조건
- 사용자 명시 승인 — 별도 repo 신설 (큰 아키텍처 분기, CLAUDE.md §1).
- ADR-0059 사용자 명시 승인.
- LMmaster 본 repo `freessunky-bit/lmmaster`에 sister repo 권한 정책 정의.

### sub-phase 5단계

| Phase | 제목 | 의존성 | DoD |
|---|---|---|---|
| **21'.a** | 별도 repo 신설 + Cargo workspace + CI 골격 | ADR-0059 | `lmmaster-trending-watcher` 생성, Rust binary 스켈레톤, GHA workflow 골격, README ko/en |
| **21'.b** | HF + Open LLM Leaderboard fetcher | 21'.a | `huggingface.co/api/models` + `datasets-server.huggingface.co` Parquet 파싱. 캐시. 5 invariant |
| **21'.c** | Deterministic 필터 매트릭스 | 21'.b | 가중치 score 함수 + license whitelist + Korean signal 정규식 + 사이즈/다운로드 게이트. 8 invariant |
| **21'.d** | GHA cron + JasonEtco/create-an-issue | 21'.c | 6h cron + report.md 생성 + Issue dedupe. 라벨/assignee 정책 |
| **21'.e** | 큐레이션 흐름 통합 + CURATION_GUIDE 갱신 | 21'.d | LMmaster 본 repo `docs/CURATION_GUIDE.md` 1단락 + manifest PR 템플릿 + 1주 운영 모니터링 |

### 위험 매트릭스

| 위험 | 영향 | 완화 |
|---|---|---|
| HF API rate limit 429 | 큐레이션 알림 누락 | RateLimit-Retry-After honor + 6h 간격 (익명 quota 50배 마진) |
| Issue noise (false positive) | 큐레이터 부담 ↑ | deterministic 임계 + dedupe + 1주 운영 후 가중치 튜닝 |
| 새 모델 인지 지연 | competitive 가치 하락 | 6h cron이 충분 (기존 수동 발견 대비 큰 개선) |
| 큐레이터 1인 병목 | review queue 정체 | v1.x 후순위에 큐레이터 다인화 + 라벨 자동 분류 |
| 별도 repo secrets 누출 | trending watcher 위장 PR 위험 | LMmaster 본 repo PR review로 차단 (자동 PR 거부 정책) |

### 다음 standby (사용자 결정 후 v1.x 진입)
- Phase 21'.a 진입 — `lmmaster-trending-watcher` repo 신설 결정.

### 검증 명령 (별도 repo)
```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
# GHA 자체 검증은 workflow_dispatch로 dry-run.
```

---

## 출처 (보강 리서치 노트 §11 참조)

`docs/research/phase-21p-trending-watcher-reinforcement.md` §11에 8개 출처 — HF Hub API / Rate Limits / Open LLM Leaderboard / Arena 미러 / KMMLU / GHA actions / parquet crate.

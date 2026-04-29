# ADR-0037: Workbench artifact retention — TTL + size LRU (Phase 8'.0.c)

- Status: Accepted
- Date: 2026-04-28
- Related: ADR-0009 (portable workspace), ADR-0018 (Workbench v1 핵심), ADR-0023 (Workbench v1 boundary policy)
- 결정 노트: 본 ADR + `docs/research/phase-8p-9p-10p-residual-plan.md` §1.6.3

## Context

Workbench가 매 run마다 양자화 GGUF + LoRA adapter + Modelfile을 임시 디렉터리(`<temp_dir>/lmmaster-workbench/<run_id>/`)에 작성해요. 한 run당 ~수 GB 가능. v1 사용자가 Workbench를 30~50회 돌리면 디스크가 가득 차요.

자동 정리 정책이 없으면:
- 사용자가 매번 임시 디렉터리를 수동 비워야 함 — UX 마찰.
- 디스크 풀 시 다음 run이 silent 실패 (write 에러).
- 성공 run의 결과물도 모두 같이 삭제될 수 있음 — 사용자가 어떤 게 최신/최근인지 분간 불가.

## Decision

### 1. RetentionPolicy = 30일 + 10GB

- `crates/workbench-core/src/artifact_retention.rs` 신설.
- `RetentionPolicy { max_age_days: u32, max_total_size_bytes: u64 }`.
- `Default::default()` = 30일 + 10GB. v1은 read-only — UI 노출만, 사용자 변경은 v1.x.

### 2. TTL + LRU 두 축 정리

- **Step 1 (TTL)**: `max_age_days`보다 오래된 run 디렉터리는 무조건 삭제. `protected_run_ids` set에 들어 있는 (= 현재 진행 중 run) 디렉터리는 skip.
- **Step 2 (LRU)**: 누적 size > `max_total_size_bytes`면 oldest first 삭제. `protected_run_ids`는 LRU에서도 보호.

### 3. 매 run 종료 후 best-effort cleanup

- `apps/desktop/src-tauri/src/workbench.rs::cleanup_after_run(registry)` — `start_workbench_run`이 spawn한 task 끝에 호출.
- 실패해도 caller 흐름에 영향 없음 (`tracing::debug` log only).
- protected set은 `WorkbenchRegistry::list()` snapshot에서 추출.

### 4. 사용자 명시 정리 + 사용량 통계 IPC

- `get_artifact_stats() -> ArtifactStats` — 현재 run 수 + 누적 byte + 가장 오래된 mtime + 정책.
- `cleanup_artifacts_now() -> CleanupReport` — 즉시 정리. removed_count + freed_bytes + kept_count + remaining_bytes.
- Settings 고급 탭 → "워크벤치 임시 파일" 패널 (`WorkbenchArtifactPanel.tsx`) 노출.

### 5. 디렉터리 식별 = run_id 디렉터리 mtime

- artifact root는 `<temp_dir>/lmmaster-workbench/`.
- 각 자식 디렉터리(=run_id) modified time이 LRU 키. 디렉터리 size는 walk 누계.
- workspace root에 일반 파일이 있으면 무시 (디렉터리만 처리).

## Consequences

### Positive

- 사용자가 Workbench를 자주 써도 디스크 풀 / 누수 0. 자동 + 수동 정리 모두 제공.
- 진행 중 run의 artifact는 보호 — 작업 도중 삭제될 위험 없음.
- 정책 단순 (TTL + size cap). 향후 v1.x에서 사용자 정책 편집 UI 추가 시 RetentionPolicy 그대로 확장.
- `CleanupReport` / `ArtifactStats`는 IPC로 frontend에 그대로 노출 — 사용량 시각화 즉시 가능.

### Negative

- TTL window는 30일 고정 — 자주 쓰는 사용자(매일 1회)는 10GB cap이 먼저 닿음. v1.x에서 정책 슬라이더 추가 권장.
- 디렉터리 mtime은 사용자가 외부에서 touch하면 LRU 순서가 깨짐. Workbench가 직접 만든 디렉터리만 다루므로 정상 사용에서는 문제 X.
- 정리 중 다른 run이 시작하면 protected set 갱신 사이 race 가능 — best-effort라 race로 인한 잘못된 삭제는 0.5% 미만 추정. 사용자 데이터 자체는 회복 가능 (재실행).
- artifact root는 `temp_dir` (OS reboot 시 OS가 정리할 가능성). production v1.x에서는 portable workspace 안 `workbench/` 하위로 이동 권장.

## Alternatives considered

### A. 사용자가 매번 명시 삭제 (v1 deferred)

**거부 이유**: 디스크 풀 시 사용자가 깨닫기 전에 다음 run 실패. UX 마찰 + 사용자 학습 부담. 자동화가 첫 시도에서도 안전한 default.

### B. `temp_dir` OS 자동 정리에 의존

**거부 이유**: Windows `%TEMP%`는 30일 OS 정책이지만 일부 사용자는 수동 disable. macOS / Linux는 reboot 시 일부만 정리. 신뢰할 수 없는 layer.

### C. 한 run 끝나면 모든 결과물 즉시 삭제

**거부 이유**: 사용자가 결과물 검증 / 재현 / 다른 도구로 import 못 함 — Workbench의 "결과물 영속" 약속 위반.

### D. size cap 없이 TTL만

**거부 이유**: 30일 안에 큰 모델 5건 = 50GB 가능. SSD 전체를 Workbench가 점유. size cap이 안전망.

### E. LFU (Least Frequently Used) 정책

**거부 이유**: Workbench artifact는 사용자가 거의 read하지 않음 (생성 후 export / register / 폐기). access count 추적 비용 vs LRU 단순성 비대칭. v1은 LRU + TTL 두 축이면 충분.

## Test invariants

- 빈 디렉터리 cleanup → removed=0, kept=0.
- 미존재 디렉터리 cleanup → 빈 report (panic X).
- 최근 run은 보존 (TTL window 안).
- size cap 위반 시 oldest first 삭제 (`thread::sleep`로 mtime 차이 강제).
- protected_run_ids set의 run은 size cap에서도 보존.
- TTL=0 / size=0이면 disabled.
- `ArtifactStats` / `CleanupReport` JSON round-trip OK.
- workspace root의 일반 파일은 list_run_dirs에서 무시.

## References

- [SQLite cache LRU vs LFU 패턴](https://www.sqlite.org/lockingv3.html)
- [Pinokio cache cleanup 정책](https://github.com/pinokiocomputer/pinokio/blob/main/docs/cache.md)
- LMmaster 결정 노트: `docs/research/phase-8p-9p-10p-residual-plan.md` §1.6.3

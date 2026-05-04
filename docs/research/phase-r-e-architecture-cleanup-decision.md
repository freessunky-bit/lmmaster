# Phase R-E — Architecture Cleanup 결정 노트

> 2026-05-04. R-A/R-B/R-C/R-D 4 페이즈로 v0.0.1 ship-blocker 17건 모두 해소. R-E는 아키텍처 cleanup 7건 — 사용자 가시 동작 변경 0, 코드 품질/성능/신뢰성 cleanup만.

## 1. 결정 요약

7 sub-phase 모두 단일 ADR-0057에 통합 (모두 *architecture cleanup* 같은 본질):

- **D1 (R-E.1, T3)**: chat_stream graceful disconnect 자동 회귀 가드 — raw TcpListener × 3 어댑터 × 2 케이스 = 6 신규 invariant.
- **D2 (R-E.2, C2)**: OpenAI compat DTO 8 struct/enum 중복 제거 → 신규 crate `openai-compat-dto`. alias import로 사용처 0 변경.
- **D3 (R-E.3, A1)**: chat 타입 (ChatMessage / ChatEvent / ChatOutcome) → 신규 crate `chat-protocol`. adapter-ollama re-export 백워드 호환.
- **D4 (R-E.4, A2 재스코프)**: adapter-ollama 역의존 완전 제거. 두 어댑터 + desktop이 chat-protocol 직접 import. RuntimeAdapter trait split은 ROI 낮아 보류 (현 trait이 이미 focused).
- **D5 (R-E.5, P1)**: KnowledgeStorePool 도입 — IPC 호출당 SQLite open 반복 → Arc 캐시. FIFO eviction max=4. Tauri State.
- **D6 (R-E.6, P4)**: Channel send 실패 → backend cancel cascade. chat (Ollama+LmStudio) + portable (Export+Import) 4 사이트.
- **D7 (R-E.7, R2)**: WorkspaceCancellationScope — workspace 전환 시 prev workspace의 op 토큰 cascade cancel. opt-in register 정책.

## 2. 채택안

[ADR-0057 §결정 참조 — 본 결정 노트는 negative space + 인계 중심.]

각 sub-phase의 commit:
- R-E.2 (`34c3cf7`) `openai-compat-dto` crate 신설 + 8 DTO + 6 invariant
- R-E.5 (`e421f24`) `KnowledgeStorePool` + 5 invariant + 4 IPC 갱신
- R-E.1 (`8f2e8f9`) raw TcpListener 기반 6 invariant (3 adapter × 2 case)
- R-E.3 (`59b8a0a`) `chat-protocol` crate + 7 invariant + adapter-ollama re-export
- R-E.4 (`e102eee`) adapter-ollama dep 제거 + 의존 그래프 정상화
- R-E.6 (`cd4fc71`) Channel close → cancel cascade (chat + portable)
- R-E.7 (`39924ff`) `WorkspaceCancellationScope` + 5 invariant + set_active_workspace 트리거

## 3. 기각안 + 이유

| # | 기각안 | 이유 |
|---|---|---|
| 1 | wiremock으로 직접 abrupt disconnect | wiremock-rs API가 raw socket drop 미지원. fork ROI 낮음. raw TcpListener 정공 |
| 2 | DTO를 `shared-types::openai_compat` 모듈에 | shared-types는 cross-cutting 도메인 타입. 와이어 DTO는 책임 분리 |
| 3 | adapter-ollama 그대로 두고 외부에서 분기 | 의존 방향 여전히 뒤집힌 채 — 새 어댑터 추가 시 cycle 위험 |
| 4 | RuntimeAdapter trait split (R-E.4 원안) | 현 trait이 이미 lifecycle-focused. impl Fn(...) async_trait 호환성 이슈. 재스코프 "역의존 제거"가 더 의미 |
| 5 | KnowledgeStorePool RwLock으로 read concurrency | SQLite WAL + busy_timeout 충분. RwLock은 contention 측정 후 v2.x |
| 6 | proper LRU (move-to-front) | max=4 환경에선 FIFO와 LRU 차이 미미. v2.x |
| 7 | Tauri 2 Channel::on_close 콜백 | 현 API에 명시 on_close 미지원. send-fail polling 표준 |
| 8 | 모든 op CancellationToken을 parent.child_token()으로 | 모든 op 시그니처 변경 — 큰 리팩토링. opt-in register가 점진적 |
| 9 | workspace 전환 시 모든 workspace cancel | 다른 workspace에 활성 op (미래 multi-workspace UX) 있을 수 있음 — prev만 cancel 정합 |
| 10 | 모든 R-E를 단일 commit | PR review 친화도 ↓. 7 commit 분할로 각 sub-phase 독립 verify |
| 11 | KnowledgeStorePool drop on workspace switch | 다음 workspace 진입 시 open 비용 재발생. 현 LRU max=4면 실질적으로 4 workspace 활성 캐시 |
| 12 | Channel cancel을 callback parameter로 (closure 없이) | adapter chat_stream 시그니처 변경 — 어댑터 책임 침범. closure capture가 정공 |
| 13 | WorkspaceCancellationScope를 op마다 강제 | 백워드 호환 깨짐. opt-in이 점진적 wiring 가능 |
| 14 | wiremock 테스트를 통합 테스트(integration)로 분리 | 단위 테스트 영역 충분. integration crate 신설 ROI 낮음 |

## 4. 미정 / 후순위 이월

R-E 후속 (v2.x 잠재):

- **R-E.5 RwLock 전환** — concurrency 측정 후 결정
- **R-E.7 chat/ingest/bench register wiring** — 점진적 적용
- **R-E.7 LRU access ordering** (proper move-to-front)
- **chat / bench / install ChannelSink cancel 통합 audit** — 일관 패턴 정리
- **RuntimeAdapter trait true split** — 새 어댑터(synthetic test responder 등) 추가 시 재평가
- **wiremock chunked response abrupt disconnect 헬퍼 추출** — 3 어댑터 테스트 코드 중복
- **KnowledgeStorePool 통계 IPC** — Diagnostics에 캐시 hit rate 노출

## 5. 테스트 invariant

본 sub-phase가 깨면 안 되는 invariant:

### R-E.1 (T3)
1. ollama: delta 1+ emit + transport error → ChatOutcome::Completed
2. ollama: delta 0 + transport error → ChatOutcome::Failed
3. lmstudio: 동 정상 케이스
4. lmstudio: 동 에러 케이스
5. llama-cpp: 동 정상 케이스
6. llama-cpp: 동 에러 케이스

### R-E.2 (C2)
7. Content::Text untagged serialize → string
8. Content::Array → array of {type, ...}
9. ContentPart tag = "text" / "image_url" snake_case
10. ChatChunk default empty choices
11. finish_reason round-trip
12. ChatRequest stream:true serialize

### R-E.3 (A1)
13. ChatMessage None images → 직렬화 skip
14. ChatMessage Some images → array 직렬화
15. ChatMessage 백워드 (images 없는 wire) → None
16. ChatEvent::Delta tag = "delta"
17. Completed round-trip
18. Failed message field
19. Cancelled tag = "cancelled"

### R-E.5 (P1)
20. 같은 path → 같은 Arc (cache hit)
21. 다른 path → 다른 Arc
22. FIFO eviction (max 초과 시 oldest drop)
23. 빈 path = in-memory cache
24. Default = max_size 4 + 빈 pool

### R-E.7 (R2)
25. register → cancel_workspace → 모든 토큰 cancel + 인덱스 제거
26. cancel_workspace는 다른 workspace 영향 없음
27. cancel_all 모든 workspace drain
28. unknown workspace cancel = noop
29. register 1건 = workspace_count 1, token_count 1

**총 신규 invariant: 29**. 기존 24+ invariant 0건 깨짐.

## 6. 다음 페이즈 인계

### v0.0.1 ship 가능

R-A/B/C/D 17 ship-blocker + R-E 7 architecture cleanup 모두 완료. v0.0.1 release tag push 가능:

```bash
git tag v0.0.1 && git push origin v0.0.1
```

→ release.yml 자동 트리거 → 4-platform 빌드 + draft Release.

### 분리된 sub-phase (별개 진행)

- **#31** — Knowledge IPC tokenized path (R-A.3 분리). 8+ IPC + frontend 영향.
- **#38** — knowledge-stack caller wiring (R-B.2 후속). keyring + 마이그레이션 + per-workspace passphrase.

둘 다 v0.0.1 ship 후 사용자 피드백 보고 우선순위 재평가.

### v2.x 잠재 (POST v0.0.1 release)

- §4 미정 항목 모두

### 검증 명령

```powershell
.\.claude\scripts\check-acl-drift.ps1
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude lmmaster-desktop
cd apps/desktop
pnpm exec tsc -b
pnpm exec vitest run
```

### 다음 standby

**v0.0.1 release tag push** (사용자 결정). 또는 분리된 sub-phase #31 / #38 진입.

# Phase 22'.h — 사용자 측 분산 모델 발견 결정 노트

> 작성: 2026-05-09 (v0.7.1 직후). ADR-0065 기반 결정 노트.

## 1. 결정 요약 (5건)

1. **운영자 중앙 큐레이션 폐기** — 사용자 명시 거부 (관리 이슈).
2. **HF Trending API + 사용자 PC 측 24h cron** — 후보 자동 fetch.
3. **user_local_manifest.json 분리** — 메인 catalog (jsdelivr 배포) ↔ 사용자 추가 entry 분리.
4. **자동 라벨 시스템** — 라이선스 / 한국어 / NSFW / VRAM heuristic. 사용자 판단 도움.
5. **GUI = Catalog 안 "신규 발견" 탭** — 별도 페이지 신규 X. 단일 진입점.

## 2. 채택안

ADR-0065 §2 참조. 핵심:
- 24h cron으로 HF Trending fetch (anonymous, rate limit 안전).
- 메인 catalog + user_local_manifest 빼고 *모르는 모델*만 후보로.
- 카드에 자동 라벨 chip + 1-click 추가/거부.
- 거부 7일 dismiss (재노출 방지).

## 3. 기각안 + 이유

- *운영자 큐레이션*: 사용자 명시 거부.
- *하이브리드*: 두 채널 동시 → 신뢰 vs 자율 혼란.
- *완전 자동 추가* (review 없음): 위험 모델 자동 진입.
- *별도 페이지*: 사용자 진입점 분산.
- *Trending 외 다른 source* (Reddit / GitHub trending 등): API 안정성 + LMmaster의 외부 통신 화이트리스트 정책 부합 X. v1.x.
- *Trending 후보 자체 사용자별 personalize*: 사용자 PC 환경 (GPU / RAM) 기반 추천 추가 → 복잡도. v1.x reinforce.

## 4. 미정 / 후순위

- 자동 라벨 정확도 (라이선스 / NSFW heuristic 한계). v1.x 사용자 피드백 후 reinforce.
- HF Trending API endpoint 변경 시 fallback. v1.x.
- *사용자가 추가한 모델의 download URL은 사용자 입력*인지 *자동 derive*인지. ModelEntry::source 자동 채우기 (HuggingFace 패턴) — `https://huggingface.co/{repo}/resolve/main/{file}` 자동.
- portable workspace export 시 user_local_manifest 포함 여부. v1.1.
- *발견 후 시간이 흘러 모델이 deprecated 된 경우* 자동 표시. v1.x.

## 5. 테스트 invariant

- HF Trending API JSON 변환 stub test.
- 자동 라벨 함수 (라이선스 분류 5종 / 한국어 detect / NSFW heuristic) unit tests.
- 거부 7일 dismiss cache (동일 모델 ID 7일 안 보임 + 만료 후 다시 노출).
- user_local_manifest add/remove round-trip.
- 메인 catalog 충돌 시 메인 우선.
- 후보 fetch 빈 결과 graceful (빈 리스트).
- a11y — Catalog 신규 탭 focus + 카드 키보드 navigation.

## 6. 다음 페이즈 인계

ADR-0065 §6 4 sub-phase 분할 그대로:
- 22'.h.1 backend (3-4일)
- 22'.h.2 frontend (3-4일)
- 22'.h.3 catalog 합치기 (2일)
- 22'.h.4 검증 + v0.8.0 출시 (1일)

진입 조건:
- 본 ADR + 결정 노트 채택 ✅
- 메모리 갱신 (큐레이션 정책 변경 명시) — 현 세션 완료
- 다음 세션 깨끗한 컨텍스트로 진입

위험 노트:
- 본 페이즈는 LMmaster 6 pillar 중 *큐레이션*을 *자율 발견*으로 변경 — **제품 비전 정책 변화**. 메모리 `product_pivot_v1` 갱신 필요.
- HF Trending API 의존 → 외부 서비스 down 시 폴백 (캐시 stale 사용 + 한국어 안내).

# 6. 위험요소와 대응책

> 산출물 #6. 발생 가능한 리스크를 영역별로 정리하고 대응책을 명시한다.
> 우선순위는 **영향도(블래스트 반경)** × **발생 가능성**.

## 6.1 기술 리스크

| ID | 리스크 | 영향 | 가능성 | 대응 |
|---|---|---|---|---|
| T1 | localhost 포트 충돌 | 사용자 첫 실행 실패 | 중 | 자동 포트 회피(0..N 순차 시도) + 사용자 변경 가능 + SDK에 health probe 자동 탐색 |
| T2 | 자식 런타임 프로세스 좀비/누수 | 메모리 폭발, GPU 점유 | 중 | supervisor가 PID 추적, OS shutdown hook + crash recovery, process group kill |
| T3 | 모델 다운로드 도중 중단/손상 | 디스크 낭비, 사용자 좌절 | 높음 | resumable HTTP(range) + sha256 검증 + 검증 실패 시 자동 재다운로드 옵션 + 부분파일 정리 |
| T4 | 런타임 OS×GPU 매트릭스 빌드 실패 | 일부 PC에서 동작 안 함 | 중 | 사전 build matrix CI(Win/mac/Linux × CUDA/ROCm/Vulkan/Metal/CPU) + fallback runtime 추천 |
| T5 | Tauri WebView 차이로 UI 깨짐 | 일부 OS에서 시각 회귀 | 중 | 디자인 시스템에서 vendor prefix + visual regression 테스트(Playwright) |
| T6 | SQLite 동시성 손상(WAL 미설정) | 데이터 손실 | 낮음 | WAL on, 단일 writer 패턴, 마이그레이션은 가동 전 |
| T7 | 양자화 다양성으로 모델별 메모리 추정 오차 | 추천 부정확 → 메모리 부족 OOM | 중 | recommender에 안전 마진(예: rec_vram의 1.15배) + 첫 실행 OOM 감지 시 자동 다운그레이드 제안 |
| T8 | GPU 드라이버/CUDA 버전 호환성 | 런타임 시작 실패 | 높음 | hardware-probe가 드라이버 버전 수집, 어댑터별 supported matrix와 비교, 상세 한국어 안내 |
| T9 | Hugging Face 다운로드 정책/auth 변경 | 모델 fetch 실패 | 중 | manifest source를 다중화(미러, 직접 호스팅 옵션), HF token 사용자 입력 옵션 |
| T10 | llama.cpp 등 OSS API breaking change | 어댑터 깨짐 | 높음 | 어댑터별 버전 핀 + CI에서 N-1 호환 회귀 테스트 + 업스트림 watch 자동화 |

## 6.2 제품/UX 리스크

| ID | 리스크 | 영향 | 가능성 | 대응 |
|---|---|---|---|---|
| P1 | 사용자가 "별도 설치" 단계에 좌절 | 첫 사용 이탈 | 중 | OS 표준 installer + 첫 실행 wizard + 한국어 onboarding + 기존 웹앱이 실행 유도 모달 + custom URL scheme |
| P2 | 추천 모델이 사용자 PC에 부적합 | 신뢰 상실 | 중 | recommender 안전 마진 + 첫 실행 후 워밍업 단계에서 실측 + 부적합 시 자동 다운그레이드 제안 |
| P3 | 모델 카탈로그가 트렌드를 못 따라감 | 매력도 하락 | 높음 | curated manifest 외부 git repo + CI 자동화 + 메인테이너 1인 이상 배정 |
| P4 | 한국어 카피의 일관성 부족 | 톤 drift | 중 | voice & tone 가이드 + 카피 리뷰 체크리스트 + i18n 키 lint |
| P5 | 워크벤치 placeholder가 "비어 있다"는 인상 | 제품성 의심 | 중 | "곧 제공" 정확히 명시, 로드맵 카드, 베타 신청 옵션 |
| P6 | 모델/런타임 용량으로 디스크 압박 | 사용자 분노 | 높음 | 디스크 사용량 화면 + 자동 캐시 정리 + LRU 모델 정리 제안 |

## 6.3 보안 리스크

| ID | 리스크 | 영향 | 가능성 | 대응 |
|---|---|---|---|---|
| S1 | 동일 PC의 다른 프로세스가 무인증 호출 | 키 우회 | 높음 | localhost 바인딩 + 모든 endpoint에 API key 의무화 + 키 없으면 401 |
| S2 | API 키 평문 저장 | 키 유출 | 중 | secrets DB는 SQLCipher, 키 자체는 OS keychain 위임 |
| S3 | 사용 로그에 프롬프트/응답 그대로 저장 → 민감정보 | 프라이버시 | 중 | 기본은 메타(토큰 수, 모델, 시각)만, content 저장은 opt-in + 자동 회전·삭제 |
| S4 | raw runtime port 외부 노출 | 로컬 네트워크 공격 | 중 | runtime은 항상 127.0.0.1 + 우리만 connect, 외부 노출 금지를 코드 + 문서 양쪽에 강제 |
| S5 | 서명되지 않은 모델 파일 실행 | 공급망 공격 | 중 | manifest의 sha256 강제, 미스매치 시 거부 |
| S6 | 자동 업데이트 변조 | RCE | 중 | 코드 사인 + Sparkle/Tauri updater 서명 검증 |
| S7 | Gemini API 키 유출 (사용자 입력) | 사용자 과금 폭증 | 중 | secrets DB 저장, 발신 로그 표시, 토글 OFF 옵션, 사용량 표시 |
| S8 | 서드파티 SDK가 키 발급 후 권한 남용 | 의도치 않은 모델 호출 | 중 | scope per key + 사용 로그 + 1-click revoke |

## 6.4 라이선스/법 리스크

| ID | 리스크 | 영향 | 가능성 | 대응 |
|---|---|---|---|---|
| L1 | 모델 라이선스 위반 (재배포/상업 사용) | 법적 분쟁 | 중 | manifest의 `license` 필드 사용자에게 항상 표시, 상업/비상업 표시, "I agree" 게이트 옵션 |
| L2 | 런타임 OSS 라이선스 (GPL 등) 결합 | 배포 형태 제약 | 중 | 어댑터는 별도 프로세스로 호출 → 결합도 약화, 라이선스 매트릭스 문서화 |
| L3 | Hugging Face TOS 변경 | 자동 다운로드 차단 | 낮음 | 다중 mirror, 사용자 token 옵션 |
| L4 | GDPR/PIPA 등 사용 로그 보관 | 규제 위반 | 낮음 | 로그 보관 기간 사용자 설정 + 1-click 삭제 |

## 6.5 운영 리스크

| ID | 리스크 | 영향 | 가능성 | 대응 |
|---|---|---|---|---|
| O1 | 자동 업데이트 실패로 앱 부팅 불가 | 모든 사용자 차단 | 낮음 | 업데이트 실패 시 이전 버전 자동 롤백, staged rollout |
| O2 | manifest 호스팅 다운 | 카탈로그 갱신 불가 | 낮음 | 마지막 cache 사용 + 다중 미러 + 24시간 cache TTL fallback |
| O3 | 한국어 voice 변경으로 카피 마이그레이션 부담 | UX 회귀 | 중 | 키 베이스 i18n + 마이그레이션 스크립트 + 시각 테스트 |
| O4 | macOS 공증 실패 | mac 사용자 차단 | 중 | Tauri 권장 사인 흐름 자동화 + 사전 검증 CI |

## 6.6 의사결정 리스크 (메타)

| ID | 리스크 | 영향 | 가능성 | 대응 |
|---|---|---|---|---|
| M1 | ADR 없이 핵심 스택 변경 | 일관성 붕괴 | 중 | PR 체크리스트에 "스택 변경은 ADR 필수" + 코드오너 |
| M2 | LiteLLM/Ollama 단일 종속으로 회귀 | 록인 | 중 | 어댑터/키 매니저는 우리 자산이라는 원칙 ADR-0007에 명시, 신규 PR 가드 |
| M3 | Gemini가 추천에 끼어듦 | 비결정성 | 중 | 코드 리뷰 + recommender 단위 테스트(Gemini disable 시 동일 결과) |
| M4 | 스코프 크리프(워크벤치 v1 강제) | 일정 폭발 | 높음 | M0~M6 마일스톤 동결, 워크벤치는 placeholder 유지 |

## 6.7 우선순위 Top 10 (요약)

1. **T10** — OSS 업스트림 break (지속 모니터링 + N-1 회귀 테스트)
2. **T3** — 다운로드 손상 (resumable + checksum)
3. **S1** — 인증 우회 (모든 endpoint key 의무)
4. **P3** — 카탈로그 신선도 (manifest CI 자동화)
5. **T8** — GPU 드라이버 호환 (probe + supported matrix)
6. **P6** — 디스크 압박 (사용량 + LRU)
7. **L1** — 모델 라이선스 (UI 노출 + 동의)
8. **T7** — 메모리 추정 오차 (안전 마진 + 자동 다운그레이드)
9. **M4** — 스코프 크리프 (placeholder 유지)
10. **P1** — 첫 진입 마찰 (onboarding + URL scheme)

각 항목은 별도 GitHub Issue 템플릿으로 발행 → 마일스톤 진행 시 진행 상태 추적.

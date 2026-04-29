# 2. 왜 이 구조가 "기존 웹앱이 호출만 하는" 방식에 가장 유리한가

> 산출물 #2. companion 통합 방식이 다른 대안들보다 우위인 이유와 그 비용을 정리한다.

## 2.1 비교 대안들

기존 웹앱과 로컬 AI를 연결하는 방식은 크게 4가지가 있다.

| 대안 | 형태 | 결합도 |
|---|---|---|
| **A. 직접 통합** | 기존 웹앱 안에 Ollama/llama.cpp 클라이언트 직접 임포트 | 매우 높음 |
| **B. 브라우저 확장** | Chrome/Edge extension이 native messaging | 중간 |
| **C. Companion 데스크톱 앱 (이 프로젝트)** | 별도 프로세스가 localhost gateway 노출 | 낮음 |
| **D. 클라우드 프록시** | 사용자 PC가 아닌 서버에서 추론 | 0 (로컬 아님) |

LMmaster가 채택하는 것은 **C**다.

## 2.2 핵심 요구사항(사용자가 명시)

1. 기존 웹앱은 새 프로그램을 **호출만** 한다.
2. 기존 웹앱은 Ollama/llama.cpp/vLLM/LM Studio를 **직접 붙들지 않는다**.
3. 기존 웹앱은 provider abstraction에 **local-companion provider 1개**를 추가하는 수준으로 끝난다.
4. 새 프로그램이 install/update/recommend/health/routing/fallback을 모두 책임진다.
5. 기존 웹앱에서 새 프로그램의 **미설치/미실행 상태를 감지**할 수 있어야 한다.
6. 새 프로그램을 **실행/연결/설치 유도하는 UX**를 제공해야 한다.
7. 다른 웹앱도 API 키만 받으면 붙일 수 있어야 한다.

이 7개 요구를 한 번에 만족시키는 단일 형태가 **localhost HTTP gateway + companion 데스크톱 앱**이다. 이유를 항목별로 본다.

## 2.3 항목별 우위

### 2.3.1 "호출만" 가능 (요구 1, 3)
- Companion은 **OpenAI-compatible REST**를 노출. 기존 웹앱은 base URL과 API key만 바꾸면 호출이 끝난다.
- provider abstraction에 추가하는 코드는 사실상 `class LocalCompanionProvider extends OpenAIProvider { baseURL = sdk.getBaseUrl() }` 수준.
- 직접 통합(A)이라면 Ollama Node SDK나 llama.cpp HTTP 클라이언트가 웹앱 번들에 들어가고, 모델 라이프사이클 코드까지 따라온다 → 결합도 폭증.

### 2.3.2 런타임 비종속 (요구 2)
- Companion 안에서 `RuntimeAdapter` trait이 모든 차이를 흡수. 웹앱은 어떤 런타임이 동작 중인지 **모른다**.
- 사용자가 GPU를 바꾸면서 vLLM → llama.cpp로 전환해도 웹앱 코드/배포는 무변경.
- 직접 통합(A)은 이 모든 변경이 웹앱 PR로 흘러간다.

### 2.3.3 install/update/health 책임 (요구 4)
- companion은 데스크톱 프로그램이라 **파일 시스템·자식 프로세스·OS 권한**을 자유롭게 쓸 수 있다.
- 브라우저 확장(B)은 native messaging으로 우회는 가능하지만, 권한 모델·OS별 설치 경로·사용자 가이드가 훨씬 복잡하고 macOS/Linux/Windows마다 다른 native host JSON manifest가 필요.
- 브라우저 안의 웹앱은 fetch/WebSocket 외엔 OS 권한이 없어 구조적으로 불가능.

### 2.3.4 미설치/미실행 감지 + 설치 유도 UX (요구 5, 6)
- 기존 웹앱은 부팅 시 `GET http://127.0.0.1:<port>/health`를 시도.
  - 응답 OK → 연결.
  - 응답 실패 → "LMmaster를 설치/실행해주세요" 모달, OS 별 설치 링크 + custom URL scheme(`lmmaster://`)으로 자동 실행 시도.
- companion에 **OS 별 installer**(MSI/DMG/AppImage)가 있으니 사용자 설치가 표준 OS 흐름.
- 직접 통합(A)이라면 "런타임이 안 깔려 있으니 사용자에게 직접 Ollama 설치하라고 하세요" 같은 UX를 웹앱이 떠안는다.

### 2.3.5 다른 웹앱도 키만 받으면 붙음 (요구 7)
- companion은 **다중 클라이언트 처리**가 자연스럽다 — 그냥 HTTP 서버이기 때문.
- 키 매니저가 **scope per key**를 관리. 기존 웹앱과 다른 웹앱이 각자 키를 가진다.
- 직접 통합(A)으로 이걸 하려면 각 웹앱이 자기 안에 런타임 클라이언트를 넣고 같은 모델 파일 두 벌이 되거나, 락/리소스 충돌 처리가 필요.

### 2.3.6 보안·신뢰성
- localhost 바인딩 + API key + 키별 scope = **최소 권한 원칙**을 데스크톱에서 표준적으로 구현 가능.
- raw runtime port를 가리고 있어 향후 런타임 CVE에 대해 우리가 게이트웨이에서 패치할 수 있다.
- 직접 통합(A)은 런타임의 raw port를 사용자 시스템에서 모든 프로그램이 볼 수 있게 노출하는 경우가 많음.

### 2.3.7 운영 분리
- companion이 죽어도 기존 웹앱 자체는 계속 떠 있다(원격 API로 폴백 가능).
- 모델 다운로드 같은 long-running task가 웹앱 탭을 못 죽인다.
- 업데이트/마이그레이션이 웹앱 배포와 디커플링.

### 2.3.8 향후 확장 (워크벤치 / 사운드 / SLM)
- 파인튜닝·양자화·export 같은 무거운 작업은 **데스크톱 프로세스**여야 효율적이고 안전하다(GPU 메모리, 디스크, ML 라이브러리).
- companion은 그 자리(`workers/ml`)를 미리 만들어두고 v1엔 비활성. A/B 안으로는 넣기 매우 어렵다.

## 2.4 비용과 단점 (정직하게)

| 비용 | 영향 | 완화 |
|---|---|---|
| 사용자가 "별도 프로그램 설치"라는 추가 단계 필요 | 처음 진입 마찰 | OS 표준 installer + 첫 실행 wizard + 한국어 가이드, 기존 웹앱이 실행 유도 모달까지 제공 |
| 두 코드베이스 동기화(SDK 버전, gateway API) | breaking change 시 양쪽 배포 | semver + capability discovery (`GET /capabilities`) + SDK가 gateway 버전을 읽고 graceful degrade |
| OS 별 installer/서명/공증 운영 부담 | Windows/macOS/Linux 빌드 파이프 | Tauri 표준 빌드, 단계별 도입 (Win → mac → Linux) |
| localhost 포트 충돌 가능성 | 드물게 다른 앱과 부딪힘 | 자동 포트 회피 + 사용자 변경 가능 + SDK가 health probe로 자동 탐지 |
| 단일 사용자 PC 전제 (멀티유저 OS에서 사용자별 인스턴스 필요) | 기업/가족 PC | per-user 데이터/키 디렉터리, OS user 단위 격리 |

이 비용들은 모두 **회피 가능하거나 표준 데스크톱 앱이 늘 다루는 문제**다. 직접 통합(A)에서 발생하는 결합도 폭증과 권한 한계는 회피가 어렵다.

## 2.5 의사결정 결론

- 사용자의 7개 요구 모두를 동시에 충족시키는 형태는 사실상 **C(companion)** 뿐이다.
- B(브라우저 확장)는 "확장 설치를 강요"가 더 큰 마찰이고 권한이 좁다.
- A(직접 통합)는 "호출만 한다"는 1번 요구와 정면 충돌.
- D(클라우드)는 "로컬 AI"라는 본 프로젝트 정체성과 충돌.

따라서 **localhost HTTP gateway를 노출하는 Tauri 데스크톱 companion + JS/TS SDK**가 기본 설계다.

## 2.6 기존 웹앱의 통합 시 변경 최소 면적

기존 웹앱이 추가해야 하는 것은 다음 4가지뿐이다.

1. `@lmmaster/sdk` 의존성 추가
2. provider abstraction에 `LocalCompanionProvider` 클래스 1개 추가
3. 부팅 시 `sdk.pingHealth()` 호출 후 미연결 상태 UI(설치 유도 모달) 1개
4. (옵션) 모델 선택 UI에 "로컬" 카테고리 노출

기존 웹앱의 채팅 화면, 메시지 형식, 스트리밍 처리 등은 **무변경**. 이게 "호출만 한다"의 의미.

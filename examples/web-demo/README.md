# LMmaster 웹 데모

기존 HTML/JS 웹앱이 endpoint URL과 API 키만 바꿔 LMmaster local gateway를 호출하는 시나리오를 보여주는 최소 데모.

## 시나리오

1. 사용자가 LMmaster 데스크톱을 실행 — gateway가 `http://127.0.0.1:N` (OS 할당)에 listening.
2. LMmaster Settings → "로컬 API 키" → "새 키 만들기" — 발급 modal에서 alias + `http://localhost:5173` Origin + 모델 패턴(`*`).
3. 평문 키 (`lm-…`)를 1회 카피.
4. 본 데모 dev 서버 실행:
   ```bash
   cd examples/web-demo
   pnpm install
   pnpm dev
   ```
5. 브라우저에서 `http://localhost:5173` 열고 baseUrl + 키 + 모델 입력 → "보내볼게요".

## 구조

- 단일 의존성: `@lmmaster/sdk` (workspace:*).
- `src/main.ts`: gateway ping → streamChat (SSE iterator) → DOM에 누적 표시.
- 에러는 `LMmasterApiError`로 캐치 후 한국어 메시지 표시.
- 빌드는 vite. dev 서버는 5173 (LMmaster gateway 포트와 별개).

## ADR-0022 §9 검증 포인트

- OpenAI 호환 baseURL — gateway URL만 바꾸면 됨.
- Origin 정확 매칭 — `http://localhost:5173`이 키 발급 시 등록되어야.
- 1회 reveal 평문 키 — 잃어버리면 새로 발급.
- 게이트웨이 미실행 시 친절 안내 — gateway down → "데스크톱이 실행 중인지 확인해 주세요".

## 트러블슈팅

| 증상 | 원인 | 해결 |
|---|---|---|
| `게이트웨이가 응답하지 않아요` | 데스크톱 미실행 / 포트 다름 | LMmaster 데스크톱 실행 + 홈 화면에서 포트 확인 |
| `[origin_denied] 이 키는 이 사이트에서 호출할 수 없어요` | 키의 allowed_origins에 dev 서버 origin이 없음 | 새 키 발급 시 `http://localhost:5173` 추가 |
| `[invalid_api_key]` | 키 typo / 회수 후 재사용 | 새 키 발급 |
| `[model_not_found]` | 카탈로그에 없는 모델 ID | LMmaster 카탈로그에서 모델 ID 확인 (예: `exaone-4.0-1.2b-instruct`) |

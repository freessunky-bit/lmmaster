# 기존 웹앱 연동 가이드 (한국어)

> 기존 웹앱이 LMmaster를 호출만 하도록 만드는 절차입니다. 변경 면적은 SDK 1개 + provider 1개 + 헬스 체크 모달 1개입니다.

## 1. SDK 설치

```bash
npm i @lmmaster/sdk
# 또는
pnpm add @lmmaster/sdk
```

## 2. provider 추가

`examples/webapp-local-provider/src/providers/local-companion.ts` 의 패턴을 따라
기존 웹앱의 provider abstraction에 `LocalCompanionProvider`를 1개 추가합니다.

## 3. 부팅 시 헬스 체크

```ts
const r = await provider.ensureAvailable();
if (!r.ok) {
  // 한국어 모달: "LMmaster를 설치하거나 실행해주세요."
  // r.launchUrl 로 lmmaster:// 실행 유도.
}
```

## 4. 키 발급

데스크톱 앱 → 프로젝트 연결 → 새 프로젝트 등록 → 키 발급(scope 선택, 1회 표시).
발급된 키를 웹앱의 환경 설정에 보관.

## 5. base URL

기본값: `http://127.0.0.1:43117/v1` (자동 포트 회피로 실제 포트는 변동 가능).
SDK의 `autoFindGateway()` 헬퍼를 사용해 자동 탐색 권장.

## 변경하지 않아도 되는 것

- 기존 채팅 화면, 메시지 포맷, 스트리밍 처리 — 그대로.
- OpenAI SDK 코드 — base URL/키만 LocalCompanionProvider로 라우팅.

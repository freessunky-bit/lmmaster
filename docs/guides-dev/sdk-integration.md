# SDK 연동 가이드 (개발자)

`@lmmaster/sdk`로 LMmaster local gateway에 호출하는 방법.

## 설치

```bash
pnpm add @lmmaster/sdk
```

## 클라이언트 초기화

```ts
import { LMmasterClient, autoFindGateway } from "@lmmaster/sdk";

const client = new LMmasterClient({ apiKey: process.env.LMMASTER_KEY! });

const baseUrl = await autoFindGateway(client);
if (baseUrl) client.baseUrl = baseUrl;
```

## 채팅

```ts
import { chatCompletions, streamChat } from "@lmmaster/sdk";

const res = await chatCompletions(client, {
  model: "qwen2.5-7b-instruct",
  messages: [{ role: "user", content: "안녕" }],
});

for await (const chunk of streamChat(client, { model: "...", messages: [...] })) {
  // chunk는 SSE data 페이로드. JSON.parse 후 delta.content 추출.
}
```

## 키 발급 (관리자 권한 필요)

GUI에서 발급한 admin scope 키로만 호출 가능. 외부 웹앱은 사용하지 않습니다.

```ts
import { issueApiKey } from "@lmmaster/sdk";
const issued = await issueApiKey(adminClient, "my-app", {
  models: ["qwen-*"], endpoints: ["/v1/chat/*"]
});
console.log(issued.plaintext_once); // 1회 표시
```

## 미설치/미실행 감지

```ts
import { pingHealth, buildLaunchUrl } from "@lmmaster/sdk";

const health = await pingHealth(client);
if (!health) {
  // 한국어 모달 + buildLaunchUrl()로 lmmaster:// 실행 유도
}
```

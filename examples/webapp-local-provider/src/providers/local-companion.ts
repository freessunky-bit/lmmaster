// 기존 웹앱이 provider abstraction에 추가하는 LocalCompanionProvider 예제.
// 실제 기존 웹앱은 자체 Provider 인터페이스에 맞춰 이 클래스를 어댑트하면 된다.
//
// 핵심 포인트:
// - SDK 의존성 1개만 추가.
// - 기존 OpenAI provider와 같은 모양의 chat / streamChat 메서드.
// - 부팅 시 pingHealth()로 미연결 감지 → 모달 + lmmaster:// 실행 유도.

import {
  LMmasterClient,
  pingHealth,
  buildLaunchUrl,
  chatCompletions,
  streamChat,
  type ChatRequest,
} from "@lmmaster/sdk";

export class LocalCompanionProvider {
  private client: LMmasterClient;

  constructor(opts: { baseUrl?: string; apiKey: string }) {
    this.client = new LMmasterClient({ baseUrl: opts.baseUrl, apiKey: opts.apiKey });
  }

  async ensureAvailable(): Promise<{ ok: boolean; launchUrl?: string }> {
    const h = await pingHealth(this.client);
    if (h) return { ok: true };
    return { ok: false, launchUrl: buildLaunchUrl(window.location.href) };
  }

  async chat(req: ChatRequest): Promise<unknown> {
    return chatCompletions(this.client, req);
  }

  stream(req: ChatRequest): AsyncGenerator<string, void, void> {
    return streamChat(this.client, req);
  }
}

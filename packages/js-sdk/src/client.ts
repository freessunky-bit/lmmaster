// LMmasterClient — gateway URL + apiKey + custom fetch 보유.
//
// 정책 (ADR-0022):
// - baseUrl은 gateway의 `/v1` prefix 포함 (OpenAI 호환).
// - apiKey가 있으면 Authorization Bearer 헤더 자동 부착.
// - fetchImpl은 SSR / 테스트에서 주입 가능.

import { LMmasterApiError, type ApiErrorEnvelope } from "./types";

export interface LMmasterClientOptions {
  /** 예: "http://127.0.0.1:43117/v1". 가운데 슬래시는 trailing 없이. */
  baseUrl?: string;
  apiKey?: string;
  fetchImpl?: typeof fetch;
}

export class LMmasterClient {
  baseUrl: string;
  apiKey?: string;
  fetchImpl: typeof fetch;

  constructor(opts: LMmasterClientOptions = {}) {
    this.baseUrl = (opts.baseUrl ?? "http://127.0.0.1:43117/v1").replace(
      /\/$/,
      "",
    );
    this.apiKey = opts.apiKey;
    this.fetchImpl = opts.fetchImpl ?? globalThis.fetch.bind(globalThis);
  }

  authHeaders(): Record<string, string> {
    return this.apiKey ? { Authorization: `Bearer ${this.apiKey}` } : {};
  }

  /** 응답 status가 ok가 아니면 OpenAI 호환 envelope을 보존하는 에러로 변환. */
  async ensureOk(res: Response): Promise<void> {
    if (res.ok) return;
    const text = await res.text();
    let envelope: ApiErrorEnvelope | null = null;
    try {
      const parsed = JSON.parse(text);
      if (parsed && typeof parsed === "object" && "error" in parsed) {
        envelope = parsed as ApiErrorEnvelope;
      }
    } catch {
      // text가 JSON이 아니면 generic.
    }
    throw new LMmasterApiError(
      res.status,
      envelope?.error?.type ?? "upstream_error",
      envelope?.error?.code ?? "http_error",
      envelope?.error?.message ?? `gateway ${res.status}: ${text}`,
    );
  }
}

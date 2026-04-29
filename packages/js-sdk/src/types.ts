// SDK 공용 타입. Rust shared-types와 의도적으로 동일한 shape.
// 변경 시 양쪽을 함께 갱신해야 함 (CLAUDE.md §4.4).

export type ModelCategory =
  | "agent-general"
  | "roleplay"
  | "coding"
  | "sound-stt"
  | "sound-tts"
  | "slm"
  | "embeddings"
  | "rerank";

export type RuntimeKind =
  | "llama-cpp"
  | "kobold-cpp"
  | "ollama"
  | "lm-studio"
  | "vllm";

export type RuntimeState =
  | "not-installed"
  | "downloading"
  | "verifying"
  | "extracting"
  | "cold"
  | "warming-up"
  | "standby"
  | "active"
  | "failed";

export interface ChatMessage {
  role: "system" | "user" | "assistant" | "tool";
  content: string;
  name?: string;
}

export interface ChatRequest {
  model: string;
  messages: ChatMessage[];
  stream?: boolean;
  temperature?: number;
  max_tokens?: number;
  top_p?: number;
}

/** OpenAI 호환 single completion 응답. */
export interface ChatCompletion {
  id: string;
  object: "chat.completion";
  created: number;
  model: string;
  choices: ChatCompletionChoice[];
  usage?: ChatUsage;
}

export interface ChatCompletionChoice {
  index: number;
  message: ChatMessage;
  finish_reason: string | null;
}

export interface ChatUsage {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}

/** OpenAI 호환 streaming chunk. */
export interface ChatCompletionChunk {
  id: string;
  object: "chat.completion.chunk";
  created: number;
  model: string;
  choices: ChatCompletionChunkChoice[];
}

export interface ChatCompletionChunkChoice {
  index: number;
  delta: { role?: string; content?: string };
  finish_reason: string | null;
}

export interface InstallProgress {
  stage: string;
  bytes_done: number;
  bytes_total?: number;
  message?: string;
}

export interface ApiKeyScope {
  models: string[];
  endpoints: string[];
  allowed_origins?: string[];
  expires_at?: string | null;
  project_id?: string | null;
  rate_limit?: { per_minute?: number; per_day?: number };
}

export interface IssuedKey {
  id: string;
  alias: string;
  plaintext_once: string;
  scope: ApiKeyScope;
}

/** OpenAI 호환 에러 envelope (gateway 거부 시). */
export interface ApiErrorEnvelope {
  error: {
    message: string;
    type: string;
    code: string;
  };
}

/** SDK가 throw하는 에러 — gateway envelope을 보존. */
export class LMmasterApiError extends Error {
  status: number;
  type: string;
  code: string;
  constructor(status: number, type: string, code: string, message: string) {
    super(message);
    this.name = "LMmasterApiError";
    this.status = status;
    this.type = type;
    this.code = code;
  }
}

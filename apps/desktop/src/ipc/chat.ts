// chat IPC — Ollama /api/chat 스트리밍 wrapper.
//
// 정책 (사용자 모델 검증/체험 — 2026-04-30):
// - tauri::ipc::Channel<ChatEvent> per-call 스트림.
// - 외부 웹앱은 gateway /v1/chat/completions (API 키) 사용 — 별개 경로.

import { invoke, Channel } from "@tauri-apps/api/core";

import type { RuntimeKind } from "./catalog";

/**
 * Phase 13'.h.2.e.4 — 사용자 cache_dir에 받은 LlamaCpp 모델 catalog id 리스트.
 * Chat dropdown filter용 — 받은 모델만 노출.
 */
export async function listLocalLlamaCppModels(): Promise<string[]> {
  return invoke<string[]>("list_local_llama_cpp_models");
}

export interface ChatMessage {
  /** "system" / "user" / "assistant". */
  role: "system" | "user" | "assistant";
  content: string;
  /**
   * Phase 13'.h (ADR-0050) — 멀티모달 이미지. base64 인코딩된 string 배열.
   * 미지정 또는 빈 배열이면 텍스트 전용 (기존 호환). Ollama API: messages[i].images.
   * `vision_support: true` 모델만 의미 있음.
   */
  images?: string[];
}

export type ChatEvent =
  | { kind: "delta"; text: string }
  | { kind: "completed"; took_ms: number }
  | { kind: "cancelled" }
  | { kind: "failed"; message: string };

export type ChatOutcome =
  | { kind: "completed" }
  | { kind: "cancelled" }
  | { kind: "failed"; message: string };

// Rust ChatApiError enum과 1:1 대응 (chat/mod.rs #[serde(tag = "kind", rename_all = "kebab-case")]).
export type ChatApiError =
  | { kind: "unsupported-runtime"; runtime: string }
  | { kind: "internal"; message: string }
  | { kind: "llama-server-not-configured" }
  | { kind: "llama-cpp-not-prepared"; message: string }
  | { kind: "llama-server-start-failed"; message: string };

/** ChatApiError에서 사용자 향 한국어 메시지 추출. */
export function chatApiErrorMessage(e: unknown): string {
  const err = e as Partial<ChatApiError>;
  switch (err.kind) {
    case "unsupported-runtime":
      return `지원하지 않는 런타임이에요 (${(err as { runtime?: string }).runtime ?? ""}). 카탈로그에서 Ollama 또는 llama.cpp용 모델인지 확인해 주세요.`;
    case "internal":
      return (err as { message?: string }).message ?? "내부 오류가 났어요. 다시 시도해 볼래요?";
    case "llama-server-not-configured":
      return "llama-server 경로가 설정되지 않았어요. 설정 화면에서 경로를 등록해 주세요.";
    case "llama-cpp-not-prepared":
      return (err as { message?: string }).message ?? "LlamaCpp 모델 파일을 찾지 못했어요. 카탈로그에서 먼저 받아주세요.";
    case "llama-server-start-failed":
      return (err as { message?: string }).message ?? "LlamaCpp 서버를 시작하지 못했어요. 다시 시도해 볼래요?";
    default:
      // Tauri가 message 필드를 가진 Error-like 객체로 래핑하는 경우.
      if (typeof (e as { message?: unknown }).message === "string") {
        return (e as { message: string }).message;
      }
      return "알 수 없는 오류가 났어요. 다시 시도해 볼래요?";
  }
}

/** 채팅 1턴 시작. delta 이벤트가 스트리밍으로 도착. */
export async function startChat(args: {
  runtimeKind: RuntimeKind;
  modelId: string;
  messages: ChatMessage[];
  onEvent: (event: ChatEvent) => void;
}): Promise<ChatOutcome> {
  const channel = new Channel<ChatEvent>();
  channel.onmessage = args.onEvent;
  return invoke<ChatOutcome>("start_chat", {
    runtimeKind: args.runtimeKind,
    modelId: args.modelId,
    messages: args.messages,
    channel,
  });
}

/** 진행 중인 모든 채팅 cancel. */
export async function cancelAllChats(): Promise<void> {
  return invoke<void>("cancel_all_chats");
}

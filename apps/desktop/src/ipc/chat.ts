// chat IPC — Ollama /api/chat 스트리밍 wrapper.
//
// 정책 (사용자 모델 검증/체험 — 2026-04-30):
// - tauri::ipc::Channel<ChatEvent> per-call 스트림.
// - 외부 웹앱은 gateway /v1/chat/completions (API 키) 사용 — 별개 경로.

import { invoke, Channel } from "@tauri-apps/api/core";

import type { RuntimeKind } from "./catalog";

export interface ChatMessage {
  /** "system" / "user" / "assistant". */
  role: "system" | "user" | "assistant";
  content: string;
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

export type ChatApiError =
  | { kind: "unsupported-runtime"; runtime: string }
  | { kind: "internal"; message: string };

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

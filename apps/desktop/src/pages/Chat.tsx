// Chat — 사용자가 카탈로그/측정 끝난 모델을 데스크톱 안에서 바로 채팅으로 검증.
//
// 정책 (2026-04-30 — 모델 검증 / 체험):
// - 외부 웹앱은 gateway /v1/chat/completions (API 키) 사용 — 본 페이지는 별개 경로.
// - Ollama 우선 지원. LM Studio는 v1.x.
// - 단일 turn → 다중 turn (history 유지). 영속 X (localStorage 안 함, 페이지 떠나면 초기화).
// - 모델 선택은 /api/tags (loaded models) + 카탈로그 entry 매칭.
// - 스트리밍: delta event 누적해 마지막 메시지에 append.
// - cancel: 진행 중 메시지 abort, 누적 텍스트는 보존.

import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import {
  cancelAllChats,
  startChat,
  type ChatEvent,
  type ChatMessage,
} from "../ipc/chat";
import {
  getCatalog,
  runtimeModelId,
  type ModelEntry,
  type RuntimeKind,
} from "../ipc/catalog";
import { listRuntimeModels, type RuntimeModelView } from "../ipc/runtimes";

import "./chat.css";

interface DisplayMessage {
  /** UI key. */
  id: string;
  role: "user" | "assistant";
  content: string;
  /** 응답 진행 중 — UI에서 streaming dot. */
  streaming?: boolean;
  /** 마지막 turn took_ms. */
  tookMs?: number;
  /** 실패 시 사용자 향 카피. */
  errorMessage?: string;
}

const DEFAULT_RUNTIME: RuntimeKind = "ollama";

export function ChatPage() {
  const [entries, setEntries] = useState<ModelEntry[]>([]);
  const [localModels, setLocalModels] = useState<RuntimeModelView[]>([]);
  const [selectedRuntimeId, setSelectedRuntimeId] = useState<string>("");
  const [messages, setMessages] = useState<DisplayMessage[]>([]);
  const [input, setInput] = useState("");
  const [running, setRunning] = useState(false);

  const scrollRef = useRef<HTMLDivElement>(null);
  const endRef = useRef<HTMLDivElement>(null);
  // 사용자가 위로 스크롤해서 history 읽는 중이면 auto-scroll 잠금 — ChatGPT/Open WebUI 패턴.
  const [isPinnedToBottom, setIsPinnedToBottom] = useState(true);

  // 1) 카탈로그 + Ollama에 받은 모델 목록 로드.
  useEffect(() => {
    let cancelled = false;
    Promise.all([getCatalog(), listRuntimeModels("ollama")])
      .then(([view, local]) => {
        if (cancelled) return;
        setEntries(view.entries);
        setLocalModels(local);
        // preselect: 사용자가 catalog drawer에서 보낸 모델 우선.
        let preselect: string | null = null;
        try {
          preselect = window.localStorage.getItem("lmmaster.chat.preselect");
          if (preselect) {
            window.localStorage.removeItem("lmmaster.chat.preselect");
          }
        } catch {
          /* ignore */
        }
        if (preselect) {
          setSelectedRuntimeId(preselect);
        } else if (local.length > 0) {
          // 첫 번째 받은 모델 자동 선택.
          setSelectedRuntimeId(local[0]?.id ?? "");
        }
      })
      .catch((e) => {
        if (cancelled) return;
        console.warn("chat: catalog/runtime fetch failed:", e);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // 2) 메시지 변경 시 sentinel로 부드럽게 스크롤 — 단, 사용자가 위로 올린 상태면 잠금 유지.
  //    rAF로 layout 후 실행해 streaming delta로 bubble 높이 자라는 동안 race 없이 따라감.
  useEffect(() => {
    if (!isPinnedToBottom) return;
    const raf = requestAnimationFrame(() => {
      endRef.current?.scrollIntoView({ behavior: "auto", block: "end" });
    });
    return () => cancelAnimationFrame(raf);
  }, [messages, isPinnedToBottom]);

  // 3) 사용자 스크롤 추적 — 맨 아래에서 100px 안이면 pinned 유지, 위로 올리면 해제.
  const handleScroll = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    setIsPinnedToBottom(distanceFromBottom < 100);
  }, []);

  /** "맨 아래로 가기" 클릭 — pinned 상태 복구 + 즉시 스크롤. */
  const handleScrollToBottom = useCallback(() => {
    setIsPinnedToBottom(true);
    endRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, []);

  // 3) 페이지 떠날 때 진행 중 채팅 취소 — backend resource 정리.
  useEffect(() => {
    return () => {
      void cancelAllChats().catch(() => {});
    };
  }, []);

  /** 카탈로그 entry → runtime model id (Ollama 형식). */
  const runtimeIdsByModelId = useMemo(() => {
    const map = new Map<string, string>();
    for (const e of entries) {
      const id = runtimeModelId(e, null, DEFAULT_RUNTIME);
      if (id) map.set(e.id, id);
    }
    return map;
  }, [entries]);

  /** 카탈로그 entry 중 사용자가 받은 (local에 있는) 것만 필터. */
  const availableModels = useMemo(() => {
    const localNames = new Set(localModels.map((m) => m.id));
    const list: { id: string; runtimeId: string; displayName: string }[] = [];
    for (const e of entries) {
      const rid = runtimeIdsByModelId.get(e.id);
      if (rid && localNames.has(rid)) {
        list.push({ id: e.id, runtimeId: rid, displayName: e.display_name });
      }
    }
    // 카탈로그에 없는 모델도 표시 — 사용자가 직접 받은 community 모델.
    const catalogRuntimeIds = new Set(list.map((x) => x.runtimeId));
    for (const m of localModels) {
      if (!catalogRuntimeIds.has(m.id)) {
        list.push({
          id: m.id,
          runtimeId: m.id,
          displayName: m.id,
        });
      }
    }
    return list;
  }, [entries, localModels, runtimeIdsByModelId]);

  const handleSend = useCallback(async () => {
    const text = input.trim();
    if (!text || !selectedRuntimeId || running) return;
    setInput("");
    const userMsg: DisplayMessage = {
      id: crypto.randomUUID(),
      role: "user",
      content: text,
    };
    const assistantMsg: DisplayMessage = {
      id: crypto.randomUUID(),
      role: "assistant",
      content: "",
      streaming: true,
    };
    setMessages((prev) => [...prev, userMsg, assistantMsg]);
    setRunning(true);

    // backend ChatMessage[]로 변환 — system 프롬프트는 v1에서 default 없음.
    const history: ChatMessage[] = messages
      .filter((m) => !m.streaming && !m.errorMessage)
      .map((m) => ({ role: m.role, content: m.content }));
    const turn: ChatMessage[] = [...history, { role: "user", content: text }];

    try {
      const outcome = await startChat({
        runtimeKind: DEFAULT_RUNTIME,
        modelId: selectedRuntimeId,
        messages: turn,
        onEvent: (e: ChatEvent) => {
          setMessages((prev) => mergeChatEvent(prev, assistantMsg.id, e));
        },
      });
      if (outcome.kind === "failed") {
        setMessages((prev) =>
          prev.map((m) =>
            m.id === assistantMsg.id
              ? {
                  ...m,
                  streaming: false,
                  errorMessage: outcome.message,
                }
              : m,
          ),
        );
      }
    } catch (e) {
      const msg =
        (e as { message?: string }).message ??
        (e as { kind?: string; runtime?: string }).runtime ??
        String(e);
      setMessages((prev) =>
        prev.map((m) =>
          m.id === assistantMsg.id
            ? { ...m, streaming: false, errorMessage: msg }
            : m,
        ),
      );
    } finally {
      setRunning(false);
    }
  }, [input, selectedRuntimeId, running, messages]);

  const handleStop = useCallback(async () => {
    try {
      await cancelAllChats();
    } catch (e) {
      console.warn("cancelAllChats failed:", e);
    }
  }, []);

  const handleClear = useCallback(() => {
    setMessages([]);
  }, []);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // Enter로 전송, Shift+Enter로 줄바꿈.
      if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
        e.preventDefault();
        void handleSend();
      }
    },
    [handleSend],
  );

  return (
    <div className="chat-root">
      <header className="chat-header">
        <h2 className="chat-title">채팅</h2>
        <p className="chat-subtitle">
          받아둔 모델로 바로 대화해 볼 수 있어요. 외부 웹앱은 로컬 API에서 따로 연결해 주세요.
        </p>
      </header>

      <div className="chat-toolbar" role="toolbar" aria-label="채팅 설정">
        <label className="chat-model-select">
          <span className="chat-label">모델</span>
          <select
            value={selectedRuntimeId}
            onChange={(e) => setSelectedRuntimeId(e.target.value)}
            disabled={running}
          >
            {availableModels.length === 0 ? (
              <option value="">받은 모델이 없어요</option>
            ) : (
              availableModels.map((m) => (
                <option key={m.runtimeId} value={m.runtimeId}>
                  {m.displayName}
                </option>
              ))
            )}
          </select>
        </label>
        <button
          type="button"
          className="chat-action"
          onClick={handleClear}
          disabled={messages.length === 0 || running}
        >
          처음부터 다시
        </button>
      </div>

      {availableModels.length === 0 && (
        <div className="chat-empty" role="status">
          <p className="chat-empty-text">
            아직 받은 Ollama 모델이 없어요. 카탈로그에서 모델을 받고 다시 와주세요.
          </p>
        </div>
      )}

      <div className="chat-stream-wrap">
        <section
          className="chat-stream"
          aria-live="polite"
          aria-label="대화 내용"
          ref={scrollRef}
          onScroll={handleScroll}
        >
          {messages.length === 0 ? (
            <p className="chat-placeholder">
              메시지를 입력해서 대화를 시작해 볼래요?
            </p>
          ) : (
            messages.map((m) => (
              <div
                key={m.id}
                className={`chat-bubble is-${m.role}${
                  m.errorMessage ? " is-error" : ""
                }`}
                data-testid={`chat-bubble-${m.role}`}
              >
                <div className="chat-bubble-meta">
                  <span className="chat-bubble-role">
                    {m.role === "user" ? "나" : "모델"}
                  </span>
                  {m.tookMs != null && (
                    <span className="chat-bubble-took num">
                      {(m.tookMs / 1000).toFixed(1)}초
                    </span>
                  )}
                </div>
                {m.errorMessage ? (
                  <p className="chat-bubble-text chat-bubble-error">
                    {m.errorMessage}
                  </p>
                ) : (
                  <p className="chat-bubble-text">
                    {m.content}
                    {m.streaming && <span className="chat-cursor" aria-hidden />}
                  </p>
                )}
              </div>
            ))
          )}
          {/* 자동 스크롤 sentinel — scrollIntoView 대상. 마지막 bubble 뒤에 위치해
             streaming delta가 들어와 bubble 높이 자라도 항상 그 끝까지 따라감. */}
          <div ref={endRef} aria-hidden style={{ height: 1 }} />
        </section>
        {!isPinnedToBottom && messages.length > 0 && (
          <button
            type="button"
            className="chat-scroll-to-bottom"
            onClick={handleScrollToBottom}
            aria-label="맨 아래로 이동"
          >
            맨 아래로 ↓
          </button>
        )}
      </div>

      <footer className="chat-input-row">
        <textarea
          className="chat-input"
          placeholder="메시지를 입력해 주세요. Enter로 보내고, Shift+Enter로 줄바꿈할 수 있어요."
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={!selectedRuntimeId}
          rows={3}
        />
        <div className="chat-input-actions">
          {running ? (
            <button
              type="button"
              className="chat-action is-stop"
              onClick={handleStop}
            >
              그만할게요
            </button>
          ) : (
            <button
              type="button"
              className="chat-action is-primary"
              onClick={handleSend}
              disabled={!input.trim() || !selectedRuntimeId}
            >
              보낼게요
            </button>
          )}
        </div>
      </footer>
    </div>
  );
}

function mergeChatEvent(
  prev: DisplayMessage[],
  targetId: string,
  event: ChatEvent,
): DisplayMessage[] {
  switch (event.kind) {
    case "delta":
      return prev.map((m) =>
        m.id === targetId
          ? { ...m, content: m.content + event.text, streaming: true }
          : m,
      );
    case "completed":
      return prev.map((m) =>
        m.id === targetId
          ? { ...m, streaming: false, tookMs: event.took_ms }
          : m,
      );
    case "cancelled":
      return prev.map((m) =>
        m.id === targetId ? { ...m, streaming: false } : m,
      );
    case "failed":
      return prev.map((m) =>
        m.id === targetId
          ? { ...m, streaming: false, errorMessage: event.message }
          : m,
      );
  }
}

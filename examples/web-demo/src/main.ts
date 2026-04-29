// 데모 웹앱 — @lmmaster/sdk 단일 의존성으로 LMmaster gateway에 호출.
//
// 정책 (ADR-0022 §9):
// - SDK 외 의존성 없음.
// - baseUrl + apiKey 사용자 입력. 한국어 streaming 응답을 그대로 표시.
// - 게이트웨이 미실행 / 키 없음 / 모델 없음 등 친절한 한국어 에러.

import {
  LMmasterApiError,
  LMmasterClient,
  pingHealth,
  streamChat,
} from "@lmmaster/sdk";

const baseUrlEl = document.getElementById("base-url") as HTMLInputElement;
const apiKeyEl = document.getElementById("api-key") as HTMLInputElement;
const modelEl = document.getElementById("model") as HTMLInputElement;
const promptEl = document.getElementById("prompt") as HTMLTextAreaElement;
const sendBtn = document.getElementById("send") as HTMLButtonElement;
const responseEl = document.getElementById("response") as HTMLDivElement;
const errorEl = document.getElementById("error") as HTMLParagraphElement;
const statusEl = document.getElementById("status") as HTMLDivElement;
const statusText = document.getElementById("status-text") as HTMLSpanElement;

function setStatus(state: "checking" | "ready" | "error", text: string) {
  statusEl.dataset.state = state;
  statusText.textContent = text;
}

function makeClient(): LMmasterClient {
  return new LMmasterClient({
    baseUrl: baseUrlEl.value.trim(),
    apiKey: apiKeyEl.value.trim() || undefined,
  });
}

async function refreshStatus() {
  setStatus("checking", "게이트웨이를 찾고 있어요…");
  const c = makeClient();
  const health = await pingHealth(c);
  if (health) {
    setStatus("ready", `게이트웨이 v${health.version} 사용 중이에요`);
  } else {
    setStatus(
      "error",
      "게이트웨이가 응답하지 않아요. LMmaster 데스크톱이 실행 중인지 확인해 주세요.",
    );
  }
}

async function send() {
  errorEl.textContent = "";
  responseEl.textContent = "";
  sendBtn.disabled = true;

  const c = makeClient();
  try {
    for await (const chunk of streamChat(c, {
      model: modelEl.value.trim(),
      messages: [{ role: "user", content: promptEl.value }],
      stream: true,
    })) {
      const piece = chunk.choices[0]?.delta?.content ?? "";
      responseEl.textContent += piece;
    }
  } catch (e) {
    if (e instanceof LMmasterApiError) {
      errorEl.textContent = `[${e.code}] ${e.message}`;
    } else {
      errorEl.textContent = `예상치 못한 오류가 났어요: ${(e as Error).message}`;
    }
  } finally {
    sendBtn.disabled = false;
  }
}

baseUrlEl.addEventListener("change", refreshStatus);
sendBtn.addEventListener("click", send);

void refreshStatus();

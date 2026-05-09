// Phase 8'.c.4 (ADR-0066) — 발급 후 reveal step + "이렇게 쓰세요" 동적 가이드.
//
// 정책:
// - 키 평문은 8초 자동 마스크 (기존 정책 보존, ADR-0022 §10).
// - "이렇게 쓰세요" 섹션은 닫을 때까지 노출 — 사용자가 base URL + 모델 ID + curl 예시를 옮겨 적을 시간 필요.
// - network_scope 분기:
//   - localhost: 127.0.0.1만.
//   - lan: 127.0.0.1 + 자동 감지 LAN IPs.
//   - any: 127.0.0.1 + "외부 URL은 사용자가 셋업 후 직접 사용" 안내.
// - 5분 auto-close (기존).
// - 복사 버튼 4종: 키 / 이 PC URL / 모델 ID / curl 전체.

import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

type CodeTab = "curl" | "js";

import type { NetworkScope } from "../../ipc/keys";

const AUTOMASK_SECONDS = 8;
const AUTOCLOSE_SECONDS = 300;

export interface ApiKeyRevealStepProps {
  /** 1회만 노출되는 평문 키. */
  plaintext: string;
  /** 키 prefix — masked 단계에서 표시용. */
  keyPrefix: string;
  /** "어디서 호출?" 라디오에서 선택한 의도. */
  networkScope: NetworkScope;
  /** 게이트웨이 포트 — 호출 URL 조립. null이면 "<포트>" placeholder. */
  gatewayPort: number | null;
  /** 자동 감지된 LAN IPs (RFC 1918 private). lan scope에서만 노출. */
  lanIps: string[];
  /** 예시 모델 ID — curl 예시에 삽입. 빈 문자열이면 placeholder. */
  modelExample: string;
  onClose: () => void;
  closeRef: React.RefObject<HTMLButtonElement>;
}

export function ApiKeyRevealStep({
  plaintext,
  keyPrefix,
  networkScope,
  gatewayPort,
  lanIps,
  modelExample,
  onClose,
  closeRef,
}: ApiKeyRevealStepProps) {
  const { t } = useTranslation();
  const [maskedAt, setMaskedAt] = useState<number | null>(null);
  const [copiedTarget, setCopiedTarget] = useState<string | null>(null);
  const [secondsLeft, setSecondsLeft] = useState(AUTOMASK_SECONDS);
  const [codeTab, setCodeTab] = useState<CodeTab>("curl");
  const [showTrouble, setShowTrouble] = useState(false);

  // 8초 카운트다운 + auto-mask.
  useEffect(() => {
    if (maskedAt !== null) return;
    if (secondsLeft <= 0) {
      setMaskedAt(Date.now());
      return;
    }
    const id = window.setTimeout(() => setSecondsLeft(secondsLeft - 1), 1000);
    return () => window.clearTimeout(id);
  }, [secondsLeft, maskedAt]);

  // 5분 auto-close.
  useEffect(() => {
    const id = window.setTimeout(onClose, AUTOCLOSE_SECONDS * 1000);
    return () => window.clearTimeout(id);
  }, [onClose]);

  const handleCopy = useCallback(
    async (target: string, value: string) => {
      try {
        await navigator.clipboard.writeText(value);
        setCopiedTarget(target);
        window.setTimeout(() => setCopiedTarget(null), 2000);
      } catch (e) {
        console.warn("clipboard write failed:", e);
      }
    },
    [],
  );

  const masked = maskedAt !== null;
  const display = masked
    ? plaintext.slice(0, 11) + "·".repeat(Math.max(0, plaintext.length - 11))
    : plaintext;

  // Base URL 조립.
  const portText = gatewayPort != null ? `:${gatewayPort}` : ":<포트>";
  const localhostBaseUrl = `http://127.0.0.1${portText}/v1`;
  const showLanUrls = networkScope === "lan" && lanIps.length > 0;
  const primaryLanUrl = showLanUrls ? `http://${lanIps[0]}${portText}/v1` : null;

  // curl 예시 — 우선 LAN URL, 없으면 localhost. 닫혔는데 평문이 마스크된 상태에서도 "<키>" placeholder.
  const exampleBase = primaryLanUrl ?? localhostBaseUrl;
  const exampleKey = masked ? "<키>" : plaintext;
  const exampleModel = modelExample.length > 0 ? modelExample : "qwen-3-30b-a3b";
  const curlExample = [
    `curl ${exampleBase}/chat/completions \\`,
    `  -H "Authorization: Bearer ${exampleKey}" \\`,
    `  -H "Content-Type: application/json" \\`,
    `  -d '{"model":"${exampleModel}","messages":[{"role":"user","content":"안녕"}]}'`,
  ].join("\n");

  // JavaScript fetch 예시.
  const jsExample = [
    `const res = await fetch("${exampleBase}/chat/completions", {`,
    `  method: "POST",`,
    `  headers: {`,
    `    "Authorization": "Bearer ${exampleKey}",`,
    `    "Content-Type": "application/json",`,
    `  },`,
    `  body: JSON.stringify({`,
    `    model: "${exampleModel}",`,
    `    messages: [{ role: "user", content: "안녕" }],`,
    `  }),`,
    `});`,
    `const data = await res.json();`,
    `console.log(data.choices[0].message.content);`,
  ].join("\n");

  return (
    <div className="keys-modal-backdrop" role="presentation">
      <div
        className="keys-modal keys-reveal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="keys-reveal-title"
      >
        <header className="keys-modal-header">
          <h3 id="keys-reveal-title" className="keys-modal-title">
            {t("keys.modal.revealTitle")}
          </h3>
        </header>
        <div className="keys-modal-body">
          <p className="keys-reveal-body">{t("keys.modal.revealBody")}</p>
          <div className="keys-reveal-key num" data-testid="keys-reveal-key">
            {display}
          </div>
          <div className="keys-reveal-actions">
            <button
              type="button"
              className="keys-button-secondary"
              onClick={() => void handleCopy("key", plaintext)}
              disabled={masked}
              data-testid="keys-reveal-copy-key"
            >
              {copiedTarget === "key"
                ? t("keys.modal.revealCopied")
                : t("keys.modal.revealCopy")}
            </button>
            {!masked && (
              <span className="keys-reveal-countdown" aria-live="polite">
                {t("keys.modal.revealAutomask", { seconds: secondsLeft })}
              </span>
            )}
          </div>

          {/* Phase 8'.c.4 — "이렇게 쓰세요" 가이드 */}
          <section
            className="keys-reveal-guide"
            data-testid="keys-reveal-guide"
            aria-labelledby="keys-reveal-guide-title"
          >
            <h4
              id="keys-reveal-guide-title"
              className="keys-reveal-guide-title"
            >
              {t("keys.modal.guide.title")}
            </h4>

            {/* Base URL (this PC) */}
            <GuideRow
              label={t("keys.modal.guide.baseUrlLocalhost")}
              value={localhostBaseUrl}
              onCopy={() => void handleCopy("baseLocalhost", localhostBaseUrl)}
              copied={copiedTarget === "baseLocalhost"}
              testId="keys-reveal-base-localhost"
              t={t}
            />

            {/* Base URL (LAN) — network_scope=lan일 때만 */}
            {showLanUrls &&
              lanIps.map((ip) => {
                const url = `http://${ip}${portText}/v1`;
                return (
                  <GuideRow
                    key={ip}
                    label={t("keys.modal.guide.baseUrlLan")}
                    value={url}
                    onCopy={() => void handleCopy(`baseLan-${ip}`, url)}
                    copied={copiedTarget === `baseLan-${ip}`}
                    testId={`keys-reveal-base-lan-${ip}`}
                    t={t}
                  />
                );
              })}

            {networkScope === "any" && (
              <p
                className="keys-reveal-note"
                data-testid="keys-reveal-any-note"
              >
                {t("keys.modal.guide.anyNote")}
              </p>
            )}

            {/* Header */}
            <div
              className="keys-reveal-row"
              data-testid="keys-reveal-header-row"
            >
              <span className="keys-reveal-label">
                {t("keys.modal.guide.headerLabel")}
              </span>
              <code className="keys-reveal-value num">
                Authorization: Bearer {keyPrefix}…
              </code>
            </div>

            {/* Model ID */}
            <GuideRow
              label={t("keys.modal.guide.modelLabel")}
              value={exampleModel}
              onCopy={() => void handleCopy("model", exampleModel)}
              copied={copiedTarget === "model"}
              testId="keys-reveal-model"
              t={t}
            />

            {/* 코드 예시 — curl / JS 탭 */}
            <div className="keys-reveal-curl-block">
              <div className="keys-reveal-code-tabs" role="tablist" aria-label="코드 예시">
                <button
                  type="button"
                  role="tab"
                  aria-selected={codeTab === "curl"}
                  className={`keys-reveal-tab${codeTab === "curl" ? " is-active" : ""}`}
                  onClick={() => setCodeTab("curl")}
                >
                  curl
                </button>
                <button
                  type="button"
                  role="tab"
                  aria-selected={codeTab === "js"}
                  className={`keys-reveal-tab${codeTab === "js" ? " is-active" : ""}`}
                  onClick={() => setCodeTab("js")}
                >
                  JavaScript
                </button>
                <button
                  type="button"
                  className="keys-button-secondary keys-reveal-copy-btn"
                  style={{ marginLeft: "auto" }}
                  onClick={() =>
                    void handleCopy(
                      "code",
                      codeTab === "curl" ? curlExample : jsExample,
                    )
                  }
                  data-testid="keys-reveal-copy-curl"
                >
                  {copiedTarget === "code"
                    ? t("keys.modal.guide.copied")
                    : t("keys.modal.guide.copyAll")}
                </button>
              </div>
              <pre
                className="keys-reveal-curl num"
                data-testid="keys-reveal-curl-block"
                role="tabpanel"
              >
                {codeTab === "curl" ? curlExample : jsExample}
              </pre>
              {masked && (
                <p
                  className="keys-reveal-note"
                  data-testid="keys-reveal-masked-note"
                >
                  {t("keys.modal.guide.maskedNote")}
                </p>
              )}
            </div>

            {/* 오류 체크리스트 — 토글 */}
            <div className="keys-reveal-trouble">
              <button
                type="button"
                className="keys-reveal-trouble-toggle"
                aria-expanded={showTrouble}
                onClick={() => setShowTrouble((v) => !v)}
              >
                {showTrouble ? "▾" : "▸"} 연결이 안 될 때 확인하세요
              </button>
              {showTrouble && (
                <ol className="keys-reveal-trouble-list">
                  <li>
                    <strong>포트가 맞나요?</strong>{" "}
                    하단 상태바의 포트 번호와 URL의 포트가 같아야 해요.
                    현재 포트: <code className="num">{gatewayPort ?? "?"}</code>
                  </li>
                  <li>
                    <strong>CORS 오류?</strong>{" "}
                    키 발급 시 허용한 ORIGIN 주소와 브라우저 주소창이 정확히 같아야 해요
                    (예: <code>http://localhost:3000</code>).
                  </li>
                  <li>
                    <strong>401 인증 오류?</strong>{" "}
                    헤더가 <code>Authorization: Bearer &lt;키&gt;</code> 형식인지 확인해요.
                    공백·대소문자 주의.
                  </li>
                  <li>
                    <strong>모델을 못 찾나요?</strong>{" "}
                    <code>model</code> 값이 Ollama에 받아둔 모델 ID와 정확히 일치해야 해요.
                    <code>ollama list</code>로 확인할 수 있어요.
                  </li>
                  <li>
                    <strong>Ollama가 꺼져 있나요?</strong>{" "}
                    LMmaster 하단에 "사용 가능" 표시가 있어야 Ollama가 실행 중인 거예요.
                  </li>
                </ol>
              )}
            </div>
          </section>
        </div>
        <footer className="keys-modal-footer">
          <button
            ref={closeRef}
            type="button"
            className="keys-button-primary"
            onClick={onClose}
          >
            {t("keys.modal.revealClose")}
          </button>
        </footer>
      </div>
    </div>
  );
}

interface GuideRowProps {
  label: string;
  value: string;
  onCopy: () => void;
  copied: boolean;
  testId: string;
  t: (key: string) => string;
}

function GuideRow({ label, value, onCopy, copied, testId, t }: GuideRowProps) {
  return (
    <div className="keys-reveal-row" data-testid={testId}>
      <span className="keys-reveal-label">{label}</span>
      <code className="keys-reveal-value num">{value}</code>
      <button
        type="button"
        className="keys-button-secondary keys-reveal-copy-btn"
        onClick={onCopy}
        aria-label={t("keys.modal.guide.copy")}
      >
        {copied ? t("keys.modal.guide.copied") : t("keys.modal.guide.copy")}
      </button>
    </div>
  );
}

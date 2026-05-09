// RemoteEndpointsPanel — 원격 LMmaster 연결 관리.
//
// 정책:
// - 연결 목록(카드), 추가 폼, 연결 테스트, 삭제.
// - 테스트 성공 시 사용 가능한 모델 목록 인라인 표시.
// - 추가 시 자동으로 테스트 실행 → 실패해도 저장 가능 (URL이 나중에 켜질 수 있으므로).
// - 연결 정보는 Rust settings.json에 저장 — 앱 재시작 후에도 유지.

import { useCallback, useEffect, useState } from "react";
import { Wifi, WifiOff, Plus, Trash2, CheckCircle2, AlertCircle, Loader2 } from "lucide-react";

import {
  addRemoteEndpoint,
  listRemoteEndpoints,
  removeRemoteEndpoint,
  testRemoteEndpoint,
  type RemoteEndpoint,
} from "../../ipc/remote-endpoints";

import "./remote-endpoints.css";

type TestState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "success"; models: string[] }
  | { kind: "error"; message: string };

// ── 연결 카드 ─────────────────────────────────────────────────────────

function EndpointCard({
  ep,
  onRemove,
}: {
  ep: RemoteEndpoint;
  onRemove: (id: string) => void;
}) {
  const [test, setTest] = useState<TestState>({ kind: "idle" });
  const [showModels, setShowModels] = useState(false);

  const handleTest = useCallback(async () => {
    setTest({ kind: "loading" });
    try {
      const models = await testRemoteEndpoint({
        base_url: ep.base_url,
        api_key: ep.api_key,
      });
      setTest({ kind: "success", models });
      setShowModels(true);
    } catch (e) {
      const err = e as { message?: string };
      setTest({
        kind: "error",
        message: err.message ?? "연결에 실패했어요.",
      });
    }
  }, [ep.base_url, ep.api_key]);

  const handleRemove = useCallback(() => {
    if (!window.confirm(`"${ep.alias}" 연결을 삭제할게요. 이 서버의 모델은 채팅에서 사라져요.`)) return;
    onRemove(ep.id);
  }, [ep, onRemove]);

  return (
    <div className="remote-card" data-testid={`remote-card-${ep.id}`}>
      <div className="remote-card-header">
        <div className="remote-card-title-row">
          <Wifi size={16} aria-hidden="true" className="remote-card-icon" />
          <span className="remote-card-alias">{ep.alias}</span>
        </div>
        <div className="remote-card-actions">
          <button
            type="button"
            className="remote-btn remote-btn-secondary"
            onClick={handleTest}
            disabled={test.kind === "loading"}
            data-testid={`remote-test-${ep.id}`}
          >
            {test.kind === "loading" ? (
              <Loader2 size={13} className="remote-spin" aria-hidden="true" />
            ) : null}
            {test.kind === "loading" ? "연결 확인 중…" : "연결 테스트"}
          </button>
          <button
            type="button"
            className="remote-btn remote-btn-danger"
            onClick={handleRemove}
            aria-label={`${ep.alias} 연결 삭제`}
            data-testid={`remote-remove-${ep.id}`}
          >
            <Trash2 size={13} aria-hidden="true" />
          </button>
        </div>
      </div>

      <p className="remote-card-url num">{ep.base_url}</p>
      <p className="remote-card-meta">
        키: <code className="num">{ep.api_key.slice(0, 11)}…</code>
        &nbsp;·&nbsp;등록: {ep.created_at.slice(0, 10)}
      </p>

      {test.kind === "success" && (
        <div className="remote-test-result is-success" role="status">
          <CheckCircle2 size={14} aria-hidden="true" />
          <span>연결됐어요 — 모델 {test.models.length}개</span>
          <button
            type="button"
            className="remote-btn-link"
            onClick={() => setShowModels((v) => !v)}
          >
            {showModels ? "접기" : "보기"}
          </button>
        </div>
      )}
      {test.kind === "error" && (
        <div className="remote-test-result is-error" role="alert">
          <WifiOff size={14} aria-hidden="true" />
          <span>{test.message}</span>
        </div>
      )}
      {test.kind === "success" && showModels && (
        <ul className="remote-model-list" aria-label="사용 가능한 모델">
          {test.models.map((m) => (
            <li key={m} className="remote-model-item num">
              {m}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

// ── 추가 폼 ────────────────────────────────────────────────────────────

function AddEndpointForm({ onAdded }: { onAdded: (ep: RemoteEndpoint) => void }) {
  const [open, setOpen] = useState(false);
  const [alias, setAlias] = useState("");
  const [baseUrl, setBaseUrl] = useState("http://");
  const [apiKey, setApiKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<string[] | null>(null);

  const reset = () => {
    setAlias("");
    setBaseUrl("http://");
    setApiKey("");
    setError(null);
    setTestResult(null);
    setOpen(false);
  };

  const handleSubmit = useCallback(async () => {
    if (!alias.trim()) { setError("별명을 입력해 주세요."); return; }
    if (!baseUrl.trim() || !baseUrl.startsWith("http")) {
      setError("올바른 URL을 입력해 주세요. (예: http://192.168.1.10:14964/v1)");
      return;
    }
    if (!apiKey.trim()) { setError("API 키를 입력해 주세요."); return; }

    setBusy(true);
    setError(null);

    // 저장 먼저, 테스트는 후속.
    try {
      const ep = await addRemoteEndpoint({
        alias: alias.trim(),
        base_url: baseUrl.trim(),
        api_key: apiKey.trim(),
      });
      onAdded(ep);
      reset();
      // 저장 후 테스트.
      try {
        const models = await testRemoteEndpoint({ base_url: ep.base_url, api_key: ep.api_key });
        setTestResult(models);
      } catch {
        // 테스트 실패해도 저장은 완료됨 — 별도 안내 없이 카드에서 재테스트 가능.
      }
    } catch (e) {
      const err = e as { message?: string };
      setError(err.message ?? "저장에 실패했어요.");
    } finally {
      setBusy(false);
    }
  }, [alias, baseUrl, apiKey, onAdded]);

  if (!open) {
    return (
      <button
        type="button"
        className="remote-btn remote-btn-primary remote-add-trigger"
        onClick={() => setOpen(true)}
        data-testid="remote-add-open"
      >
        <Plus size={14} aria-hidden="true" />
        원격 연결 추가
      </button>
    );
  }

  return (
    <div className="remote-form" data-testid="remote-add-form">
      <h4 className="remote-form-title">원격 연결 추가</h4>

      <label className="remote-form-label">
        별명 <span className="remote-form-hint">(예: "진우 PC")</span>
        <input
          type="text"
          className="remote-form-input"
          value={alias}
          onChange={(e) => setAlias(e.target.value)}
          placeholder="팀원 PC"
          data-testid="remote-form-alias"
          autoFocus
        />
      </label>

      <label className="remote-form-label">
        Base URL{" "}
        <span className="remote-form-hint">
          (사용자 A의 LMmaster 하단 포트 확인. /v1 포함)
        </span>
        <input
          type="text"
          className="remote-form-input num"
          value={baseUrl}
          onChange={(e) => setBaseUrl(e.target.value)}
          placeholder="http://192.168.1.10:14964/v1"
          data-testid="remote-form-url"
        />
      </label>

      <label className="remote-form-label">
        API 키{" "}
        <span className="remote-form-hint">
          (사용자 A가 LAN 범위로 발급한 키)
        </span>
        <input
          type="text"
          className="remote-form-input num"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          placeholder="lm-…"
          data-testid="remote-form-key"
        />
      </label>

      {error && (
        <p className="remote-form-error" role="alert">
          <AlertCircle size={13} aria-hidden="true" />
          {error}
        </p>
      )}

      <div className="remote-form-actions">
        <button
          type="button"
          className="remote-btn remote-btn-secondary"
          onClick={reset}
          disabled={busy}
        >
          취소
        </button>
        <button
          type="button"
          className="remote-btn remote-btn-primary"
          onClick={handleSubmit}
          disabled={busy}
          data-testid="remote-form-submit"
        >
          {busy ? (
            <Loader2 size={13} className="remote-spin" aria-hidden="true" />
          ) : null}
          {busy ? "저장 중…" : "저장하고 연결할게요"}
        </button>
      </div>
    </div>
  );
}

// ── 패널 진입점 ─────────────────────────────────────────────────────────

export function RemoteEndpointsPanel() {
  const [endpoints, setEndpoints] = useState<RemoteEndpoint[]>([]);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const list = await listRemoteEndpoints();
      setEndpoints(list);
    } catch (e) {
      console.warn("listRemoteEndpoints failed:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleAdded = useCallback((ep: RemoteEndpoint) => {
    setEndpoints((prev) => [...prev, ep]);
  }, []);

  const handleRemove = useCallback(async (id: string) => {
    try {
      await removeRemoteEndpoint(id);
      setEndpoints((prev) => prev.filter((e) => e.id !== id));
    } catch (e) {
      console.warn("removeRemoteEndpoint failed:", e);
    }
  }, []);

  return (
    <section className="remote-panel" aria-labelledby="remote-panel-title">
      <header className="remote-panel-header">
        <div>
          <h3 id="remote-panel-title" className="remote-panel-title">
            원격 연결
          </h3>
          <p className="remote-panel-subtitle">
            다른 PC의 LMmaster가 실행 중인 모델을 빌려 쓸 수 있어요.
            상대방이 <strong>LAN API 키</strong>를 발급해서 전달해 주면 등록해 보세요.
          </p>
        </div>
      </header>

      <div className="remote-how-to">
        <div className="remote-how-to-step">
          <span className="remote-how-to-num">1</span>
          <span>사용자 A: 로컬 API 메뉴 → 새 키 만들기 → 허용 범위 <strong>사내망</strong> 선택 후 발급</span>
        </div>
        <div className="remote-how-to-step">
          <span className="remote-how-to-num">2</span>
          <span>사용자 A: LMmaster 하단 <strong>포트 번호</strong> + 키를 사용자 B에게 전달</span>
        </div>
        <div className="remote-how-to-step">
          <span className="remote-how-to-num">3</span>
          <span>사용자 B (여기): 아래 "원격 연결 추가"에 URL과 키 입력 → 채팅에서 모델 사용</span>
        </div>
      </div>

      {loading ? (
        <div className="remote-loading" role="status">
          <Loader2 size={20} className="remote-spin" aria-hidden="true" />
          <span>연결 목록을 불러오고 있어요…</span>
        </div>
      ) : (
        <>
          {endpoints.length === 0 && (
            <div className="remote-empty" role="status">
              <WifiOff size={24} aria-hidden="true" className="remote-empty-icon" />
              <p>아직 등록된 원격 연결이 없어요.</p>
              <p className="remote-empty-hint">
                아래 "원격 연결 추가"를 눌러 다른 PC의 모델을 빌려 써보세요.
              </p>
            </div>
          )}
          <div className="remote-card-list">
            {endpoints.map((ep) => (
              <EndpointCard key={ep.id} ep={ep} onRemove={handleRemove} />
            ))}
          </div>
        </>
      )}

      <AddEndpointForm onAdded={handleAdded} />
    </section>
  );
}

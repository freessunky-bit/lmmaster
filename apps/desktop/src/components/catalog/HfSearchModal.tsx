// HfSearchModal — Phase 11'.c (ADR-0049) HuggingFace 하이브리드 검색 결과 모달.
//
// 정책:
// - 노란 ⚠ 배너 + "지원 외" 배지 — 큐레이션 thesis 보존.
// - role="dialog" + aria-modal + aria-labelledby + Esc/배경 클릭 닫기.
// - "지금 시도해 볼게요" → registerHfModel → onRegistered 콜백 → CustomModelsSection에 자동 노출.
// - "큐레이션 추가 요청" → 시스템 브라우저로 GitHub Issue 폼 open (자동 POST 거부).

import { open as openExternal } from "@tauri-apps/plugin-shell";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  curationRequestUrl,
  registerHfModel,
  searchHfModels,
  type HfSearchHit,
} from "../../ipc/hf_search";
import type { CustomModel } from "../../ipc/workbench";

interface HfSearchModalProps {
  isOpen: boolean;
  query: string;
  onClose: () => void;
  onRegistered?: (model: CustomModel) => void;
}

type SearchState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ok"; hits: HfSearchHit[] }
  | { kind: "error"; message: string };

export function HfSearchModal({
  isOpen,
  query,
  onClose,
  onRegistered,
}: HfSearchModalProps) {
  const { t } = useTranslation();
  const [state, setState] = useState<SearchState>({ kind: "idle" });
  const [busyRepo, setBusyRepo] = useState<string | null>(null);
  const closeBtnRef = useRef<HTMLButtonElement | null>(null);

  // 검색 실행 — query 변경 + isOpen 시. t는 deps 제외 (i18next stable + 무한 루프 회피).
  useEffect(() => {
    if (!isOpen) {
      setState({ kind: "idle" });
      return;
    }
    let cancelled = false;
    setState({ kind: "loading" });
    searchHfModels(query)
      .then((hits) => {
        if (!cancelled) setState({ kind: "ok", hits });
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        const message = extractMessage(
          e,
          "HuggingFace 검색에 실패했어요. 잠시 뒤에 다시 시도해 주세요.",
        );
        setState({ kind: "error", message });
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, query]);

  // 마운트 시 close 버튼에 포커스 — 키보드 사용자 진입점.
  useEffect(() => {
    if (isOpen) closeBtnRef.current?.focus();
  }, [isOpen]);

  // Esc 닫기.
  useEffect(() => {
    if (!isOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [isOpen, onClose]);

  const handleTry = useCallback(
    async (repo: string) => {
      setBusyRepo(repo);
      try {
        const model = await registerHfModel(repo);
        onRegistered?.(model);
        onClose();
      } catch (e) {
        console.warn("registerHfModel failed:", e);
        const message = extractMessage(
          e,
          "등록에 실패했어요. 잠시 뒤에 다시 시도해 주세요.",
        );
        setState({ kind: "error", message });
      } finally {
        setBusyRepo(null);
      }
    },
    [onClose, onRegistered],
  );

  const handleCurationRequest = useCallback(async (repo: string) => {
    try {
      await openExternal(curationRequestUrl(repo));
    } catch (e) {
      console.warn("openExternal failed:", e);
    }
  }, []);

  if (!isOpen) return null;

  return (
    <div
      className="hf-search-modal-backdrop"
      data-testid="hf-search-backdrop"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className="hf-search-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="hf-search-title"
        data-testid="hf-search-modal"
      >
        <header className="hf-search-modal-header">
          <h2 id="hf-search-title" className="hf-search-modal-title">
            {t("catalog.hfSearch.title", "HuggingFace에서 찾았어요")}
          </h2>
          <button
            ref={closeBtnRef}
            type="button"
            className="hf-search-modal-close"
            onClick={onClose}
            aria-label={t("catalog.hfSearch.close", "닫기")}
            data-testid="hf-search-close"
          >
            ×
          </button>
        </header>

        <div
          className="hf-search-modal-banner"
          role="note"
          data-testid="hf-search-banner"
        >
          ⚠{" "}
          {t(
            "catalog.hfSearch.banner",
            "큐레이션 외 모델은 호환성·한국어 강도가 검증되지 않았어요. 도메인 점수도 표시되지 않아요.",
          )}
        </div>

        <div className="hf-search-modal-body">
          {state.kind === "loading" && (
            <p className="hf-search-status" data-testid="hf-search-loading">
              {t("catalog.hfSearch.loading", "검색하고 있어요…")}
            </p>
          )}

          {state.kind === "error" && (
            <p
              className="hf-search-status hf-search-error"
              role="alert"
              data-testid="hf-search-error"
            >
              {state.message}
            </p>
          )}

          {state.kind === "ok" && state.hits.length === 0 && (
            <p className="hf-search-status" data-testid="hf-search-empty">
              {t(
                "catalog.hfSearch.empty",
                "검색 결과가 없어요. 다른 키워드로 시도해 볼래요?",
              )}
            </p>
          )}

          {state.kind === "ok" && state.hits.length > 0 && (
            <ul className="hf-search-hits" data-testid="hf-search-hits">
              {state.hits.map((hit) => (
                <li
                  key={hit.repo}
                  className="hf-search-hit"
                  data-testid={`hf-search-hit-${hit.repo}`}
                >
                  <div className="hf-search-hit-head">
                    <code className="hf-search-hit-repo num">{hit.repo}</code>
                    <span
                      className="hf-search-hit-badge"
                      data-testid="hf-search-unsupported"
                    >
                      {t("catalog.hfSearch.unsupported", "큐레이션 외")}
                    </span>
                  </div>
                  <div className="hf-search-hit-meta num">
                    <span aria-label={t("catalog.hfSearch.downloads", "다운로드")}>
                      ⬇ {formatCount(hit.downloads)}
                    </span>
                    <span aria-label={t("catalog.hfSearch.likes", "좋아요")}>
                      ❤ {formatCount(hit.likes)}
                    </span>
                    <span
                      aria-label={t("catalog.hfSearch.lastModified", "갱신")}
                    >
                      🕒 {formatLastModified(hit.last_modified)}
                    </span>
                    {hit.pipeline_tag && (
                      <span className="hf-search-hit-tag">{hit.pipeline_tag}</span>
                    )}
                  </div>
                  <div className="hf-search-hit-actions">
                    <button
                      type="button"
                      className="hf-search-action hf-search-action-primary"
                      onClick={() => handleTry(hit.repo)}
                      disabled={busyRepo !== null}
                      data-testid={`hf-search-try-${hit.repo}`}
                    >
                      {busyRepo === hit.repo
                        ? t("catalog.hfSearch.registering", "등록하고 있어요…")
                        : t("catalog.hfSearch.tryNow", "지금 시도해 볼게요")}
                    </button>
                    <button
                      type="button"
                      className="hf-search-action hf-search-action-secondary"
                      onClick={() => handleCurationRequest(hit.repo)}
                      data-testid={`hf-search-curate-${hit.repo}`}
                    >
                      {t(
                        "catalog.hfSearch.requestCuration",
                        "큐레이션 추가 요청",
                      )}
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}

function extractMessage(e: unknown, fallback: string): string {
  if (e && typeof e === "object" && "message" in e) {
    const m = (e as { message: unknown }).message;
    if (typeof m === "string" && m.length > 0) return m;
  }
  if (typeof e === "string" && e.length > 0) return e;
  return fallback;
}

function formatCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

function formatLastModified(iso: string): string {
  if (!iso) return "-";
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return iso.slice(0, 10);
  const diff = Date.now() - t;
  if (diff < 24 * 60 * 60_000) return "오늘";
  if (diff < 7 * 24 * 60 * 60_000)
    return `${Math.round(diff / (24 * 60 * 60_000))}일 전`;
  return new Date(t).toISOString().slice(0, 10);
}

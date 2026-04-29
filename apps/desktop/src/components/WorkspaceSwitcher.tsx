// WorkspaceSwitcher — Phase 8'.1.
//
// 사이드바 상단 dropdown — 현재 active workspace 표시 + 전환 + 생성 + 이름변경 + 삭제.
//
// 정책 (ADR-0038, CLAUDE.md §4.3):
// - role="menu" + aria-haspopup="menu" + aria-expanded.
// - Esc / 배경 클릭 / 다른 곳 click으로 dropdown 닫기.
// - 모달은 role="dialog" aria-modal="true" + focus trap (첫 input 자동 focus).
// - 한국어 해요체.
// - design-system tokens.
// - 삭제는 confirmation dialog 의무 — 사용자 인지.

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type FormEvent,
} from "react";
import { useTranslation } from "react-i18next";

import { useActiveWorkspace } from "../contexts/ActiveWorkspaceContext";
import type { WorkspaceInfo, WorkspacesApiError } from "../ipc/workspaces";

import "./workspaceSwitcher.css";

type ModalKind =
  | { kind: "none" }
  | { kind: "create" }
  | { kind: "rename"; target: WorkspaceInfo }
  | { kind: "delete"; target: WorkspaceInfo };

export function WorkspaceSwitcher() {
  const { t } = useTranslation();
  const { active, workspaces, setActive, create, rename, remove } =
    useActiveWorkspace();

  const [open, setOpen] = useState(false);
  const [modal, setModal] = useState<ModalKind>({ kind: "none" });

  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const menuRef = useRef<HTMLDivElement | null>(null);

  // ── Dropdown — Esc / 외부 click 닫기 ─────────────────────────────────
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setOpen(false);
        triggerRef.current?.focus();
      }
    };
    const onClick = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        menuRef.current &&
        !menuRef.current.contains(target) &&
        triggerRef.current &&
        !triggerRef.current.contains(target)
      ) {
        setOpen(false);
      }
    };
    window.addEventListener("keydown", onKey);
    // mousedown으로 잡으면 click 직전에 닫혀서 트리거가 다시 열림. click을 사용.
    window.addEventListener("click", onClick);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("click", onClick);
    };
  }, [open]);

  const handleSelect = useCallback(
    async (id: string) => {
      if (active && active.id === id) {
        setOpen(false);
        return;
      }
      try {
        await setActive(id);
      } catch (e) {
        console.warn("setActive 실패:", e);
      }
      setOpen(false);
      triggerRef.current?.focus();
    },
    [active, setActive],
  );

  const openCreate = useCallback(() => {
    setOpen(false);
    setModal({ kind: "create" });
  }, []);

  const openRename = useCallback((target: WorkspaceInfo) => {
    setOpen(false);
    setModal({ kind: "rename", target });
  }, []);

  const openDelete = useCallback((target: WorkspaceInfo) => {
    setOpen(false);
    setModal({ kind: "delete", target });
  }, []);

  const closeModal = useCallback(() => {
    setModal({ kind: "none" });
    triggerRef.current?.focus();
  }, []);

  // ── 표시명 ───────────────────────────────────────────────────────────
  const displayName = active?.name ?? t("screens.workspaceSwitcher.loading");

  return (
    <div className="workspace-switcher" data-testid="workspace-switcher">
      <button
        ref={triggerRef}
        type="button"
        className="workspace-switcher-trigger"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={t("screens.workspaceSwitcher.triggerAria", {
          name: displayName,
        })}
        data-testid="workspace-switcher-trigger"
        onClick={(e) => {
          // 외부 click handler가 잡지 않도록 stop.
          e.stopPropagation();
          setOpen((v) => !v);
        }}
      >
        <span className="workspace-switcher-trigger-content">
          <span className="workspace-switcher-eyebrow">
            {t("screens.workspaceSwitcher.current")}
          </span>
          <span className="workspace-switcher-name">{displayName}</span>
        </span>
        <span className="workspace-switcher-chevron" aria-hidden="true">
          ▾
        </span>
      </button>

      {open && (
        <div
          ref={menuRef}
          role="menu"
          aria-label={t("screens.workspaceSwitcher.menuAria")}
          className="workspace-switcher-menu"
          data-testid="workspace-switcher-menu"
          onClick={(e) => e.stopPropagation()}
        >
          <div className="workspace-switcher-menu-section" role="none">
            {workspaces.length === 0 && (
              <p
                className="workspace-switcher-empty"
                data-testid="workspace-switcher-empty"
                role="none"
              >
                {t("screens.workspaceSwitcher.empty")}
              </p>
            )}
            {workspaces.map((w) => {
              const isActive = active && active.id === w.id;
              // 한 row = menuitemradio (전환) + 두 menuitem (rename/delete).
              // 외부 div는 role="none"으로 ARIA 트리에서 제외 — 자식 menuitems는 menu의 직접 자손으로 인식.
              return (
                <div
                  key={w.id}
                  className="workspace-switcher-row"
                  role="none"
                >
                  <button
                    type="button"
                    role="menuitemradio"
                    aria-checked={isActive ?? false}
                    aria-current={isActive ? "true" : undefined}
                    className="workspace-switcher-item"
                    data-testid={`workspace-switcher-item-${w.id}`}
                    onClick={() => handleSelect(w.id)}
                  >
                    <span
                      className="workspace-switcher-item-check"
                      aria-hidden="true"
                    >
                      {isActive ? "✓" : ""}
                    </span>
                    <span className="workspace-switcher-item-name">
                      {w.name}
                    </span>
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    className="workspace-switcher-action"
                    onClick={() => openRename(w)}
                    data-testid={`workspace-switcher-rename-${w.id}`}
                    aria-label={t("screens.workspaceSwitcher.renameAria", {
                      name: w.name,
                    })}
                  >
                    {t("screens.workspaceSwitcher.rename")}
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    className="workspace-switcher-action is-danger"
                    onClick={() => openDelete(w)}
                    data-testid={`workspace-switcher-delete-${w.id}`}
                    aria-label={t("screens.workspaceSwitcher.deleteAria", {
                      name: w.name,
                    })}
                    disabled={workspaces.length <= 1}
                  >
                    {t("screens.workspaceSwitcher.delete")}
                  </button>
                </div>
              );
            })}
          </div>

          <div className="workspace-switcher-menu-section" role="none">
            <button
              type="button"
              role="menuitem"
              className="workspace-switcher-create"
              data-testid="workspace-switcher-create"
              onClick={openCreate}
            >
              {t("screens.workspaceSwitcher.create")}
            </button>
          </div>
        </div>
      )}

      {modal.kind === "create" && (
        <CreateModal onClose={closeModal} onCreate={create} />
      )}
      {modal.kind === "rename" && (
        <RenameModal
          target={modal.target}
          onClose={closeModal}
          onRename={rename}
        />
      )}
      {modal.kind === "delete" && (
        <DeleteModal
          target={modal.target}
          onClose={closeModal}
          onDelete={remove}
        />
      )}
    </div>
  );
}

// ── Modals ───────────────────────────────────────────────────────────

interface CreateModalProps {
  onClose: () => void;
  onCreate: (name: string, description?: string) => Promise<WorkspaceInfo>;
}

function CreateModal({ onClose, onCreate }: CreateModalProps) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const handleSubmit = useCallback(
    async (e: FormEvent) => {
      e.preventDefault();
      setError(null);
      const trimmed = name.trim();
      if (trimmed.length === 0) {
        setError(t("screens.workspaceSwitcher.errors.empty"));
        return;
      }
      setSubmitting(true);
      try {
        await onCreate(trimmed, description.trim() || undefined);
        onClose();
      } catch (e) {
        setError(extractKoreanError(e, t));
      } finally {
        setSubmitting(false);
      }
    },
    [description, name, onClose, onCreate, t],
  );

  return (
    <div
      className="workspace-switcher-modal-backdrop"
      role="presentation"
      onClick={onClose}
      data-testid="workspace-switcher-create-modal"
    >
      <form
        role="dialog"
        aria-modal="true"
        aria-labelledby="workspace-switcher-create-title"
        className="workspace-switcher-modal"
        onClick={(e) => e.stopPropagation()}
        onSubmit={handleSubmit}
      >
        <h3
          id="workspace-switcher-create-title"
          className="workspace-switcher-modal-title"
        >
          {t("screens.workspaceSwitcher.createTitle")}
        </h3>

        <label className="workspace-switcher-modal-field">
          <span className="workspace-switcher-modal-label">
            {t("screens.workspaceSwitcher.nameLabel")}
          </span>
          <input
            ref={inputRef}
            type="text"
            className="workspace-switcher-modal-input"
            placeholder={t("screens.workspaceSwitcher.namePlaceholder")}
            value={name}
            onChange={(e) => setName(e.target.value)}
            data-testid="workspace-switcher-create-name"
          />
        </label>

        <label className="workspace-switcher-modal-field">
          <span className="workspace-switcher-modal-label">
            {t("screens.workspaceSwitcher.descriptionLabel")}
          </span>
          <input
            type="text"
            className="workspace-switcher-modal-input"
            placeholder={t("screens.workspaceSwitcher.descriptionPlaceholder")}
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            data-testid="workspace-switcher-create-desc"
          />
        </label>

        {error && (
          <p
            className="workspace-switcher-modal-error"
            role="alert"
            data-testid="workspace-switcher-create-error"
          >
            {error}
          </p>
        )}

        <footer className="workspace-switcher-modal-footer">
          <button
            type="button"
            className="workspace-switcher-modal-button"
            onClick={onClose}
            disabled={submitting}
            data-testid="workspace-switcher-create-cancel"
          >
            {t("screens.workspaceSwitcher.cancel")}
          </button>
          <button
            type="submit"
            className="workspace-switcher-modal-button is-primary"
            disabled={submitting}
            data-testid="workspace-switcher-create-submit"
          >
            {t("screens.workspaceSwitcher.submit")}
          </button>
        </footer>
      </form>
    </div>
  );
}

interface RenameModalProps {
  target: WorkspaceInfo;
  onClose: () => void;
  onRename: (id: string, newName: string) => Promise<void>;
}

function RenameModal({ target, onClose, onRename }: RenameModalProps) {
  const { t } = useTranslation();
  const [name, setName] = useState(target.name);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const handleSubmit = useCallback(
    async (e: FormEvent) => {
      e.preventDefault();
      setError(null);
      const trimmed = name.trim();
      if (trimmed.length === 0) {
        setError(t("screens.workspaceSwitcher.errors.empty"));
        return;
      }
      if (trimmed === target.name) {
        onClose();
        return;
      }
      setSubmitting(true);
      try {
        await onRename(target.id, trimmed);
        onClose();
      } catch (e) {
        setError(extractKoreanError(e, t));
      } finally {
        setSubmitting(false);
      }
    },
    [name, onClose, onRename, t, target.id, target.name],
  );

  return (
    <div
      className="workspace-switcher-modal-backdrop"
      role="presentation"
      onClick={onClose}
      data-testid="workspace-switcher-rename-modal"
    >
      <form
        role="dialog"
        aria-modal="true"
        aria-labelledby="workspace-switcher-rename-title"
        className="workspace-switcher-modal"
        onClick={(e) => e.stopPropagation()}
        onSubmit={handleSubmit}
      >
        <h3
          id="workspace-switcher-rename-title"
          className="workspace-switcher-modal-title"
        >
          {t("screens.workspaceSwitcher.renameTitle")}
        </h3>

        <label className="workspace-switcher-modal-field">
          <span className="workspace-switcher-modal-label">
            {t("screens.workspaceSwitcher.nameLabel")}
          </span>
          <input
            ref={inputRef}
            type="text"
            className="workspace-switcher-modal-input"
            placeholder={t("screens.workspaceSwitcher.namePlaceholder")}
            value={name}
            onChange={(e) => setName(e.target.value)}
            data-testid="workspace-switcher-rename-name"
          />
        </label>

        {error && (
          <p
            className="workspace-switcher-modal-error"
            role="alert"
            data-testid="workspace-switcher-rename-error"
          >
            {error}
          </p>
        )}

        <footer className="workspace-switcher-modal-footer">
          <button
            type="button"
            className="workspace-switcher-modal-button"
            onClick={onClose}
            disabled={submitting}
            data-testid="workspace-switcher-rename-cancel"
          >
            {t("screens.workspaceSwitcher.cancel")}
          </button>
          <button
            type="submit"
            className="workspace-switcher-modal-button is-primary"
            disabled={submitting}
            data-testid="workspace-switcher-rename-submit"
          >
            {t("screens.workspaceSwitcher.submit")}
          </button>
        </footer>
      </form>
    </div>
  );
}

interface DeleteModalProps {
  target: WorkspaceInfo;
  onClose: () => void;
  onDelete: (id: string) => Promise<void>;
}

function DeleteModal({ target, onClose, onDelete }: DeleteModalProps) {
  const { t } = useTranslation();
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const cancelRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    // 안전 default — 취소 버튼에 focus.
    cancelRef.current?.focus();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const handleConfirm = useCallback(async () => {
    setError(null);
    setSubmitting(true);
    try {
      await onDelete(target.id);
      onClose();
    } catch (e) {
      setError(extractKoreanError(e, t));
    } finally {
      setSubmitting(false);
    }
  }, [onClose, onDelete, t, target.id]);

  return (
    <div
      className="workspace-switcher-modal-backdrop"
      role="presentation"
      onClick={onClose}
      data-testid="workspace-switcher-delete-modal"
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="workspace-switcher-delete-title"
        aria-describedby="workspace-switcher-delete-body"
        className="workspace-switcher-modal"
        onClick={(e) => e.stopPropagation()}
      >
        <h3
          id="workspace-switcher-delete-title"
          className="workspace-switcher-modal-title"
        >
          {t("screens.workspaceSwitcher.deleteConfirmTitle", {
            name: target.name,
          })}
        </h3>
        <p
          id="workspace-switcher-delete-body"
          className="workspace-switcher-modal-body"
        >
          {t("screens.workspaceSwitcher.deleteConfirmBody", {
            name: target.name,
          })}
        </p>

        {error && (
          <p
            className="workspace-switcher-modal-error"
            role="alert"
            data-testid="workspace-switcher-delete-error"
          >
            {error}
          </p>
        )}

        <footer className="workspace-switcher-modal-footer">
          <button
            ref={cancelRef}
            type="button"
            className="workspace-switcher-modal-button"
            onClick={onClose}
            disabled={submitting}
            data-testid="workspace-switcher-delete-cancel"
          >
            {t("screens.workspaceSwitcher.deleteCancel")}
          </button>
          <button
            type="button"
            className="workspace-switcher-modal-button is-danger"
            onClick={handleConfirm}
            disabled={submitting}
            data-testid="workspace-switcher-delete-confirm"
          >
            {t("screens.workspaceSwitcher.deleteConfirm")}
          </button>
        </footer>
      </div>
    </div>
  );
}

// ── helpers ──────────────────────────────────────────────────────────

/** WorkspacesApiError를 한국어 메시지로 변환. fallback은 i18n key. */
function extractKoreanError(
  err: unknown,
  t: (key: string, opts?: Record<string, unknown>) => string,
): string {
  if (err && typeof err === "object" && "kind" in err) {
    const e = err as WorkspacesApiError;
    switch (e.kind) {
      case "duplicate-name":
        return t("screens.workspaceSwitcher.errors.duplicate", { name: e.name });
      case "empty-name":
        return t("screens.workspaceSwitcher.errors.empty");
      case "not-found":
        return t("screens.workspaceSwitcher.errors.notFound");
      case "cannot-delete-only-workspace":
        return t("screens.workspaceSwitcher.errors.cannotDeleteOnly");
      case "persist":
        return t("screens.workspaceSwitcher.errors.persist", {
          message: e.message,
        });
      case "internal":
        return t("screens.workspaceSwitcher.errors.internal", {
          message: e.message,
        });
    }
  }
  if (err instanceof Error) return err.message;
  return t("screens.workspaceSwitcher.errors.unknown");
}


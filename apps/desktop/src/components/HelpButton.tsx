// HelpButton — Phase 12'.b. 페이지 헤더에 노출되는 ? 도움말 버튼.
//
// 정책 (phase-8p-9p-10p-residual-plan.md §1.9):
// - 클릭 시 popover (focus trap + Esc + role=dialog + aria-modal=true).
// - 짧은 hint 1~2줄 + "전체 가이드 보기" 링크 → Guide page deep link 진입.
// - design-system token, prefers-reduced-motion 존중.
// - sectionId는 Guide.tsx의 SECTION_IDS와 일치해야 해요.

import { HelpCircle } from "lucide-react";
import {
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";

import "./helpButton.css";

interface PopoverPosition {
  top: number;
  left: number;
  /** popover가 trigger 좌측 정렬(false) 또는 우측 정렬(true)인지 — viewport edge 충돌 회피용. */
  alignRight: boolean;
}

export interface HelpButtonProps {
  /** Guide section id — popover의 "전체 가이드 보기"로 진입할 섹션. */
  sectionId: string;
  /** 짧은 설명 텍스트 — i18n key 또는 plain string. props로 받음. */
  hint?: string;
  /** 트리거 버튼 aria-label override. 기본은 i18n. */
  ariaLabel?: string;
  /** 테스트용 prefix — 동일 페이지에 여러 도움말 버튼이 있을 때 분리. */
  testId?: string;
}

const NAV_EVENT = "lmmaster:navigate";
const GUIDE_OPEN_EVENT = "lmmaster:guide:open";

export function HelpButton({
  sectionId,
  hint,
  ariaLabel,
  testId,
}: HelpButtonProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState<PopoverPosition | null>(null);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const popoverRef = useRef<HTMLDivElement | null>(null);
  const closeBtnRef = useRef<HTMLButtonElement | null>(null);
  const linkBtnRef = useRef<HTMLButtonElement | null>(null);
  const popoverId = useId();
  const POPOVER_WIDTH = 280;
  const VIEWPORT_MARGIN = 12;

  // 트리거 위치 측정 → viewport-relative 좌표 + collision-aware align 결정.
  const computePosition = useCallback((): PopoverPosition | null => {
    const trigger = triggerRef.current;
    if (!trigger) return null;
    const rect = trigger.getBoundingClientRect();
    const viewportWidth = globalThis.window?.innerWidth ?? 0;
    // trigger 우측 끝 + popover 폭이 viewport 우측 margin을 침범하면 우측 정렬.
    const wouldOverflowRight = rect.left + POPOVER_WIDTH + VIEWPORT_MARGIN > viewportWidth;
    const top = rect.bottom + 8;
    const left = wouldOverflowRight
      ? Math.max(VIEWPORT_MARGIN, rect.right - POPOVER_WIDTH)
      : rect.left;
    return { top, left, alignRight: wouldOverflowRight };
  }, []);

  // open 시 위치 측정 + resize/scroll listener.
  useLayoutEffect(() => {
    if (!open) {
      setPosition(null);
      return undefined;
    }
    setPosition(computePosition());
    const onResize = () => setPosition(computePosition());
    globalThis.window?.addEventListener("resize", onResize);
    globalThis.window?.addEventListener("scroll", onResize, true);
    return () => {
      globalThis.window?.removeEventListener("resize", onResize);
      globalThis.window?.removeEventListener("scroll", onResize, true);
    };
  }, [open, computePosition]);

  // popover 열릴 때 close 버튼에 포커스.
  useEffect(() => {
    if (open) {
      // 마이크로 task 후 — DOM 마운트 완료 대기.
      const id = globalThis.requestAnimationFrame?.(() => {
        closeBtnRef.current?.focus();
      });
      return () => {
        if (id !== undefined) globalThis.cancelAnimationFrame?.(id);
      };
    }
    // 닫혔을 때 트리거로 포커스 복원.
    triggerRef.current?.focus({ preventScroll: true });
    return undefined;
  }, [open]);

  // Esc + 외부 클릭 닫기.
  useEffect(() => {
    if (!open) return undefined;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        setOpen(false);
      }
    };
    const onClick = (e: MouseEvent) => {
      const root = popoverRef.current;
      const trigger = triggerRef.current;
      if (!root) return;
      const target = e.target as Node | null;
      if (target && !root.contains(target) && !trigger?.contains(target)) {
        setOpen(false);
      }
    };
    globalThis.window?.addEventListener("keydown", onKey);
    globalThis.window?.addEventListener("mousedown", onClick);
    return () => {
      globalThis.window?.removeEventListener("keydown", onKey);
      globalThis.window?.removeEventListener("mousedown", onClick);
    };
  }, [open]);

  const handleToggle = useCallback(() => {
    setOpen((v) => !v);
  }, []);

  const handleOpenGuide = useCallback(() => {
    // 1) Guide page로 nav.
    try {
      globalThis.window?.dispatchEvent(
        new CustomEvent(NAV_EVENT, { detail: "guide" }),
      );
    } catch {
      /* noop */
    }
    // 2) 어떤 섹션으로 이동할지 알림 (Guide가 listen).
    try {
      globalThis.window?.dispatchEvent(
        new CustomEvent(GUIDE_OPEN_EVENT, { detail: { section: sectionId } }),
      );
    } catch {
      /* noop */
    }
    setOpen(false);
  }, [sectionId]);

  // Tab focus trap 안.
  const handleKeyDown = useCallback(
    (e: ReactKeyboardEvent) => {
      if (e.key !== "Tab") return;
      const root = popoverRef.current;
      if (!root) return;
      const focusable = root.querySelectorAll<HTMLElement>(
        'button, [href], input, [tabindex]:not([tabindex="-1"])',
      );
      if (focusable.length === 0) return;
      const first = focusable.item(0);
      const last = focusable.item(focusable.length - 1);
      if (!first || !last) return;
      const active = globalThis.document.activeElement as HTMLElement | null;
      if (e.shiftKey && active === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && active === last) {
        e.preventDefault();
        first.focus();
      }
    },
    [],
  );

  const tid = testId ?? `help-${sectionId}`;
  const trigLabel = ariaLabel ?? t("screens.help.triggerAria");

  return (
    <span className="help-button-wrap">
      <button
        type="button"
        ref={triggerRef}
        className="help-button-trigger"
        aria-label={trigLabel}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={open ? popoverId : undefined}
        onClick={handleToggle}
        data-testid={tid}
      >
        <HelpCircle
          size={16}
          strokeWidth={2}
          aria-hidden="true"
          className="help-button-icon"
        />
      </button>

      {open &&
        position &&
        globalThis.document?.body &&
        createPortal(
          <div
            ref={popoverRef}
            id={popoverId}
            role="dialog"
            aria-modal="true"
            aria-label={t("screens.help.popoverAria") ?? undefined}
            className="help-button-popover"
            data-testid={`${tid}-popover`}
            onKeyDown={handleKeyDown}
            style={{
              top: `${position.top}px`,
              left: `${position.left}px`,
              width: `${POPOVER_WIDTH}px`,
            }}
          >
            <p className="help-button-hint" data-testid={`${tid}-hint`}>
              {hint ?? t("screens.help.defaultHint")}
            </p>
            <div className="help-button-actions">
              <button
                type="button"
                ref={linkBtnRef}
                className="help-button-link"
                onClick={handleOpenGuide}
                data-testid={`${tid}-open-guide`}
              >
                {t("screens.help.openGuide")}
              </button>
              <button
                type="button"
                ref={closeBtnRef}
                className="help-button-close"
                onClick={() => setOpen(false)}
                data-testid={`${tid}-close`}
              >
                {t("screens.help.close")}
              </button>
            </div>
          </div>,
          globalThis.document.body,
        )}
    </span>
  );
}

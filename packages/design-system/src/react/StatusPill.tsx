// StatusPill — Tailscale 패턴.
//
// 정책 (Phase 4 결정 노트 §1.1):
// - 4 상태 (booting / listening / failed / stopping) + idle.
// - dot + label + 보조 num (port / latency / size).
// - data-status로 토큰 swap (CSS는 StatusPill.css).
// - a11y: role="status" + aria-live="polite".

import type { CSSProperties } from "react";

export type PillStatus = "booting" | "listening" | "failed" | "stopping" | "idle";

export interface StatusPillProps {
  status: PillStatus;
  label: string;
  detail?: string | null;
  size?: "sm" | "md" | "lg";
  ariaLabel?: string;
  className?: string;
  style?: CSSProperties;
}

export function StatusPill({
  status,
  label,
  detail,
  size = "md",
  ariaLabel,
  className,
  style,
}: StatusPillProps) {
  return (
    <div
      className={`ds-pill ds-pill-${size}${className ? " " + className : ""}`}
      data-status={status}
      role="status"
      aria-live="polite"
      aria-label={ariaLabel}
      style={style}
    >
      <span className="ds-pill-dot" aria-hidden />
      <span className="ds-pill-label">{label}</span>
      {detail != null && detail.length > 0 && (
        <span className="ds-pill-detail num">{detail}</span>
      )}
    </div>
  );
}

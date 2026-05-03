// LMmaster 공식 픽토그램 v3 — Phase 14' v3 하이엔드 리뉴얼 (2026-05-04).
//
// 디자인 컨셉: Triangulated Network Mark.
// - 외곽: 둥근 사각 (rx 7) + 4-stop diagonal gradient (cyan→neon green→deep green).
// - 내부: center dot (결집) + 4 corner dot + 4 connection line. AI 네트워크가 결집하는 메타.
// - 모든 내부 element는 어두운 fill (#04140a == --primary-on) — gradient 위 음각 효과.
// - drop-shadow 다층 글로우 (brand.css).
// - 3-tone gradient + cyan accent stop으로 "Vercel + Anthropic" 톤.
//
// 정책 (CLAUDE.md §4.3 / §4.6):
// - 외부 브랜드 마크 예외 정책 따라 컬러 SVG OK.
// - linear-gradient `<defs>` 안 — currentColor와 분리.
// - useId()로 instance별 gradient ID 고유화.

import { useId } from "react";

import "./brand.css";

export interface BrandMarkProps {
  size?: number;
  className?: string;
  ariaLabel?: string;
}

export function BrandMark({ size = 28, className, ariaLabel }: BrandMarkProps) {
  const reactId = useId().replace(/:/g, "");
  const gradId = `lmm-brand-bg-${reactId}`;
  const labelProps = ariaLabel
    ? { role: "img", "aria-label": ariaLabel }
    : { "aria-hidden": true };
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      viewBox="0 0 32 32"
      className={`brand-mark${className ? ` ${className}` : ""}`}
      {...labelProps}
    >
      <defs>
        {/* 3-tone gradient + cyan accent — diagonal flow. */}
        <linearGradient id={gradId} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="#7cfff5" />
          <stop offset="40%" stopColor="#4cffa0" />
          <stop offset="80%" stopColor="#38ff7e" />
          <stop offset="100%" stopColor="#1ee063" />
        </linearGradient>
      </defs>

      {/* 둥근 사각 컨테이너 — gradient fill (브랜드 본체). */}
      <rect x="0" y="0" width="32" height="32" rx="7.5" fill={`url(#${gradId})`} />

      {/* 4 connection line — 외곽 corner → 중앙. 네트워크 메타. */}
      <line x1="9.8" y1="9.8" x2="14.2" y2="14.2" stroke="#04140a" strokeWidth="1.2" strokeLinecap="round" />
      <line x1="22.2" y1="9.8" x2="17.8" y2="14.2" stroke="#04140a" strokeWidth="1.2" strokeLinecap="round" />
      <line x1="22.2" y1="22.2" x2="17.8" y2="17.8" stroke="#04140a" strokeWidth="1.2" strokeLinecap="round" />
      <line x1="9.8" y1="22.2" x2="14.2" y2="17.8" stroke="#04140a" strokeWidth="1.2" strokeLinecap="round" />

      {/* 4 corner dot — 외곽 노드 (작음). */}
      <circle cx="9" cy="9" r="1.5" fill="#04140a" />
      <circle cx="23" cy="9" r="1.5" fill="#04140a" />
      <circle cx="23" cy="23" r="1.5" fill="#04140a" />
      <circle cx="9" cy="23" r="1.5" fill="#04140a" />

      {/* center dot — 결집 (가장 큼, 시각 hierarchy). */}
      <circle cx="16" cy="16" r="2.8" fill="#04140a" />
    </svg>
  );
}

/** 워드마크 + 마크 lockup — 사이드바 상단 등에서 사용. */
export function BrandLockup({
  size = 28,
  wordmark = "LMmaster",
  className,
}: {
  size?: number;
  wordmark?: string;
  className?: string;
}) {
  return (
    <div className={`brand-lockup${className ? ` ${className}` : ""}`}>
      <BrandMark size={size} ariaLabel={`${wordmark} logo`} />
      <span className="brand-wordmark">{wordmark}</span>
    </div>
  );
}

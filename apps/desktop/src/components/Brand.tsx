// LMmaster 공식 픽토그램 v4 — Phase 14' v6 (2026-05-04, 권장안 채택).
//
// 디자인 컨셉: chunky filled M letterform + 깊이감 gradient + 위쪽 작은 status dot.
// - Anthropic Claude / Notion / Linear 스타일 (단순 + 직관 + 작은 사이즈 가독성).
// - Triangulated Network Mark (v3)가 작은 사이즈에서 X로 보이던 문제 해결.
// - 톤다운된 sage green gradient (#5eddae 권장 컬러 채택, "전문 AI 테크놀로지" 느낌).
// - 상단 작은 dot — Local AI status / signal 메타.

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
        {/* 3-tone gradient — cyan accent → sage green → deep green. 톤다운된 채도. */}
        <linearGradient id={gradId} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="#7cd4cc" />
          <stop offset="50%" stopColor="#5eddae" />
          <stop offset="100%" stopColor="#3fb887" />
        </linearGradient>
      </defs>

      {/* 둥근 사각 컨테이너 — gradient fill. */}
      <rect x="0" y="0" width="32" height="32" rx="7.5" fill={`url(#${gradId})`} />

      {/*
       * Chunky M letterform — outline path.
       * 좌측 stroke + 좌측 dip → 가운데 peak ↑ → 우측 dip → 우측 stroke.
       * 두께 4px, 가운데 V dip 깊이 약 50%.
       */}
      <path
        d="M7 22 V10 L11 14.5 L16 9 L21 14.5 L25 10 V22 H21.5 V14.8 L16.5 21 H15.5 L11 14.8 V22 Z"
        fill="#0a1a14"
      />

      {/* 상단 status dot — Local AI signal 메타. 작고 미세. */}
      <circle cx="16" cy="6" r="0.9" fill="#0a1a14" opacity="0.65" />
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

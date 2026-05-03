// LMmaster 공식 픽토그램 — Phase 14' (2026-05-03 v2 리뉴얼).
//
// 디자인:
// - chunky filled silhouette + linear gradient — Kling AI / 모던 AI 프로덕트 스타일.
// - 둥근 사각(rx 7.5) 컨테이너 fill: 네온 그린 그라디언트 (#4cffa0 → #1ee063).
// - 내부 chunky M letterform fill: 어두운 색 (#04140a == --primary-on).
// - drop-shadow glow — 사이드바 dark 배경에서 임팩트.
// - 24x24 viewBox 표준 (size prop으로 scale).
//
// 정책 (CLAUDE.md §4.3 픽토그램 정책 / §4.6 Phase 14'):
// - 외부 컨테이너 fill = brand recognition.
// - linear gradient는 SVG `<defs>` 내 — currentColor와 분리.
// - drop-shadow는 brand.css에서 토큰 + radial 글로우.

import { useId } from "react";

import "./brand.css";

export interface BrandMarkProps {
  size?: number;
  className?: string;
  /** 워드마크와 같이 표시되지 않는 단독 마크 — aria-label 노출. */
  ariaLabel?: string;
}

export function BrandMark({ size = 28, className, ariaLabel }: BrandMarkProps) {
  // 동일 페이지에 multiple instance 시 gradient ID 충돌 방지 — useId().
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
        <linearGradient id={gradId} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="#4cffa0" />
          <stop offset="100%" stopColor="#1ee063" />
        </linearGradient>
      </defs>
      {/* 둥근 사각 컨테이너 — gradient fill (LMmaster 데스크톱 컨테이너 메타). */}
      <rect x="0" y="0" width="32" height="32" rx="7.5" fill={`url(#${gradId})`} />
      {/* chunky M letterform — Master/Model. 두꺼운 fill으로 가독성 + 임팩트. */}
      <path
        d="M7 24 V8 H10.5 L16 16.5 L21.5 8 H25 V24 H21.5 V14 L17.5 20 H14.5 L10.5 14 V24 Z"
        fill="#04140a"
      />
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

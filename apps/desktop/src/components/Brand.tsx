// LMmaster 공식 픽토그램 — Phase 14' 브랜드 정책 (2026-05-03).
//
// 디자인 메타:
// - 둥근 사각 컨테이너(데스크톱 앱) + 내부 M zigzag(Model/Master).
// - "wrap-not-replace" thesis 시각화 — 외부 컨테이너가 내부 모델을 wrap.
// - 단도 stroke + currentColor — Dark 테마 + 네온 그린 accent 자동 적응.
// - 24x24 viewBox + 1.5/1.75 stroke hierarchy (외부 강조 → 내부 디테일).
//
// 정책 (CLAUDE.md §4.6):
// - 컬러 이모지 직접 사용 금지. lucide-react 픽토그램 또는 자체 SVG만.
// - currentColor + monochrome stroke + 24px grid 표준.
// - 새 픽토그램 추가 시 본 파일 또는 별개 SVG 컴포넌트로 작성.

import "./brand.css";

export interface BrandMarkProps {
  size?: number;
  className?: string;
  /** 워드마크와 같이 표시되지 않는 단독 마크 — aria-label 노출. */
  ariaLabel?: string;
}

export function BrandMark({ size = 28, className, ariaLabel }: BrandMarkProps) {
  const labelProps = ariaLabel
    ? { role: "img", "aria-label": ariaLabel }
    : { "aria-hidden": true };
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.75"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={`brand-mark${className ? ` ${className}` : ""}`}
      {...labelProps}
    >
      {/* 외부 컨테이너 — 데스크톱 앱 (LMmaster). */}
      <rect x="2.5" y="2.5" width="19" height="19" rx="4" />
      {/* 내부 M zigzag — Model/Master. */}
      <path d="M7 17V8l5 6 5-6v9" strokeWidth="1.5" />
    </svg>
  );
}

/** 워드마크 + 마크 동시 표시 — 사이드바 상단 등에서 사용. */
export function BrandLockup({
  size = 24,
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

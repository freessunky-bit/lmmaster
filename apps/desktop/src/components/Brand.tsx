// LMmaster 공식 픽토그램 v6 — Phase 14'.x (2026-05-05, 사용자 line-art diamond 요청).
//
// 디자인: 네온 그린 line-art brilliant-cut 다이아몬드 (geometric, fill 없음).
// - v5.x fluid wing(녹색 구름/curl) 완전 폐기 → v6 geometric line-art.
// - CLAUDE.md design_system_contract "네온 그린 single accent" 일관 (multi-stop gradient 제거).
// - 6 vertex hexagonal silhouette (T·C1·C2·G1·G2·B) + crown table + girdle + 4 long facet diagonals.
//   = 입체 brilliant-cut 다이아몬드 (table + crown + girdle + pavilion 4단 구조 visible).
// - stroke="currentColor" → brand.css의 color: var(--primary)로 토큰 제어.
// - drop-shadow 네온 글로우는 brand.css 그대로 (Phase 14' v6 ambient depth 정신 일관).
// - strokeWidth 1.5 + linecap/linejoin round — 모든 사이즈 일관 가독성.
//
// 좌표 계산 (viewBox 32x32):
//   T (top apex):         (16, 3)
//   C1 (crown left):      (8, 11)
//   C2 (crown right):     (24, 11)
//   G1 (girdle left):     (3, 16)   ← 가장 넓은 가로
//   G2 (girdle right):    (29, 16)
//   B (bottom culet):     (16, 29)

import "./brand.css";

export interface BrandMarkProps {
  size?: number;
  className?: string;
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
      viewBox="0 0 32 32"
      className={`brand-mark${className ? ` ${className}` : ""}`}
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      {...labelProps}
    >
      {/* Outer hexagonal silhouette — T → C2 → G2 → B → G1 → C1 → close */}
      <path d="M16 3 L24 11 L29 16 L16 29 L3 16 L8 11 Z" />
      {/* Crown table — 위쪽 평면 가로 */}
      <path d="M8 11 L24 11" />
      {/* Girdle — 다이아몬드 가장 넓은 가로 */}
      <path d="M3 16 L29 16" />
      {/* Crown long facets — top apex → girdle 양 끝 (입체 상단) */}
      <path d="M16 3 L3 16" />
      <path d="M16 3 L29 16" />
      {/* Pavilion long facets — bottom culet → crown corners (입체 하단) */}
      <path d="M16 29 L8 11" />
      <path d="M16 29 L24 11" />
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

// HuggingFace 공식 brand 마크 (Phase 14' — 픽토그램 정책 예외 §4.6).
//
// 정책:
// - 일반적으로 LMmaster 내 모든 이모지/아이콘은 monochrome stroke 픽토그램으로 통일.
// - 다만 *외부 서비스 brand 마크*(HuggingFace, GitHub 등)는 brand recognition을 위해
//   공식 컬러 SVG 사용 OK. 사용자 명시 결정 (2026-05-03).
// - 단, 자체 디자인 요소(NEW 탭 / 성인 토글 / 도움말 등)는 컬러 이모지 사용 금지.
//
// 마크 출처: huggingface.co 브랜드 asset 단순화 — face + 양손 hugging.

export interface HuggingFaceMarkProps {
  size?: number;
  className?: string;
  ariaLabel?: string;
}

export function HuggingFaceMark({
  size = 18,
  className,
  ariaLabel,
}: HuggingFaceMarkProps) {
  const labelProps = ariaLabel
    ? { role: "img", "aria-label": ariaLabel }
    : { "aria-hidden": true };
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      viewBox="0 0 64 64"
      className={className}
      {...labelProps}
    >
      {/* 머리 — HF brand yellow. */}
      <circle cx="32" cy="32" r="22" fill="#FFD21E" />
      {/* 양손 — HF brand orange. */}
      <path
        d="M14 38 Q 8 44 12 52 L 18 47 Q 18 42 14 38 Z"
        fill="#FF9D0B"
      />
      <path
        d="M50 38 Q 56 44 52 52 L 46 47 Q 46 42 50 38 Z"
        fill="#FF9D0B"
      />
      {/* 눈 — 단순한 까만 점. */}
      <circle cx="24.5" cy="28" r="2.4" fill="#1F1F1F" />
      <circle cx="39.5" cy="28" r="2.4" fill="#1F1F1F" />
      {/* 미소 — 친근한 곡선. */}
      <path
        d="M22.5 36 Q 32 44 41.5 36"
        stroke="#1F1F1F"
        strokeWidth="2.2"
        fill="none"
        strokeLinecap="round"
      />
    </svg>
  );
}

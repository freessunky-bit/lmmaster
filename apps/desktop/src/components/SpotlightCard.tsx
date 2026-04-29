// SpotlightCard — pointermove를 따라가는 radial-gradient hover wash.
// Phase 1A.4.e Polish & Palette §A3.
//
// 구현 정책:
// - rAF 게이트로 pointermove를 자동 ~16ms throttle.
// - CSS variable (--mx, --my) → components.css `.spotlight::before`가 radial-gradient at <mouse>로 사용.
// - reduced-motion에선 자동으로 flat tone (components.css가 처리).
// - Wrapper div + role="presentation" — 시맨틱 영향 0. children이 실제 의미.

import type { HTMLAttributes, ReactNode } from "react";
import { useRef } from "react";

type SpotlightCardProps = HTMLAttributes<HTMLDivElement> & {
  children: ReactNode;
  /** 추가 className — 예: "onb-runtime-card". */
  className?: string;
};

export function SpotlightCard({
  children,
  className = "",
  ...rest
}: SpotlightCardProps) {
  const ref = useRef<HTMLDivElement>(null);
  const rafRef = useRef(0);

  const onPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (rafRef.current) return;
    rafRef.current = requestAnimationFrame(() => {
      rafRef.current = 0;
      const el = ref.current;
      if (!el) return;
      const r = el.getBoundingClientRect();
      el.style.setProperty("--mx", `${e.clientX - r.left}px`);
      el.style.setProperty("--my", `${e.clientY - r.top}px`);
    });
  };

  return (
    <div
      ref={ref}
      className={`spotlight ${className}`.trim()}
      onPointerMove={onPointerMove}
      {...rest}
    >
      {children}
    </div>
  );
}

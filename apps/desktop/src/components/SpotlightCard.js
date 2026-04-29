import { jsx as _jsx } from "react/jsx-runtime";
import { useRef } from "react";
export function SpotlightCard({ children, className = "", ...rest }) {
    const ref = useRef(null);
    const rafRef = useRef(0);
    const onPointerMove = (e) => {
        if (rafRef.current)
            return;
        rafRef.current = requestAnimationFrame(() => {
            rafRef.current = 0;
            const el = ref.current;
            if (!el)
                return;
            const r = el.getBoundingClientRect();
            el.style.setProperty("--mx", `${e.clientX - r.left}px`);
            el.style.setProperty("--my", `${e.clientY - r.top}px`);
        });
    };
    return (_jsx("div", { ref: ref, className: `spotlight ${className}`.trim(), onPointerMove: onPointerMove, ...rest, children: children }));
}

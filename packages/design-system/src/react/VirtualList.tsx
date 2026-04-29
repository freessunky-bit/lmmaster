// VirtualList — @tanstack/react-virtual 기반.
//
// 정책 (Phase 4 결정 노트 §1.1):
// - 24px row default + sticky group header 옵션 + smooth scroll + a11y role="row".
// - empty state slot.
// - 50+ row (모델 / preset / 키 / 진단 로그) 공용.

import { useVirtualizer } from "@tanstack/react-virtual";
import type { CSSProperties, ReactNode } from "react";
import { useRef } from "react";

export interface VirtualListProps<T> {
  items: T[];
  rowHeight?: number;
  overscan?: number;
  renderRow: (item: T, index: number) => ReactNode;
  keyOf: (item: T) => string;
  groupBy?: (item: T) => string;
  groupHeader?: (group: string) => ReactNode;
  emptyState?: ReactNode;
  height?: number | string;
  className?: string;
  style?: CSSProperties;
  ariaLabel?: string;
}

export function VirtualList<T>({
  items,
  rowHeight = 24,
  overscan = 8,
  renderRow,
  keyOf,
  groupBy,
  groupHeader,
  emptyState,
  height = "100%",
  className,
  style,
  ariaLabel,
}: VirtualListProps<T>) {
  const parentRef = useRef<HTMLDivElement>(null);

  // group이 활성화되면 row + header를 동일 가상 stream으로 다룸.
  type Entry =
    | { kind: "header"; group: string; key: string }
    | { kind: "row"; item: T; index: number; key: string };
  const entries: Entry[] = [];
  if (groupBy) {
    let prev: string | null = null;
    items.forEach((item, idx) => {
      const g = groupBy(item);
      if (g !== prev) {
        entries.push({ kind: "header", group: g, key: `__hdr_${g}` });
        prev = g;
      }
      entries.push({ kind: "row", item, index: idx, key: keyOf(item) });
    });
  } else {
    items.forEach((item, idx) => {
      entries.push({ kind: "row", item, index: idx, key: keyOf(item) });
    });
  }

  const virtualizer = useVirtualizer({
    count: entries.length,
    getScrollElement: () => parentRef.current,
    estimateSize: (i) => (entries[i]?.kind === "header" ? Math.round(rowHeight * 1.4) : rowHeight),
    overscan,
  });

  if (items.length === 0 && emptyState) {
    return (
      <div
        ref={parentRef}
        className={`ds-vlist ds-vlist-empty${className ? " " + className : ""}`}
        style={{ height, ...style }}
        role="list"
        aria-label={ariaLabel}
      >
        {emptyState}
      </div>
    );
  }

  return (
    <div
      ref={parentRef}
      className={`ds-vlist${className ? " " + className : ""}`}
      style={{ height, overflowY: "auto", ...style }}
      role="list"
      aria-label={ariaLabel}
    >
      <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
        {virtualizer.getVirtualItems().map((v) => {
          const e = entries[v.index]!;
          const inner =
            e.kind === "header"
              ? (groupHeader?.(e.group) ?? <div className="ds-vlist-header">{e.group}</div>)
              : renderRow(e.item, e.index);
          return (
            <div
              key={e.key}
              role={e.kind === "header" ? "presentation" : "listitem"}
              className={
                e.kind === "header" ? "ds-vlist-row ds-vlist-row-header" : "ds-vlist-row"
              }
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                transform: `translateY(${v.start}px)`,
                height: v.size,
              }}
            >
              {inner}
            </div>
          );
        })}
      </div>
    </div>
  );
}

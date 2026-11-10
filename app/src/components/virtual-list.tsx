import { useRef, type CSSProperties, type ReactNode } from "react"
import { useVirtualizer } from "@tanstack/react-virtual"
import { cn } from "@/lib/utils"

/**
 * Fixed-row-height virtual list: every item is reachable by scrolling, but only the visible
 * rows (plus a small overscan) are rendered.
 */
export function VirtualList<T>({
  items,
  rowHeight,
  render,
  getKey,
  className,
  style,
  overscan = 12,
}: {
  items: T[]
  rowHeight: number
  render: (item: T, index: number) => ReactNode
  getKey?: (item: T, index: number) => string | number
  className?: string
  style?: CSSProperties
  overscan?: number
}) {
  const parent = useRef<HTMLDivElement>(null)
  const v = useVirtualizer({
    count: items.length,
    getScrollElement: () => parent.current,
    estimateSize: () => rowHeight,
    overscan,
    getItemKey: getKey ? (i) => getKey(items[i], i) : undefined,
  })
  return (
    <div ref={parent} className={cn("overflow-auto", className)} style={style}>
      <div style={{ height: v.getTotalSize(), position: "relative", minWidth: "100%" }}>
        {v.getVirtualItems().map((row) => (
          <div
            key={row.key}
            style={{ position: "absolute", top: 0, left: 0, right: 0, height: rowHeight, transform: `translateY(${row.start}px)` }}
          >
            {render(items[row.index], row.index)}
          </div>
        ))}
      </div>
    </div>
  )
}

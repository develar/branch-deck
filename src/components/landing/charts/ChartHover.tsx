import type { ReactNode } from "react"
import type { ChartHover } from "@/hooks/useChartHover"

/** The tooltip of the hovered mark above the plot, in text tokens; never under the pointer's click */
export function ChartTooltip({ hover, children }: { hover: ChartHover | null; children: ReactNode }) {
  if (hover === null) {
    return null
  }
  return (
    <div
      role="status"
      className="pointer-events-none absolute -top-1 z-10 -translate-x-1/2 -translate-y-full rounded-md bg-popover px-2 py-1 text-xs whitespace-nowrap text-popover-foreground shadow-md ring-1 ring-foreground/10"
      style={{ left: `${Math.min(90, Math.max(10, hover.x * 100))}%` }}
      data-testid="chart-tooltip"
    >
      {children}
    </div>
  )
}

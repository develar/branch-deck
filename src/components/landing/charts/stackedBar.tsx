import type { ReactElement } from "react"
import { Bar } from "recharts"
import type { BarShapeProps } from "recharts"

/** The props of a bar of the Landing charts: at most 24px, a 2px card gap to its neighbours, no animation */
export const BAR_PROPS = { stroke: "var(--card)", strokeWidth: 2, maxBarSize: 24, isAnimationActive: false } as const

/**
 * The bars of a stacked chart, one per part in its order, each in the colour of its part (`--color-<part>`) and drawn by
 * its shape. A list of elements, not a component: recharts finds its bars among the direct children of the chart
 */
export function stackedBars<Part extends string>(parts: readonly Part[], stackId: string, shapes: Record<Part, (props: BarShapeProps) => ReactElement>): ReactElement[] {
  return parts.map(part => <Bar key={part} dataKey={part} stackId={stackId} fill={`var(--color-${part})`} shape={shapes[part]} {...BAR_PROPS} />)
}

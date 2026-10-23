import { Line, LineChart, YAxis } from "recharts"
import type { ChartConfig } from "@/components/ui/chart"
import { ChartContainer } from "@/components/ui/chart"

const config = { value: { label: "Value", color: "var(--muted-foreground)" } } satisfies ChartConfig

/**
 * The trend of a figure over the weeks, under its value: a 2px line in the de-emphasis colour, the last week as a dot
 * in the accent; no axis, no tooltip (the figure says the value). The axis runs from the smallest value to the largest,
 * not from zero, so the 24px show the change and not the level. A week without a value breaks the line
 */
/** `id` names the chart in its style, stable for the snapshots (not `useId`) */
export function Sparkline({ id, values, label }: { id: string; values: readonly (number | null)[]; label: string }) {
  const known = values.filter(value => value !== null)
  if (known.length < 2) {
    return null
  }
  const data = values.map((value, index) => ({ index, value }))
  const last = data.length - 1
  const min = Math.min(...known)
  const max = Math.max(...known)
  const domain: [number, number] = min === max ? [min - 1, max + 1] : [min, max]
  return (
    <ChartContainer id={id} config={config} className="aspect-auto h-6 w-full max-w-32 self-end" role="img" aria-label={label}>
      <LineChart data={data} margin={{ top: 5, right: 5, bottom: 5, left: 5 }}>
        <YAxis hide domain={domain} />
        <Line
          dataKey="value"
          type="monotone"
          stroke="var(--color-value)"
          strokeWidth={2}
          isAnimationActive={false}
          dot={props => {
            const { cx, cy, index } = props as { cx?: number; cy?: number; index?: number }
            return index === last && cx !== undefined && cy !== undefined ? (
              <circle key="last" cx={cx} cy={cy} r={3} fill="var(--chart-1)" stroke="var(--card)" strokeWidth={2} />
            ) : (
              <g key={index} />
            )
          }}
        />
      </LineChart>
    </ChartContainer>
  )
}

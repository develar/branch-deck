import { ChartTooltip } from "@/components/landing/charts/ChartHover"
import { useChartHover } from "@/hooks/useChartHover"
import type { WeekCount } from "@/lib/bindings"
import { formatWeek } from "@/lib/robot-stats"

/**
 * The merges of each week, oldest first: one bar per week from the baseline, its rounded end up; the first and the
 * last week named under the bars, every week in its tooltip
 */
export function WeekBars({ weeks, label }: { weeks: readonly WeekCount[]; label: string }) {
  const { hover, markProps } = useChartHover()
  const max = Math.max(1, ...weeks.map(week => week.merged))
  const hovered = hover === null ? undefined : weeks[hover.index]
  const [first, last] = [weeks.at(0), weeks.at(-1)]
  const summary = `${label}: ${weeks.map(week => `${formatWeek(week.start)} ${week.merged}`).join(", ")}`
  return (
    <figure className="flex flex-col gap-1">
      <div className="relative">
        <div role="img" aria-label={summary} className="flex h-20 items-end gap-0.5 border-b border-border" data-testid="week-bars">
          {weeks.map((week, index) => (
            <div
              key={week.start}
              tabIndex={0}
              aria-label={`Week of ${formatWeek(week.start)}: ${week.merged} merged of ${week.robots} robots`}
              className="flex h-full flex-1 items-end outline-none focus-visible:ring-2 focus-visible:ring-ring"
              {...markProps(index, (index + 0.5) / weeks.length)}
            >
              <div
                className={week.merged === 0 ? "h-px w-full bg-border" : "w-full rounded-t-sm bg-primary"}
                style={week.merged === 0 ? undefined : { height: `${(week.merged / max) * 100}%`, opacity: hover === null || hover.index === index ? 1 : 0.5 }}
              />
            </div>
          ))}
        </div>
        <ChartTooltip hover={hover}>{hovered !== undefined && `Week of ${formatWeek(hovered.start)} · ${hovered.merged} merged of ${hovered.robots} robots`}</ChartTooltip>
      </div>
      {first !== undefined && last !== undefined && (
        <figcaption className="flex justify-between text-xs text-muted-foreground">
          <span>{formatWeek(first.start)}</span>
          <span>{formatWeek(last.start)}</span>
        </figcaption>
      )}
    </figure>
  )
}

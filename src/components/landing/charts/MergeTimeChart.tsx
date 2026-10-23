import { CartesianGrid, LabelList, Line, LineChart, XAxis, YAxis } from "recharts"
import type { ChartConfig } from "@/components/ui/chart"
import { ChartContainer, ChartTooltip, ChartTooltipContent } from "@/components/ui/chart"
import { TableView } from "@/components/landing/charts/TableView"
import type { WeekCount } from "@/lib/bindings"
import { formatShortDuration } from "@/lib/robot-eta"
import { formatAxisMinutes, formatChartWeek, minuteTicks } from "@/lib/robot-stats"

/** The user in the ink of the text, everyone else in the colour of the merged robots, as in the hours */
const config = {
  mine: { label: "You", color: "var(--foreground)" },
  everyone: { label: "Everyone else", color: "var(--chart-1)" },
} satisfies ChartConfig

const SERIES = ["mine", "everyone"] as const
type Series = (typeof SERIES)[number]

/**
 * The median time to merge of each week, the user's against everyone else's merges of the same week, on one axis of
 * time: a week the user is slow and everyone else is not was the branch, both slow was the queue. 2px lines, a week
 * without a value breaks its line, each line named at its last point and in the legend under it; the crosshair tooltip
 * says both, the table twin every value. Nothing while fewer than two weeks have a value
 */
export function MergeTimeChart({ weeks }: { weeks: readonly WeekCount[] }) {
  const minutes = (seconds: number | null) => (seconds === null ? null : seconds / 60)
  const data = weeks.map(week => ({ week: formatChartWeek(week), mine: minutes(week.merge_time), everyone: minutes(week.everyone_merge_time) }))
  if (data.filter(week => week.mine !== null || week.everyone !== null).length < 2) {
    return null
  }
  const lastOf = (series: Series) => data.findLastIndex(week => week[series] !== null)
  const ticks = minuteTicks(Math.max(0, ...data.flatMap(week => [week.mine ?? 0, week.everyone ?? 0])))
  return (
    <figure className="flex flex-col gap-1" data-testid="merge-time-chart">
      <figcaption className="text-xs font-medium text-muted-foreground">Time to merge by week</figcaption>
      <ChartContainer id="merge-time" config={config} className="aspect-auto h-40 w-full">
        <LineChart data={data} accessibilityLayer margin={{ top: 8, right: 88, bottom: 0, left: 0 }}>
          <CartesianGrid vertical={false} />
          <XAxis dataKey="week" tickLine={false} axisLine={false} tickMargin={6} interval="preserveStartEnd" minTickGap={24} />
          <YAxis tickLine={false} axisLine={false} width={40} tickFormatter={formatAxisMinutes} ticks={ticks} domain={[0, ticks.at(-1) ?? 0]} />
          <ChartTooltip
            content={
              <ChartTooltipContent
                formatter={(value, name) => (
                  <span className="flex w-full items-center gap-2">
                    <span aria-hidden className="h-0.5 w-3 shrink-0" style={{ backgroundColor: config[name as Series].color }} />
                    <span className="flex-1 text-muted-foreground">{config[name as Series].label}</span>
                    <span className="font-medium text-foreground tabular-nums">{formatShortDuration(Number(value) * 60)}</span>
                  </span>
                )}
              />
            }
          />
          {SERIES.map(series => (
            <Line key={series} dataKey={series} type="monotone" stroke={`var(--color-${series})`} strokeWidth={2} dot={false} isAnimationActive={false}>
              <LabelList
                dataKey={series}
                content={props =>
                  props.index === lastOf(series) && typeof props.x === "number" && typeof props.y === "number" ? (
                    <text x={props.x + 6} y={props.y} dy={4} className="fill-foreground text-xs">
                      {config[series].label}
                    </text>
                  ) : null
                }
              />
            </Line>
          ))}
        </LineChart>
      </ChartContainer>
      {/* a line key for a line, in the order of the series */}
      <ul className="flex flex-wrap justify-center gap-x-4 gap-y-1 text-xs text-muted-foreground" data-testid="merge-time-legend">
        {SERIES.map(series => (
          <li key={series} className="flex items-center gap-1.5">
            <span aria-hidden className="h-0.5 w-3" style={{ backgroundColor: config[series].color }} />
            {config[series].label}
          </li>
        ))}
      </ul>
      <TableView
        caption="Median time to merge by week"
        columns={["Week", "You", "Everyone else"]}
        rows={weeks.map(week => ({
          key: String(week.start),
          cells: [
            formatChartWeek(week),
            week.merge_time === null ? "–" : formatShortDuration(week.merge_time),
            week.everyone_merge_time === null ? "–" : formatShortDuration(week.everyone_merge_time),
          ],
        }))}
      />
    </figure>
  )
}

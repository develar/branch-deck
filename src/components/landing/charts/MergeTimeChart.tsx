import { CartesianGrid, LabelList, Line, LineChart, XAxis, YAxis } from "recharts"
import type { ChartConfig } from "@/components/ui/chart"
import { ChartContainer, ChartTooltip, ChartTooltipContent } from "@/components/ui/chart"
import { ChartKey } from "@/components/landing/charts/ChartKey"
import { TableView } from "@/components/landing/charts/TableView"
import type { WeekCount } from "@/lib/bindings"
import { formatShortDuration } from "@/lib/robot-eta"
import { formatAxisMinutes, formatChartWeek, minuteTicks } from "@/lib/robot-stats"

/** The user in the ink of the text, the point of the chart; everyone else in the context blue, as in the hours */
const config = {
  mine: { label: "You", color: "var(--foreground)" },
  everyone: { label: "Everyone else", color: "var(--chart-context)" },
} satisfies ChartConfig

const SERIES = ["mine", "everyone"] as const
type Series = (typeof SERIES)[number]

/** The height of the plot inside the `h-40` chart (its margin and axis taken), and of a name at the end of a line */
const PLOT_HEIGHT = 114
const LABEL_HEIGHT = 14

/**
 * The median time to merge of each week, the user's against everyone else's merges of the same week, on one axis of
 * time: a week the user is slow and everyone else is not was the branch, both slow was the queue. The user's line in the
 * ink, everyone else's in the context blue (emphasis: one series is the point), 2px each, a week without a value breaks
 * its line, each line named at its last point while the ends stand apart, and in the key under it; the crosshair tooltip says both, the table twin
 * every value. Shown only while `hasMergeTimes`
 */
export function MergeTimeChart({ weeks }: { weeks: readonly WeekCount[] }) {
  const minutes = (seconds: number | null) => (seconds === null ? null : seconds / 60)
  const data = weeks.map(week => ({ week: formatChartWeek(week), mine: minutes(week.merge_time), everyone: minutes(week.everyone_merge_time) }))
  const lastOf = (series: Series) => data.findLastIndex(week => week[series] !== null)
  const ticks = minuteTicks(Math.max(0, ...data.flatMap(week => [week.mine ?? 0, week.everyone ?? 0])))
  const top = ticks.at(-1) ?? 0
  // the lines are named at their ends while the ends stand apart; two names on one spot are noise, the key says both
  const endOf = (series: Series) => data[lastOf(series)]?.[series] ?? null
  const mineEnd = endOf("mine")
  const everyoneEnd = endOf("everyone")
  const apart =
    mineEnd === null || everyoneEnd === null || lastOf("mine") !== lastOf("everyone") || (Math.abs(mineEnd - everyoneEnd) / Math.max(1, top)) * PLOT_HEIGHT >= LABEL_HEIGHT
  return (
    <figure className="flex flex-col gap-2" data-testid="merge-time-chart">
      <ChartContainer id="merge-time" config={config} className="aspect-auto h-40 w-full">
        <LineChart data={data} accessibilityLayer margin={{ top: 16, right: 88, bottom: 0, left: 0 }}>
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
                  apart && props.index === lastOf(series) && typeof props.x === "number" && typeof props.y === "number" ? (
                    <text x={props.x + 6} y={props.y} dy={4} className={series === "mine" ? "fill-foreground text-xs" : "fill-muted-foreground text-xs"}>
                      {config[series].label}
                    </text>
                  ) : null
                }
              />
            </Line>
          ))}
        </LineChart>
      </ChartContainer>
      <ChartKey testId="merge-time-legend" items={SERIES.map(series => ({ label: config[series].label, color: config[series].color, mark: "line" }))} />
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

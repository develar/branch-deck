import { Bar, Cell, CartesianGrid, ComposedChart, Rectangle, Scatter, XAxis, YAxis } from "recharts"
import type { BarShapeProps } from "recharts"
import type { ChartConfig } from "@/components/ui/chart"
import { ChartContainer, ChartTooltip, ChartTooltipContent } from "@/components/ui/chart"
import { ChartKey } from "@/components/landing/charts/ChartKey"
import type { ChartKeyItem } from "@/components/landing/charts/ChartKey"
import { TableView } from "@/components/landing/charts/TableView"
import type { FastestHours, StartHour } from "@/lib/bindings"
import { formatShortDuration } from "@/lib/robot-eta"
import { formatAxisMinutes, formatHour, minuteTicks } from "@/lib/robot-stats"

/**
 * Everyone else's bars in the context blue, the fastest hours of the sentence in the full blue of the merged robots they
 * are (`--chart-1`), the user's dots in the ink of the text: the shape and the key tell them apart, and no hue of
 * another entity (orange is Failed) stands for the user
 */
const config = {
  everyone: { label: "Everyone else, median", color: "var(--chart-context)" },
  mine: { label: "You", color: "var(--foreground)" },
} satisfies ChartConfig

const FASTEST_COLOR = "var(--chart-1)"

/** An hour label stands on the axis every third hour */
const TICK_HOURS = 3

function isSeries(key: string): key is keyof typeof config {
  return key in config
}

interface HourRow {
  readonly hour: string
  readonly everyone: number | null
  readonly labelled: boolean
}

/** A bar with its median on the cap when the sentence names its hour (the shape holds the row, a label list only an index among the drawn bars) */
function BarShape(props: BarShapeProps) {
  const row = props.payload as HourRow
  const { x, y, width } = props
  return (
    <g>
      <Rectangle {...props} radius={[4, 4, 0, 0]} />
      {row.labelled && row.everyone !== null && typeof x === "number" && typeof y === "number" && typeof width === "number" && (
        <text x={x + width / 2} y={y - 5} textAnchor="middle" className="fill-muted-foreground text-xs tabular-nums">
          {formatShortDuration(row.everyone * 60)}
        </text>
      )}
    </g>
  )
}

/**
 * The workday hours by the hour a merged robot started, from the first hour with a value to the last (the night is no
 * empty width): everyone else's median time to merge as a bar (shorter is faster), the run of the fastest hours named
 * in the sentence above in full colour, the other hours in the context blue (emphasis, not a second hue), the two
 * medians of the sentence on their caps, the user's median as a dot with a card ring. One axis of time; an hour with
 * too few robots inside the range stays empty
 */
export function HourChart({ hours, fastest }: { hours: readonly StartHour[]; fastest: FastestHours | null }) {
  const known = (hour: StartHour) => hour.everyone !== null || hour.mine !== null
  const first = hours.findIndex(known)
  const last = hours.findLastIndex(known)
  const shown = first === -1 ? hours : hours.slice(first, last + 1)
  const inRun = (hour: number) => fastest !== null && hour >= fastest.start && hour < fastest.end
  const data = shown.map(hour => ({
    hour: formatHour(hour.hour),
    ofDay: hour.hour,
    everyone: hour.everyone === null ? null : hour.everyone.median / 60,
    mine: hour.mine === null ? null : hour.mine.median / 60,
    fastest: inRun(hour.hour),
    // the two numbers of the sentence stand on their bars: the median of the run on its first hour, the slowest hour's
    labelled: fastest !== null && (hour.hour === fastest.start || hour.hour === fastest.slowest),
    counts: { everyone: hour.everyone?.count ?? 0, mine: hour.mine?.count ?? 0 },
  }))
  const ticks = minuteTicks(Math.max(0, ...data.flatMap(row => [row.everyone ?? 0, row.mine ?? 0])))
  const key: ChartKeyItem[] = [
    { label: config.everyone.label, color: config.everyone.color, mark: "rect" },
    ...(fastest === null ? [] : [{ label: `Fastest hours ${formatHour(fastest.start)}–${formatHour(fastest.end)}`, color: FASTEST_COLOR, mark: "rect" as const }]),
    { label: config.mine.label, color: config.mine.color, mark: "dot" },
  ]
  return (
    <figure className="flex flex-col gap-2" data-testid="hour-chart">
      <ChartContainer id="hours" config={config} className="aspect-auto h-44 w-full">
        <ComposedChart data={data} accessibilityLayer margin={{ top: 16, right: 16, bottom: 0, left: 0 }}>
          <CartesianGrid vertical={false} />
          <XAxis dataKey="hour" tickLine={false} axisLine={false} tickMargin={6} interval={0} ticks={data.filter(row => row.ofDay % TICK_HOURS === 0).map(row => row.hour)} />
          <YAxis tickLine={false} axisLine={false} width={40} tickFormatter={formatAxisMinutes} ticks={ticks} domain={[0, ticks.at(-1) ?? 0]} />
          <ChartTooltip
            cursor={false}
            content={
              <ChartTooltipContent
                formatter={(value, name, item) => {
                  const counts = (item.payload as { counts: Record<string, number> }).counts
                  const series = String(name)
                  // the dots of a scatter bring their x (the hour) along: the label of the tooltip says it already
                  if (!isSeries(series)) {
                    return null
                  }
                  return (
                    <span className="flex w-full items-center gap-2">
                      <span aria-hidden className={series === "mine" ? "size-2.5 shrink-0 rounded-full" : "size-2.5 shrink-0"} style={{ backgroundColor: config[series].color }} />
                      <span className="flex-1 text-muted-foreground">{config[series].label}</span>
                      <span className="font-medium text-foreground tabular-nums">{`${formatShortDuration(Number(value) * 60)} · ${counts[series] ?? 0} robots`}</span>
                    </span>
                  )
                }}
              />
            }
          />
          <Bar dataKey="everyone" name="everyone" fill="var(--color-everyone)" maxBarSize={24} shape={BarShape} isAnimationActive={false}>
            {data.map(row => (
              <Cell key={row.hour} fill={row.fastest ? FASTEST_COLOR : config.everyone.color} />
            ))}
          </Bar>
          <Scatter dataKey="mine" name="mine" fill="var(--color-mine)" stroke="var(--card)" strokeWidth={2} isAnimationActive={false} />
        </ComposedChart>
      </ChartContainer>
      <ChartKey testId="hour-legend" items={key} />
      <TableView
        caption="Median time to merge by start hour"
        columns={["Hour", "Everyone else", "Robots", "You", "Your robots"]}
        rows={hours.map(hour => ({
          key: String(hour.hour),
          cells: [
            formatHour(hour.hour),
            hour.everyone === null ? "–" : formatShortDuration(hour.everyone.median),
            hour.everyone?.count ?? 0,
            hour.mine === null ? "–" : formatShortDuration(hour.mine.median),
            hour.mine?.count ?? 0,
          ],
        }))}
      />
    </figure>
  )
}

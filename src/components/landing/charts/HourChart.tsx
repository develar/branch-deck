import { Bar, Cell, CartesianGrid, ComposedChart, Scatter, XAxis, YAxis } from "recharts"
import type { ChartConfig } from "@/components/ui/chart"
import { ChartContainer, ChartTooltip, ChartTooltipContent } from "@/components/ui/chart"
import { TableView } from "@/components/landing/charts/TableView"
import type { StartHour } from "@/lib/bindings"
import { formatShortDuration } from "@/lib/robot-eta"
import { formatHour } from "@/lib/robot-stats"

/** Everyone else's bars and the user's dots in the first two slots of the chart palette */
const config = {
  everyone: { label: "Everyone else, median", color: "var(--chart-1)" },
  mine: { label: "You", color: "var(--chart-2)" },
} satisfies ChartConfig

/** The hours that are not the fastest: the series hue muted, readable on both cards */
const MUTED_OPACITY = 0.5

function isSeries(key: string): key is keyof typeof config {
  return key in config
}

/** Minutes on the axis: "0", "45m", "1h", "1.5h" (one line, unlike "1h 30m") */
function formatAxisMinutes(minutes: number): string {
  if (minutes === 0) {
    return "0"
  }
  return minutes < 60 ? `${Math.round(minutes)}m` : `${Number((minutes / 60).toFixed(1))}h`
}

/**
 * The 24 workday hours by the hour a merged robot started: everyone else's median time to merge as a bar (shorter is
 * faster; the fastest fifth, `StartHour.level` 0, in full colour, the others muted: emphasis, not a second hue), the
 * user's median as a dot with a card ring. One axis of time; an hour with too few robots stays empty
 */
export function HourChart({ hours }: { hours: readonly StartHour[] }) {
  const data = hours.map(hour => ({
    hour: formatHour(hour.hour),
    everyone: hour.everyone === null ? null : hour.everyone.median / 60,
    mine: hour.mine === null ? null : hour.mine.median / 60,
    fastest: hour.level === 0,
    counts: { everyone: hour.everyone?.count ?? 0, mine: hour.mine?.count ?? 0 },
  }))
  return (
    <figure className="flex flex-col" data-testid="hour-chart">
      <ChartContainer id="hours" config={config} className="aspect-auto h-48 w-full">
        <ComposedChart data={data} accessibilityLayer margin={{ top: 8, right: 0, bottom: 0, left: 0 }}>
          <CartesianGrid vertical={false} />
          <XAxis dataKey="hour" tickLine={false} axisLine={false} tickMargin={6} interval={2} />
          <YAxis tickLine={false} axisLine={false} width={40} tickFormatter={formatAxisMinutes} />
          <ChartTooltip
            cursor={false}
            content={
              <ChartTooltipContent
                formatter={(value, name, item) => {
                  const counts = (item.payload as { counts: Record<string, number> }).counts
                  const key = String(name)
                  // the dots of a scatter bring their x (the hour) along: the label of the tooltip says it already
                  if (!isSeries(key)) {
                    return null
                  }
                  return (
                    <span className="flex w-full items-center gap-2">
                      <span aria-hidden className={key === "mine" ? "size-2.5 shrink-0 rounded-full" : "size-2.5 shrink-0"} style={{ backgroundColor: config[key].color }} />
                      <span className="flex-1 text-muted-foreground">{config[key].label}</span>
                      <span className="font-medium text-foreground tabular-nums">{`${formatShortDuration(Number(value) * 60)} · ${counts[key] ?? 0} robots`}</span>
                    </span>
                  )
                }}
              />
            }
          />
          <Bar dataKey="everyone" name="everyone" fill="var(--color-everyone)" radius={[4, 4, 0, 0]} maxBarSize={24} isAnimationActive={false}>
            {data.map(row => (
              <Cell key={row.hour} fillOpacity={row.fastest ? 1 : MUTED_OPACITY} />
            ))}
          </Bar>
          <Scatter dataKey="mine" name="mine" fill="var(--color-mine)" stroke="var(--card)" strokeWidth={2} isAnimationActive={false} />
        </ComposedChart>
      </ChartContainer>
      <figcaption>
        <ul className="flex flex-wrap justify-center gap-x-4 gap-y-1 pt-2 text-xs text-muted-foreground" data-testid="hour-legend">
          <li className="flex items-center gap-1.5">
            <span aria-hidden className="size-2" style={{ backgroundColor: config.everyone.color, opacity: MUTED_OPACITY }} />
            {config.everyone.label}
          </li>
          <li className="flex items-center gap-1.5">
            <span aria-hidden className="size-2" style={{ backgroundColor: config.everyone.color }} />
            The fastest hours
          </li>
          <li className="flex items-center gap-1.5">
            <span aria-hidden className="size-2 rounded-full" style={{ backgroundColor: config.mine.color }} />
            {config.mine.label}
          </li>
        </ul>
      </figcaption>
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

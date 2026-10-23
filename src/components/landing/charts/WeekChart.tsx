import { Bar, BarChart, CartesianGrid, Rectangle, XAxis, YAxis } from "recharts"
import type { BarShapeProps } from "recharts"
import type { ChartConfig } from "@/components/ui/chart"
import { ChartContainer, ChartTooltip, ChartTooltipContent } from "@/components/ui/chart"
import { ChartKey } from "@/components/landing/charts/ChartKey"
import { BAR_PROPS } from "@/components/landing/charts/stackedBar"
import { TableView } from "@/components/landing/charts/TableView"
import type { WeekCount } from "@/lib/bindings"
import { countTicks, formatChartWeek } from "@/lib/robot-stats"

/**
 * The parts of a week's robots, in the order of the chart palette (`--chart-1…3`), which passes the colour-blind checks;
 * canceled is the neutral, no outcome
 */
const config = {
  merged: { label: "Merged", color: "var(--chart-1)" },
  failed: { label: "Failed", color: "var(--chart-2)" },
  canceled: { label: "Canceled", color: "var(--chart-3)" },
} satisfies ChartConfig

const PARTS = ["merged", "failed", "canceled"] as const
type Part = (typeof PARTS)[number]

/** The week of now holds its robots so far: its bar is muted, so its shortness does not read as a drop */
const SO_FAR_OPACITY = 0.5

/** The share of failed robots stands on the cap of each week while the weeks are few enough for the labels not to touch */
const MAX_LABELLED_WEEKS = 8

interface WeekRow extends Record<Part, number> {
  readonly week: string
  readonly current: boolean
  readonly robots: number
  /** The share of failed robots stands on the cap */
  readonly labelled: boolean
}

/** The part on top of a week's bar: the last one with robots */
function topPart(week: Record<Part, number>): Part | null {
  return PARTS.findLast(part => week[part] > 0) ?? null
}

/**
 * A part of a bar, its end rounded when no part lies above it in its week, and then the failed share of the week on
 * the cap (the shape holds the row, a label list only an index among the drawn bars)
 */
function partShape(part: Part) {
  return function PartShape(props: BarShapeProps) {
    const week = props.payload as WeekRow
    const top = topPart(week) === part
    const { x, y, width } = props
    return (
      <g>
        <Rectangle {...props} radius={top ? [4, 4, 0, 0] : 0} fillOpacity={week.current ? SO_FAR_OPACITY : 1} />
        {top && week.labelled && typeof x === "number" && typeof y === "number" && typeof width === "number" && (
          <text x={x + width / 2} y={y - 5} textAnchor="middle" className="fill-muted-foreground text-xs tabular-nums">
            {`${Math.round((week.failed / week.robots) * 100)}% failed`}
          </text>
        )}
      </g>
    )
  }
}

const SHAPES = { merged: partShape("merged"), failed: partShape("failed"), canceled: partShape("canceled") }

/**
 * The finished robots of each week, oldest first, stacked from the baseline: merged, failed, canceled. Bars of at most
 * 24px with a 2px card gap between the parts, the rounded end on the top part, hairline grid, the axis in clean counts;
 * the share of failed robots on the cap of each week, the one number the reader wants of the stack; the week of now
 * named "This week" and muted, as its robots so far; the key names the parts, the tooltip the week, the table twin
 * every value
 */
export function WeekChart({ weeks }: { weeks: readonly WeekCount[] }) {
  const data: WeekRow[] = weeks.map(week => ({
    week: formatChartWeek(week),
    current: week.current,
    merged: week.merged,
    failed: week.failed,
    canceled: week.canceled,
    robots: week.robots,
    labelled: weeks.length <= MAX_LABELLED_WEEKS && week.robots > 0,
  }))
  const ticks = countTicks(Math.max(0, ...data.map(week => week.robots)))
  return (
    <figure className="flex flex-col gap-2" data-testid="week-chart">
      <ChartContainer id="weeks" config={config} className="aspect-auto h-40 w-full">
        <BarChart data={data} accessibilityLayer margin={{ top: 16, right: 24, bottom: 0, left: 0 }}>
          <CartesianGrid vertical={false} />
          <XAxis dataKey="week" tickLine={false} axisLine={false} tickMargin={6} interval="preserveStartEnd" minTickGap={24} />
          <YAxis tickLine={false} axisLine={false} width={40} ticks={ticks} domain={[0, ticks.at(-1) ?? 0]} allowDecimals={false} />
          <ChartTooltip cursor={false} content={<ChartTooltipContent labelFormatter={label => <>{weekLabel(label)}</>} />} />
          {PARTS.map(part => (
            <Bar key={part} dataKey={part} stackId="robots" fill={`var(--color-${part})`} shape={SHAPES[part]} {...BAR_PROPS} />
          ))}
        </BarChart>
      </ChartContainer>
      <ChartKey testId="week-legend" items={PARTS.map(part => ({ label: config[part].label, color: config[part].color, mark: "rect" }))} />
      <TableView
        caption="Robots per week"
        columns={["Week", "Merged", "Failed", "Canceled"]}
        rows={data.map(week => ({ key: week.week, cells: [weekLabel(week.week), week.merged, week.failed, week.canceled] }))}
      />
    </figure>
  )
}

/** "Week of Oct 19", "This week, so far" */
function weekLabel(label: unknown): string {
  return label === "This week" ? "This week, so far" : `Week of ${String(label)}`
}

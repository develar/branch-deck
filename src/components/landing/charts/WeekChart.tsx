import { Bar, BarChart, CartesianGrid, Rectangle, XAxis, YAxis } from "recharts"
import type { BarShapeProps } from "recharts"
import type { ChartConfig } from "@/components/ui/chart"
import { ChartContainer, ChartLegend, ChartLegendContent, ChartTooltip, ChartTooltipContent } from "@/components/ui/chart"
import { TableView } from "@/components/landing/charts/TableView"
import type { WeekCount } from "@/lib/bindings"
import { formatChartWeek } from "@/lib/robot-stats"

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

/** A part of a bar, its end rounded when no part lies above it in its week */
function partShape(part: Part) {
  return function PartShape(props: BarShapeProps) {
    const week = props.payload as Record<Part, number> & { current: boolean }
    const above = PARTS.slice(PARTS.indexOf(part) + 1).some(next => week[next] > 0)
    return <Rectangle {...props} radius={above ? 0 : [4, 4, 0, 0]} fillOpacity={week.current ? SO_FAR_OPACITY : 1} />
  }
}

const SHAPES = { merged: partShape("merged"), failed: partShape("failed"), canceled: partShape("canceled") }

/**
 * The finished robots of each week, oldest first, stacked from the baseline: merged, failed, canceled. Bars of at most
 * 24px with a 2px card gap between the parts, the rounded end on the top part, hairline grid; the week of now named
 * "This week" and muted, as its robots so far; the legend names the parts, the tooltip the week, the table twin every
 * value
 */
export function WeekChart({ weeks }: { weeks: readonly WeekCount[] }) {
  const data = weeks.map(week => ({ week: formatChartWeek(week), current: week.current, merged: week.merged, failed: week.failed, canceled: week.canceled }))
  return (
    <figure className="flex flex-col" data-testid="week-chart">
      <ChartContainer id="weeks" config={config} className="aspect-auto h-44 w-full">
        <BarChart data={data} accessibilityLayer margin={{ top: 4, right: 0, bottom: 0, left: 0 }}>
          <CartesianGrid vertical={false} />
          <XAxis dataKey="week" tickLine={false} axisLine={false} tickMargin={6} interval="preserveStartEnd" minTickGap={24} />
          <YAxis tickLine={false} axisLine={false} width={28} allowDecimals={false} />
          <ChartTooltip cursor={false} content={<ChartTooltipContent labelFormatter={label => <>{weekLabel(label)}</>} />} />
          {/* in the order of the stack, not by name */}
          <ChartLegend content={<ChartLegendContent />} itemSorter={item => PARTS.indexOf(item.dataKey as Part)} />
          <Bar dataKey="merged" stackId="robots" fill="var(--color-merged)" stroke="var(--card)" strokeWidth={2} maxBarSize={24} shape={SHAPES.merged} />
          <Bar dataKey="failed" stackId="robots" fill="var(--color-failed)" stroke="var(--card)" strokeWidth={2} maxBarSize={24} shape={SHAPES.failed} />
          <Bar dataKey="canceled" stackId="robots" fill="var(--color-canceled)" stroke="var(--card)" strokeWidth={2} maxBarSize={24} shape={SHAPES.canceled} />
        </BarChart>
      </ChartContainer>
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

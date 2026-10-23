import { BarChart, Rectangle, XAxis, YAxis } from "recharts"
import type { BarShapeProps } from "recharts"
import type { ChartConfig } from "@/components/ui/chart"
import { ChartTooltip, ChartTooltipContent } from "@/components/ui/chart"
import { ChartFigure } from "@/components/landing/charts/ChartFigure"
import { stackedBars } from "@/components/landing/charts/stackedBar"
import type { TimeSpent } from "@/lib/bindings"
import { formatTotalDuration } from "@/lib/duration"

/** The parts of the robot time in the order of the chart palette, the order the colour-blind checks passed in */
const config = {
  merged: { label: "Merged robots", color: "var(--chart-1)" },
  failed: { label: "Failed robots", color: "var(--chart-2)" },
  canceled: { label: "Canceled robots", color: "var(--chart-3)" },
  between: { label: "Waiting to restart", color: "var(--chart-4)" },
} satisfies ChartConfig

const PARTS = ["merged", "failed", "canceled", "between"] as const
type Part = (typeof PARTS)[number]

/** A segment, the outer ends of the bar rounded */
function segmentShape(part: Part) {
  return function SegmentShape(props: BarShapeProps) {
    const row = props.payload as Record<Part, number>
    const shown = PARTS.filter(name => row[name] > 0)
    const first = shown[0] === part
    const last = shown.at(-1) === part
    return <Rectangle {...props} radius={[first ? 4 : 0, last ? 4 : 0, last ? 4 : 0, first ? 4 : 0]} />
  }
}

const SHAPES = Object.fromEntries(PARTS.map(part => [part, segmentShape(part)])) as Record<Part, ReturnType<typeof segmentShape>>

function share(seconds: number, total: number): string {
  return `${total === 0 ? 0 : Math.round((seconds / total) * 100)}%`
}

/**
 * Where the user's robot time went, as one bar of 24px: each part by its share, a 2px card gap between them; the key
 * under it names each part with its hours and share (the values in text, never only in a colour), the tooltip says it too
 */
export function TimeChart({ time }: { time: TimeSpent }) {
  const total = time.merged + time.failed + time.canceled + time.between
  const row = { name: "Robot time", merged: time.merged, failed: time.failed, canceled: time.canceled, between: time.between }
  return (
    <ChartFigure
      testId="time-chart"
      id="time"
      config={config}
      className="aspect-auto h-6 w-full"
      chart={
        <BarChart data={[row]} layout="vertical" accessibilityLayer margin={{ top: 0, right: 0, bottom: 0, left: 0 }}>
          <XAxis type="number" hide domain={[0, Math.max(1, total)]} />
          <YAxis type="category" dataKey="name" hide />
          <ChartTooltip
            cursor={false}
            content={<ChartTooltipContent hideLabel formatter={(value, name) => <TimePart name={String(name)} seconds={Number(value)} total={total} />} />}
          />
          {stackedBars(PARTS, "time", SHAPES)}
        </BarChart>
      }
      keyTestId="time-legend"
      keyItems={PARTS.map(part => ({
        label: config[part].label,
        color: config[part].color,
        mark: "rect",
        value: (
          <>
            <span className="font-medium text-foreground tabular-nums">{formatTotalDuration(time[part])}</span>
            {total > 0 && <span className="text-muted-foreground tabular-nums">{share(time[part], total)}</span>}
          </>
        ),
      }))}
      table={{
        caption: "Robot time",
        columns: ["Part", "Time", "Share"],
        rows: PARTS.map(part => ({ key: part, cells: [config[part].label, formatTotalDuration(time[part]), total === 0 ? "–" : share(time[part], total)] })),
      }}
    />
  )
}

function TimePart({ name, seconds, total }: { name: string; seconds: number; total: number }) {
  const label = config[name as Part].label
  return (
    <span className="flex w-full justify-between gap-3">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-medium text-foreground tabular-nums">{`${formatTotalDuration(seconds)} · ${share(seconds, total)}`}</span>
    </span>
  )
}

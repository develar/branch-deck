import { Bar, BarChart, LabelList, XAxis, YAxis } from "recharts"
import type { ChartConfig } from "@/components/ui/chart"
import { ChartContainer } from "@/components/ui/chart"
import { TableView } from "@/components/landing/charts/TableView"
import type { KindCount } from "@/lib/bindings"
import { formatFailureKind } from "@/lib/robot-stats"

/** Failed robots wear the colour of Failed in the weekly chart (`--chart-2`): one series, no legend */
const config = { count: { label: "Failed robots", color: "var(--chart-2)" } } satisfies ChartConfig

/** The failed robots by what failed them, the most first: a bar per kind with its count at the tip */
export function KindChart({ kinds }: { kinds: readonly KindCount[] }) {
  const data = kinds.map(kind => ({ kind: formatFailureKind(kind.kind), count: kind.count }))
  return (
    <figure data-testid="kind-chart">
      <ChartContainer id="kinds" config={config} className="aspect-auto w-full" style={{ height: data.length * 28 + 8 }}>
        <BarChart data={data} layout="vertical" accessibilityLayer margin={{ top: 4, right: 32, bottom: 4, left: 0 }} barCategoryGap={6}>
          <XAxis type="number" hide />
          <YAxis type="category" dataKey="kind" tickLine={false} axisLine={false} width={136} />
          <Bar dataKey="count" fill="var(--color-count)" radius={[0, 4, 4, 0]} maxBarSize={20} isAnimationActive={false}>
            <LabelList dataKey="count" position="right" className="fill-foreground" fontSize={12} />
          </Bar>
        </BarChart>
      </ChartContainer>
      <TableView caption="Failed robots by kind" columns={["Kind", "Robots"]} rows={data.map(row => ({ key: row.kind, cells: [row.kind, row.count] }))} />
    </figure>
  )
}

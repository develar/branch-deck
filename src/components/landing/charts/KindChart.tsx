import { RankedList } from "@/components/landing/charts/RankedList"
import { TableView } from "@/components/landing/charts/TableView"
import type { KindCount } from "@/lib/bindings"
import { formatFailureKind } from "@/lib/robot-stats"

/**
 * The failed robots by what failed them, the most first, in the language of the check lists beside it: the bar of a kind
 * is its share of the `failed` robots, in the colour of Failed (`--chart-2`); the table twin names every value
 */
export function KindChart({ kinds, failed }: { kinds: readonly KindCount[]; failed: number }) {
  const data = kinds.map(kind => ({ kind: formatFailureKind(kind.kind), count: kind.count }))
  return (
    <figure className="flex min-w-0 flex-col" data-testid="kind-chart">
      <RankedList
        title="What failed"
        testId="landing-failure-kinds"
        tone="failed"
        scale={failed}
        items={data.map(row => ({ name: row.kind, count: row.count, value: String(row.count) }))}
      />
      <TableView caption="Failed robots by kind" columns={["Kind", "Robots"]} rows={data.map(row => ({ key: row.kind, cells: [row.kind, row.count] }))} />
    </figure>
  )
}

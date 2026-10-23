import type { ComponentProps } from "react"
import type { ChartConfig } from "@/components/ui/chart"
import { ChartContainer } from "@/components/ui/chart"
import type { ChartKeyItem } from "@/components/landing/charts/ChartKey"
import { ChartKey } from "@/components/landing/charts/ChartKey"
import type { TableRow } from "@/components/landing/charts/TableView"
import { TableView } from "@/components/landing/charts/TableView"

/** The table twin of a chart: its caption, the columns and a row per value */
export interface ChartTable {
  readonly caption: string
  readonly columns: readonly string[]
  readonly rows: readonly TableRow[]
}

/**
 * A chart of the Landing view as one figure: the chart in its container, the key under it (every value in text, never
 * only in a colour) and the table twin for the screen reader
 */
export function ChartFigure({
  testId,
  id,
  config,
  className,
  chart,
  keyTestId,
  keyItems,
  table,
}: {
  testId: string
  id: string
  config: ChartConfig
  className: string
  chart: ComponentProps<typeof ChartContainer>["children"]
  keyTestId: string
  keyItems: readonly ChartKeyItem[]
  table: ChartTable
}) {
  return (
    <figure className="flex flex-col gap-2" data-testid={testId}>
      <ChartContainer id={id} config={config} className={className}>
        {chart}
      </ChartContainer>
      <ChartKey testId={keyTestId} items={keyItems} />
      <TableView caption={table.caption} columns={table.columns} rows={table.rows} />
    </figure>
  )
}

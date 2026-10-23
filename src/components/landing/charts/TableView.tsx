import type { ReactNode } from "react"

export interface TableRow {
  readonly key: string
  readonly cells: readonly ReactNode[]
}

/**
 * The table twin of a chart, for the screen reader: every value of the chart in rows, so no value is read only from a
 * colour or a tooltip (the dataviz rule "tooltips enhance, never gate")
 */
export function TableView({ caption, columns, rows }: { caption: string; columns: readonly string[]; rows: readonly TableRow[] }) {
  return (
    <table className="sr-only">
      <caption>{caption}</caption>
      <thead>
        <tr>
          {columns.map(column => (
            <th key={column} scope="col">
              {column}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map(row => (
          <tr key={row.key}>
            {row.cells.map((cell, column) => (
              <td key={columns[column]}>{cell}</td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  )
}

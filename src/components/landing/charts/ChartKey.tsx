import type { ReactNode } from "react"

export interface ChartKeyItem {
  readonly label: string
  readonly color: string
  /** The mark the key mirrors: a rect for a bar, a line for a line, a dot for a dot */
  readonly mark: "rect" | "line" | "dot"
  /** The value beside the name, in the text tokens */
  readonly value?: ReactNode
}

const MARK = { rect: "size-2 shrink-0", line: "h-0.5 w-3 shrink-0", dot: "size-2 shrink-0 rounded-full" } as const

/**
 * The key of a chart with two series or more, left-aligned under the plot as the rest of the island: each item its mark
 * in the series colour, its name and its value in the text tokens, so no value is read from a colour alone
 */
export function ChartKey({ items, testId }: { items: readonly ChartKeyItem[]; testId: string }) {
  return (
    <ul className="flex flex-wrap gap-x-4 gap-y-1 text-xs" data-testid={testId}>
      {items.map(item => (
        <li key={item.label} className="flex items-center gap-1.5">
          <span aria-hidden className={MARK[item.mark]} style={{ backgroundColor: item.color }} />
          <span className="text-muted-foreground">{item.label}</span>
          {item.value}
        </li>
      ))}
    </ul>
  )
}

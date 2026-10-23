import { openRobotUrl } from "@/lib/patronus"

export interface RankedItem {
  readonly name: string
  readonly count: number
  /** The value shown at the end of the row: "19", "68% · 99 of 145" */
  readonly value: string
  /** The newest robot of the item, opened by its row */
  readonly url?: string
  /** What the item did in that robot: "failed", "passed on a retry" */
  readonly robot?: string
}

/**
 * Items ranked by their count, the most first, in one language for every list of the failures island: the name, a thin
 * bar in the colour of the item's entity (Failed, Merged) and the value in the text tokens. The bar scales the count to
 * `scale`: the first item's count (the ranking) or a total (the share). A row with a robot opens it in Patronus
 */
export function RankedList({
  title,
  items,
  tone,
  scale,
  testId,
}: {
  title: string
  items: readonly RankedItem[]
  tone: "failed" | "merged"
  scale: "first" | number
  testId: string
}) {
  if (items.length === 0) {
    return null
  }
  const most = scale === "first" ? Math.max(...items.map(item => item.count)) : Math.max(1, scale)
  const row = "col-span-3 grid grid-cols-subgrid items-center gap-2 rounded px-1 py-0.5 text-left"
  return (
    <section aria-label={title} className="flex min-w-0 flex-col gap-1" data-testid={testId}>
      <h3 className="text-xs font-medium text-muted-foreground">{title}</h3>
      <ul className="-mx-1 grid grid-cols-[minmax(0,3fr)_minmax(2rem,2fr)_auto] text-xs">
        {items.map(item => {
          const url = item.url
          const cells = (
            <>
              <span className="min-w-0 truncate text-foreground">{item.name}</span>
              <span aria-hidden className="h-1.5 overflow-hidden rounded-full bg-muted">
                <span className={tone === "failed" ? "block h-full bg-chart-2" : "block h-full bg-chart-1"} style={{ width: `${Math.min(100, (item.count / most) * 100)}%` }} />
              </span>
              <span className="text-right text-muted-foreground tabular-nums">{item.value}</span>
            </>
          )
          return (
            <li key={item.name} className="col-span-3 grid grid-cols-subgrid">
              {url === undefined ? (
                <div className={row}>{cells}</div>
              ) : (
                <button
                  type="button"
                  className={`${row} hover:bg-muted focus-visible:bg-muted focus-visible:outline-none`}
                  aria-label={`${item.name}, ${item.value}: open the newest robot where it ${item.robot ?? "ran"}`}
                  onClick={() => openRobotUrl(url)}
                >
                  {cells}
                </button>
              )}
            </li>
          )
        })}
      </ul>
    </section>
  )
}

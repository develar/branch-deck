import type { MouseEvent, ReactNode } from "react"

function stopPropagation(event: MouseEvent): void {
  event.stopPropagation()
}

/**
 * The buttons of the Actions cell of a branch row. A click in it doesn't toggle the row: also one on a disabled button,
 * which passes the click on, and one in a menu the cell opens (its popup is in the row's React tree).
 */
export function RowActions({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center gap-2" onClick={stopPropagation}>
      {children}
    </div>
  )
}

import type { MouseEvent, ReactNode } from "react"
import { cn } from "cn"
import { TRACK_GROUP_CLASS } from "@/lib/branch-columns"

function stopPropagation(event: MouseEvent): void {
  event.stopPropagation()
}

/**
 * The buttons of the Actions cell of a branch row or of the header of a stack island, each in its track
 * (`ColumnTrack`: the push, the next step, "⋯"), so they line up across the rows of every card whatever the others
 * show. A click in it doesn't toggle the row: also one on a disabled button, which passes the click on, and one in a
 * menu the cell opens (its popup is in the row's React tree).
 */
export function ActionTracks({ children, "data-testid": testId }: { children: ReactNode; "data-testid"?: string }) {
  return (
    <div className={cn("flex items-center", TRACK_GROUP_CLASS.actions)} data-testid={testId} onClick={stopPropagation}>
      {children}
    </div>
  )
}

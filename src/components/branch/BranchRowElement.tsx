import type { DragEvent, MouseEvent, ReactNode } from "react"
import { cn } from "cn"
import type { RowDivider } from "@/lib/stack-lanes"
import { dividerAttributes } from "@/lib/stack-lanes"

interface BranchRowElementProps {
  name: string
  /** Rendered as `data-row-id`: the row opens a preview on Space (`usePreviewHotkeys`) */
  rowId?: string
  expanded: boolean
  /** an inline action is running on the row: it opens no menu (`data-processing`) */
  processing: boolean
  /** the row is a Tailwind `group`, for cells that react to the row hover */
  group?: boolean
  className?: string
  "data-testid"?: string
  /** Nesting level of the row, rendered as `data-depth` when it is nested */
  depth?: number
  /** The lane of its node in the stack graph, rendered as `data-lane`; null without stacks */
  lane?: number | null
  /** Shown as context of the filter, not a match: faded until hovered or focused (`data-dimmed`) */
  dimmed?: boolean
  /** Where the divider above the row starts (`data-divider`, `bd-branch-rows`): full width without */
  divider?: RowDivider
  /** Commits from another list are dragged ("ready"), over the row ("over"), or the row refuses them ("refused") */
  dropTarget?: "ready" | "over" | "refused"
  /** The row is dragged (`data-dragging`) */
  dragging?: boolean
  dropProps?: {
    onDragEnter?: (event: DragEvent) => void
    onDragOver?: (event: DragEvent) => void
    onDragLeave?: (event: DragEvent) => void
    onDrop?: (event: DragEvent) => void
  }
  /** The row is selected (`data-selected`) */
  selected?: boolean
  /** The row can be dragged: a drag of the branch, or of the selected branches */
  onDragStart?: (event: DragEvent) => void
  onDragEnd?: () => void
  /** A click on the row: it toggles the expansion, or, with a modifier, the selection */
  onToggle: (event: MouseEvent) => void
  /** the row cells */
  children: ReactNode
}

/**
 * Table row of a branch that toggles its expansion on click; its right-click opens the menu of the branch, which the
 * table renders for all its rows (`TableContextMenu`, the body a `TableMenuBody`), found by `data-branch-name`. Not
 * selectable, as a menu trigger. Focusable (not a tab stop): an inline form of the row gives the focus back to it when it
 * closes.
 */
export function BranchRowElement({
  name,
  rowId,
  expanded,
  processing,
  group = false,
  className,
  "data-testid": testId,
  depth = 0,
  lane = null,
  dimmed = false,
  divider,
  dropTarget,
  dragging = false,
  dropProps,
  selected,
  onDragStart,
  onDragEnd,
  onToggle,
  children,
}: BranchRowElementProps) {
  return (
    <tr
      data-testid={testId}
      data-branch-name={name}
      data-row-id={rowId}
      data-depth={depth > 0 ? depth : undefined}
      data-lane={lane ?? undefined}
      data-dimmed={dimmed || undefined}
      {...dividerAttributes(divider)}
      data-state={expanded ? "open" : "closed"}
      data-drop-target={dropTarget}
      data-dragging={dragging || undefined}
      data-selected={selected === undefined ? undefined : String(selected)}
      data-processing={processing || undefined}
      tabIndex={-1}
      draggable={onDragStart === undefined ? undefined : true}
      className={cn(
        group && "group",
        "cursor-pointer transition-all outline-none hover:bg-muted focus-visible:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30 focus-visible:ring-inset",
        selected === true && "bg-primary/15 hover:bg-primary/20",
        dimmed && "opacity-55 hover:opacity-100 focus-visible:opacity-100",
        className,
        "select-none",
      )}
      onClick={onToggle}
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      {...dropProps}
    >
      {children}
    </tr>
  )
}

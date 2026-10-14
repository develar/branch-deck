import type { DragEvent, ReactNode } from "react"
import { cn } from "cn"
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "@/components/ui/context-menu"

interface BranchContextMenuRowProps {
  "name": string
  /** Rendered as `data-row-id`: the row opens a preview on Space (`useSpacePreviewHotkey`) */
  "rowId"?: string
  "expanded": boolean
  /** an inline action is running on the row: the menu is disabled */
  "processing": boolean
  /** the row is a Tailwind `group`, for cells that react to the row hover */
  "group"?: boolean
  "className"?: string
  "data-testid"?: string
  /** Commits from another list are dragged ("ready") or over the row ("over") */
  "dropTarget"?: "ready" | "over"
  "dropProps"?: {
    onDragEnter?: (event: DragEvent) => void
    onDragOver?: (event: DragEvent) => void
    onDragLeave?: (event: DragEvent) => void
    onDrop?: (event: DragEvent) => void
  }
  "onToggle": () => void
  /** the menu items, rendered only while the menu is open */
  "menu": ReactNode
  /** the row cells */
  "children": ReactNode
}

/** Table row of a branch that toggles its expansion on click and opens the branch context menu on right-click. */
export function BranchContextMenuRow({ name, rowId, expanded, processing, group = false, className, "data-testid": testId, dropTarget, dropProps, onToggle, menu, children }: BranchContextMenuRowProps) {
  return (
    <ContextMenu disabled={processing}>
      <ContextMenuTrigger
        render={(
          <tr
            data-testid={testId}
            data-branch-name={name}
            data-row-id={rowId}
            data-state={expanded ? "open" : "closed"}
            data-drop-target={dropTarget}
            className={cn(group && "group", "cursor-pointer transition-all hover:bg-muted", className)}
            onClick={onToggle}
            {...dropProps}
          />
        )}
      >
        {children}
      </ContextMenuTrigger>
      {/* the inline form opened from the menu takes the focus */}
      <ContextMenuContent finalFocus={false}>
        {menu}
      </ContextMenuContent>
    </ContextMenu>
  )
}

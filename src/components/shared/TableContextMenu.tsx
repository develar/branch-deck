import { createContext, use, useState } from "react"
import type { ComponentProps, ReactNode } from "react"
import { ContextMenu as ContextMenuPrimitive } from "@base-ui/react/context-menu"
import { ContextMenu, ContextMenuContent } from "@/components/ui/context-menu"

/** Names the row the menu opens for, set by the right-click on it */
const MenuRowContext = createContext<((name: string) => void) | null>(null)

export interface TableContextMenuProps {
  /** The items of the menu of the row of `name` (`ContextMenuItems`), rendered only while the menu is open */
  menu: (name: string) => ReactNode
  /** The tables, each body a `TableMenuBody` */
  children: ReactNode
}

/**
 * One context menu for the rows of the tables under it: a right-click on a row (`BranchRowElement`, a
 * `tr[data-branch-name]`) opens the menu of that row at the pointer, as a menu of its own would. One root for a table
 * instead of one per row: the rows of a large table mounted 141 of them, each with its menu store and its list
 * (docs/design/rendering-performance.md, Key Decision 7).
 */
export function TableContextMenu({ menu, children }: TableContextMenuProps) {
  const [row, setRow] = useState<string | null>(null)
  return (
    <ContextMenu>
      <MenuRowContext value={setRow}>{children}</MenuRowContext>
      {/* the inline form opened from the menu takes the focus */}
      <ContextMenuContent finalFocus={false}>{row !== null && menu(row)}</ContextMenuContent>
    </ContextMenu>
  )
}

/**
 * The body of a table under `TableContextMenu`, where its right-click starts: on a row it opens the menu of the row,
 * unless an operation works on the row (`data-processing`), which takes no other action meanwhile; elsewhere (the
 * commits under a row, an inline form) the right-click is the page's own, the copy of a selected text included. Without
 * the `select-none` of the trigger of `ContextMenu`: the commits under a row stay selectable.
 */
export function TableMenuBody(props: ComponentProps<"tbody">) {
  const setRow = use(MenuRowContext)
  return (
    <ContextMenuPrimitive.Trigger
      render={<tbody {...props} />}
      onContextMenu={event => {
        const row = event.target instanceof Element ? event.target.closest<HTMLElement>("tr[data-branch-name]") : null
        const name = row?.dataset.branchName
        if (row == null || name === undefined || setRow === null) {
          // the menu of the page: the trigger would hold it back for anything in it
          event.preventBaseUIHandler()
          event.stopPropagation()
        } else if (row.dataset.processing !== undefined) {
          event.preventBaseUIHandler()
        } else {
          setRow(name)
        }
      }}
    />
  )
}

import type { ReactNode } from "react"
import { EllipsisIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { DropdownMenu, DropdownMenuContent, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"

export interface MoreActionsMenuProps {
  /** The object of the actions, for the accessible name: "More actions for feature-auth" */
  name: string
  /** The items (`DropdownMenuItems`), rendered only while the menu is open */
  menu: ReactNode
  /** An operation works on the row: no menu, as the right-click opens none */
  disabled?: boolean
  /**
   * The busy state of an action of the object started from its menu, a key or a toast ("Unstacking tool-events"): the
   * button shows its spinner, the hover says it
   */
  busy?: string | null
}

/**
 * The "⋯" button of a row, inside its `RowActions` (a click doesn't toggle the row): the menu its right-click opens,
 * under a button, so its actions are found without knowing the right-click.
 */
export function MoreActionsMenu({ name, menu, disabled = false, busy = null }: MoreActionsMenuProps) {
  return (
    <DropdownMenu>
      <Tooltip>
        <TooltipTrigger
          render={
            <DropdownMenuTrigger
              disabled={disabled || busy !== null}
              render={<Button variant="ghost" size="icon-xs" className="text-muted-foreground" aria-label={`More actions for ${name}`} aria-busy={busy !== null || undefined} />}
            />
          }
        >
          {busy === null ? <EllipsisIcon /> : <Spinner />}
        </TooltipTrigger>
        <TooltipContent>{busy === null ? "More actions" : `${busy}…`}</TooltipContent>
      </Tooltip>
      {/*
        as wide as its items, as the right-click menu; the inline form opened from the menu takes the focus. The popup
        sits in the row's React tree: a right-click in it must not open the row's menu
      */}
      <DropdownMenuContent
        align="end"
        className="w-auto min-w-36"
        finalFocus={false}
        onContextMenu={event => {
          event.preventDefault()
          event.stopPropagation()
        }}
      >
        {menu}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

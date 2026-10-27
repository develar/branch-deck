import type { ReactNode } from "react"
import { EllipsisIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { DropdownMenu, DropdownMenuContent, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import type { MenuItem } from "@/lib/menu"

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
  /**
   * Stop Waiting of the busy action while its command waits for another holder of the repository lock
   * (`presentStopWaiting`): the spinner opens a menu of it
   */
  stopWaiting?: MenuItem | null
}

/**
 * The "⋯" button of a row, inside its `ActionTracks` or `RowActions` (a click doesn't toggle the row): the menu its right-click opens,
 * under a button, so its actions are found without knowing the right-click.
 */
export function MoreActionsMenu({ name, menu, disabled = false, busy = null, stopWaiting = null }: MoreActionsMenuProps) {
  if (busy !== null && stopWaiting !== null) {
    // the action waits for another holder of the repository lock: the hover says whom for, a click offers Stop Waiting
    return (
      <DropdownMenu>
        <Tooltip>
          <TooltipTrigger
            render={<DropdownMenuTrigger render={<Button variant="ghost" size="icon-xs" className="text-muted-foreground" aria-label={`More actions for ${name}`} aria-busy />} />}
          >
            <Spinner />
          </TooltipTrigger>
          <TooltipContent>{`${busy}…`}</TooltipContent>
        </Tooltip>
        <DropdownMenuContent align="end" className="w-auto min-w-36" finalFocus={false}>
          <DropdownMenuItems groups={[[stopWaiting]]} />
        </DropdownMenuContent>
      </DropdownMenu>
    )
  }
  if (busy !== null) {
    // no menu while it runs; the hover says what runs (Base UI keeps a disabled button hoverable here, the dimming is off)
    return (
      <Tooltip>
        <TooltipTrigger
          render={<Button variant="ghost" size="icon-xs" className="text-muted-foreground" aria-label={`More actions for ${name}`} aria-busy disabled focusableWhenDisabled />}
        >
          <Spinner />
        </TooltipTrigger>
        <TooltipContent>{`${busy}…`}</TooltipContent>
      </Tooltip>
    )
  }
  return (
    <DropdownMenu>
      <Tooltip>
        <TooltipTrigger
          render={
            <DropdownMenuTrigger disabled={disabled} render={<Button variant="ghost" size="icon-xs" className="text-muted-foreground" aria-label={`More actions for ${name}`} />} />
          }
        >
          <EllipsisIcon />
        </TooltipTrigger>
        <TooltipContent>More actions</TooltipContent>
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

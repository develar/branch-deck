import type { ReactNode } from "react"
import { EllipsisIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { DropdownMenu, DropdownMenuContent, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { useLazyRoot } from "@/hooks/useLazyRoot"
import type { MenuItem } from "@/lib/menu"
import { CONTROL_KEY } from "@/lib/pointer-moves"

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
 * under a button, so its actions are found without knowing the right-click. Its menu and its tooltip mount at the first
 * hover or focus of the button (`useLazyRoot`), a plain button with the ARIA of a closed menu trigger until then: every
 * row has one.
 */
export function MoreActionsMenu({ name, menu, disabled = false, busy = null, stopWaiting = null }: MoreActionsMenuProps) {
  const root = useLazyRoot(true)
  // each state renders another button in the place of the last: one control for the hover (`CONTROL_KEY`)
  const control = { [CONTROL_KEY]: `more-actions:${name}` }
  if (!root.armed) {
    // the button of the branch below that the state renders, closed: a menu unless an action runs without Stop Waiting
    const menuTrigger = busy === null || stopWaiting !== null
    return (
      <Button
        variant="ghost"
        size="icon-xs"
        className="text-muted-foreground"
        aria-label={`More actions for ${name}`}
        {...control}
        aria-haspopup={menuTrigger ? "menu" : undefined}
        aria-expanded={menuTrigger ? "false" : undefined}
        aria-busy={busy === null ? undefined : true}
        data-slot="tooltip-trigger"
        disabled={busy === null ? disabled : !menuTrigger}
        focusableWhenDisabled={!menuTrigger}
        {...root.armProps}
      >
        {busy === null ? <EllipsisIcon /> : <Spinner />}
      </Button>
    )
  }
  if (busy !== null && stopWaiting !== null) {
    // the action waits for another holder of the repository lock: the hover says whom for, a click offers Stop Waiting
    return (
      <DropdownMenu>
        <Tooltip>
          <TooltipTrigger
            render={
              <DropdownMenuTrigger
                render={<Button variant="ghost" size="icon-xs" className="text-muted-foreground" aria-label={`More actions for ${name}`} {...control} aria-busy />}
              />
            }
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
          render={
            <Button
              variant="ghost"
              size="icon-xs"
              className="text-muted-foreground"
              aria-label={`More actions for ${name}`}
              {...control}
              aria-busy
              disabled
              focusableWhenDisabled
            />
          }
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
            <DropdownMenuTrigger
              disabled={disabled}
              render={<Button variant="ghost" size="icon-xs" className="text-muted-foreground" aria-label={`More actions for ${name}`} {...control} />}
            />
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

import { useRef, useState } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { EllipsisIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { DropdownMenu, DropdownMenuContent, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { useControlRef } from "@/hooks/useControlKey"
import { useLazyRoot } from "@/hooks/useLazyRoot"
import { focusBackFromMenu } from "@/lib/menu"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"

export interface MoreActionsMenuProps {
  /** The object of the actions, for the accessible name: "More actions for feature-auth" */
  name: string
  /**
   * The items (`DropdownMenuItems`), rendered only while the menu is open; or the builder of the menu of a row,
   * called as it opens, from the selection at that moment, as its right-click builds it (`LazyContextMenu`), never in
   * the render of the row, where React Compiler would keep the groups of an older selection
   */
  menu: ReactNode | (() => MenuItemGroups)
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
  /**
   * The menu open, kept by its owner: a button that mounts anew in the place of one whose menu was open (the heading of
   * a commit the review moves as its diffs load) opens it again; uncontrolled without
   */
  open?: boolean
  onOpenChange?: (open: boolean) => void
}

/**
 * The "⋯" button of a row, inside its `ActionTracks` or `RowActions` (a click doesn't toggle the row): the menu its right-click opens,
 * under a button, so its actions are found without knowing the right-click. Its menu and its tooltip mount at the first
 * hover or focus of the button (`useLazyRoot`), a plain button with the ARIA of a closed menu trigger until then: every
 * row has one. One tree in every state (an action of the row runs: its spinner, its hover what runs, no menu; it waits
 * for another holder of the lock: its menu is Stop Waiting), so the button keeps its element, its focus and its hover
 * as the action runs, waits and ends; disabled, it keeps the focus (`aria-disabled`).
 */
export function MoreActionsMenu({ name, menu, disabled = false, busy = null, stopWaiting = null, open, onOpenChange }: MoreActionsMenuProps) {
  // a menu its owner keeps open mounts its root at once
  const root = useLazyRoot(true, open === true)
  // the plain button and the one of the roots are one control for the hover
  const controlKey = `more-actions:${name}`
  const plainRef = useControlRef<HTMLButtonElement>(undefined, controlKey)
  // the popup of the menu, kept after it unmounts: its `finalFocus` asks as the popup goes whether the focus is in it
  const popup = useRef<HTMLDivElement | null>(null)
  // the groups of a builder, built as the menu opens, and at once for a menu that mounts open
  const [built, setBuilt] = useState<MenuItemGroups>(() => (open === true && typeof menu === "function" ? menu() : []))
  const running = busy !== null
  // a menu unless an action runs without Stop Waiting, or an operation works on the row
  const opensMenu = running ? stopWaiting !== null : !disabled
  const button = {
    variant: "ghost",
    size: "icon-xs",
    // a running action keeps its colour beside the spinner: no dimming then
    className: cn("text-muted-foreground", !running && "aria-disabled:opacity-50"),
    "aria-label": `More actions for ${name}`,
    "aria-busy": running || undefined,
    disabled: !opensMenu,
    focusableWhenDisabled: true,
  } as const
  const icon = running ? <Spinner /> : <EllipsisIcon />
  if (!root.armed) {
    return (
      <Button {...button} aria-haspopup="menu" aria-expanded="false" data-slot="tooltip-trigger" ref={plainRef} {...root.armProps}>
        {icon}
      </Button>
    )
  }
  return (
    <DropdownMenu
      controlKey={controlKey}
      {...(open !== undefined && { open })}
      onOpenChange={(next, details) => {
        if (next && !opensMenu) {
          details.cancel()
          return
        }
        if (next && typeof menu === "function") {
          setBuilt(menu())
        }
        onOpenChange?.(next)
      }}
    >
      <Tooltip controlKey={controlKey}>
        <TooltipTrigger render={<DropdownMenuTrigger render={<Button {...button} />} />}>{icon}</TooltipTrigger>
        <TooltipContent>{running ? `${busy}…` : "More actions"}</TooltipContent>
      </Tooltip>
      {/*
        as wide as its items, as the right-click menu; the inline form opened from the menu takes the focus, any other
        item gives it back here. The popup sits in the row's React tree: a right-click in it must not open the row's menu
      */}
      <DropdownMenuContent
        ref={element => {
          if (element !== null) {
            popup.current = element
          }
        }}
        align="end"
        className="w-auto min-w-36"
        finalFocus={() => focusBackFromMenu(popup.current)}
        onContextMenu={event => {
          event.preventDefault()
          event.stopPropagation()
        }}
      >
        {running ? stopWaiting !== null && <DropdownMenuItems groups={[[stopWaiting]]} /> : typeof menu === "function" ? <DropdownMenuItems groups={built} /> : menu}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

import { useEffect, useEffectEvent, useRef, useState } from "react"
import { cn } from "cn"
import { MenuIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Menubar, MenubarContent, MenubarMenu, MenubarTrigger } from "@/components/ui/menubar"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { useSyncAvailability } from "@/hooks/useSyncAvailability"
import { mainMenus, matchMainMenuOpenKey } from "@/lib/main-menu"
import type { MainMenu, MainMenuId } from "@/lib/main-menu"
import { isWindows } from "@/lib/platform"
import { updatesItself } from "@/lib/title-bar"
import { useAppSettings } from "@/stores/settings/appSettings"

/**
 * The main menu of a custom title bar (Windows and Linux), IntelliJ's ☰ (`MainMenuButton`, `ExpandableMenu`): a click
 * expands the menu bar in place, over the start of the row, with File open; ←/→ go through the menus; Esc, a click
 * outside or a chosen item collapse it to ☰ again. Alt+\, Alt+F/V/H and F10 open it from the keyboard, the mnemonics
 * underlined; on Windows a lone Alt tap highlights File, as the native menu bar did.
 */
export function MainMenuButton() {
  const sync = useSyncAvailability()
  const autoSyncOnFocus = useAppSettings(state => state.autoSyncOnFocus)
  const menus = mainMenus({ sync, autoSyncOnFocus, updates: updatesItself })
  // the open menu; `highlighted`: expanded without an open menu (the Alt tap), File focused
  const [open, setOpen] = useState<MainMenuId | null>(null)
  const [highlighted, setHighlighted] = useState(false)
  const [fromKeyboard, setFromKeyboard] = useState(false)
  const expanded = open !== null || highlighted
  const buttonRef = useRef<HTMLButtonElement>(null)
  const firstTriggerRef = useRef<HTMLButtonElement>(null)

  const collapse = (focusButton: boolean) => {
    setOpen(null)
    setHighlighted(false)
    setFromKeyboard(false)
    if (focusButton) {
      // after the menu bar unmounts
      requestAnimationFrame(() => buttonRef.current?.focus())
    }
  }

  const openMenu = (id: MainMenuId, keyboard: boolean) => {
    setHighlighted(false)
    setFromKeyboard(keyboard)
    setOpen(id)
  }

  // the keys that open it work in the whole window; the Windows Alt tap arms on a lone Alt and fires on its release
  const altTap = useRef(false)
  const handleKeyDown = useEffectEvent((event: KeyboardEvent) => {
    if (event.key === "Alt" && !event.ctrlKey && !event.metaKey && !event.shiftKey) {
      // held Alt repeats: still a tap when it is released, as the native menu bar has it
      if (!event.repeat) {
        altTap.current = isWindows
      }
      return
    }
    altTap.current = false
    const id = event.defaultPrevented ? null : matchMainMenuOpenKey(event)
    if (id !== null) {
      event.preventDefault()
      openMenu(id, true)
    }
  })
  const handleKeyUp = useEffectEvent((event: KeyboardEvent) => {
    if (event.key !== "Alt" || !altTap.current) {
      return
    }
    altTap.current = false
    event.preventDefault()
    if (expanded) {
      collapse(false)
    } else {
      setFromKeyboard(true)
      setHighlighted(true)
      requestAnimationFrame(() => firstTriggerRef.current?.focus())
    }
  })
  useEffect(() => {
    const keyDown = (event: KeyboardEvent) => handleKeyDown(event)
    const keyUp = (event: KeyboardEvent) => handleKeyUp(event)
    // another key, a click or leaving the window between Alt down and up is no tap (Alt+Tab, Alt+drag)
    const disarm = () => {
      altTap.current = false
    }
    window.addEventListener("keydown", keyDown)
    window.addEventListener("keyup", keyUp)
    window.addEventListener("mousedown", disarm, { capture: true })
    window.addEventListener("blur", disarm)
    return () => {
      window.removeEventListener("keydown", keyDown)
      window.removeEventListener("keyup", keyUp)
      window.removeEventListener("mousedown", disarm, { capture: true })
      window.removeEventListener("blur", disarm)
    }
  }, [])

  return (
    // the ☰ keeps its place in the row; the expanded menu bar lies over what follows it, which hides meanwhile
    // (`WindowTitleBar`), as IntelliJ's menu covers the project widget
    <div className="relative flex shrink-0 items-center" data-main-menu={expanded ? "expanded" : "collapsed"}>
      <Tooltip>
        <TooltipTrigger
          render={
            <Button
              ref={buttonRef}
              variant="ghost"
              size="icon-sm"
              aria-label="Main Menu"
              aria-expanded={expanded}
              className={cn(expanded && "invisible")}
              data-testid="main-menu-button"
              onClick={() => openMenu("file", false)}
            />
          }
        >
          <MenuIcon />
        </TooltipTrigger>
        <TooltipContent className="flex items-center gap-2">
          Main Menu
          <Kbd>Alt+\</Kbd>
        </TooltipContent>
      </Tooltip>
      {expanded && (
        <Menubar
          className="absolute top-1/2 left-0 z-10 h-8 -translate-y-1/2 gap-0.5 rounded-none border-0 bg-canvas p-0"
          data-testid="main-menu"
          onBlur={event => {
            // a highlighted bar collapses once the focus leaves it; an open menu collapses by its own close
            if (open === null && !event.currentTarget.contains(event.relatedTarget)) {
              collapse(false)
            }
          }}
          onKeyDown={event => {
            if (event.key === "Escape" && open === null) {
              event.preventDefault()
              collapse(true)
            }
          }}
        >
          {menus.map((menu, index) => (
            <MainMenuEntry
              key={menu.id}
              menu={menu}
              open={open === menu.id}
              underline={fromKeyboard}
              triggerRef={index === 0 ? firstTriggerRef : undefined}
              onOpenChange={(next, escape) => {
                if (next) {
                  openMenu(menu.id, fromKeyboard)
                } else if (open === menu.id) {
                  // ←/→ open the next menu before this one closes: only the last close collapses
                  collapse(escape)
                }
              }}
            />
          ))}
        </Menubar>
      )}
    </div>
  )
}

function MainMenuEntry({
  menu,
  open,
  underline,
  triggerRef,
  onOpenChange,
}: {
  menu: MainMenu
  open: boolean
  underline: boolean
  triggerRef: React.RefObject<HTMLButtonElement | null> | undefined
  onOpenChange: (open: boolean, escape: boolean) => void
}) {
  const mnemonicIndex = menu.label.indexOf(menu.mnemonic)
  return (
    <MenubarMenu open={open} onOpenChange={(next, details) => onOpenChange(next, details.reason === "escape-key")}>
      <MenubarTrigger ref={triggerRef} className="h-7 rounded-lg px-2.5 font-normal" data-testid="main-menu-trigger">
        {underline && mnemonicIndex !== -1 ? (
          <span>
            {menu.label.slice(0, mnemonicIndex)}
            <span className="underline underline-offset-2">{menu.mnemonic}</span>
            {menu.label.slice(mnemonicIndex + 1)}
          </span>
        ) : (
          menu.label
        )}
      </MenubarTrigger>
      <MenubarContent className="min-w-56" finalFocus={false}>
        <DropdownMenuItems groups={menu.groups} />
      </MenubarContent>
    </MenubarMenu>
  )
}

import type { LucideIcon } from "lucide-react"
import { modifierKey } from "@/lib/platform"

/** Where an item was chosen: a surface that stays shows the spinner of the action it starts (`useIndicatorClaim`) */
export interface SelectContext {
  /** The key of the button that started it; null for a menu, a key or a toast, whose row shows the spinner */
  readonly surface: string | null
}

/** The context of a choice in a menu, by a key or in a toast */
export const FROM_MENU: SelectContext = { surface: null }

/** Menu item; rendered by `MenuItems` in a context menu (`ContextMenuItems`) or a button's menu (`DropdownMenuItems`). */
export interface MenuItem {
  /**
   * Names the action across the surfaces that render it (a menu, the buttons of a popover or a footer), which pick
   * their items by it; the key of the item, else the label
   */
  id?: string
  label: string
  /**
   * The label in a column of steps (the Safe Push slot of a row and of an island header, the landing overview), where
   * the place says the rest and its hover the detail: "Retarget #7", "Awaiting Review"; `label` everywhere else
   */
  compactLabel?: string
  icon?: LucideIcon
  disabled?: boolean
  /** Keyboard shortcut hints, e.g. `["meta", "c"]` */
  kbds?: string[]
  color?: "default" | "destructive"
  /** Shown on hover beside the item: what it does, or why it is disabled */
  description?: string
  /** A checkbox item ("Auto-sync on Focus"): `onSelect` toggles it */
  checked?: boolean
  /** The action runs in the session (`Availability.running`, `queued`): disabled, with the spinner where it was started */
  running?: boolean
  /** Not called for an item with `children` */
  onSelect: (context: SelectContext) => void
  /** Makes the item a submenu */
  children?: MenuItem[]
}

/** Groups are separated by a separator. */
export type MenuItemGroups = MenuItem[][]

/** The items of `groups` that act, the items of their submenus too, in menu order */
export function actingMenuItems(groups: MenuItemGroups): MenuItem[] {
  const acting = (item: MenuItem): MenuItem[] => (item.children === undefined ? [item] : item.children.flatMap(acting))
  return groups.flat().flatMap(acting)
}

/** The item `id` of `groups` that acts (not a submenu), also inside a submenu, as a menu lists it now */
export function findMenuItem(groups: MenuItemGroups, id: string): MenuItem | undefined {
  return actingMenuItems(groups).find(item => item.id === id)
}

/**
 * Runs the item `id` of `groups` as a click on it does, for a surface that is no menu (a key, the review window): an
 * item disabled now runs nothing and tells why (`onRefused`, with its `description`). Resolves to the item, undefined
 * when the groups have none.
 */
export function selectMenuItemById(groups: MenuItemGroups, id: string, context: SelectContext, onRefused: (item: MenuItem) => void): MenuItem | undefined {
  const item = findMenuItem(groups, id)
  if (item === undefined) {
    return undefined
  }
  if (item.disabled === true) {
    onRefused(item)
  } else {
    item.onSelect(context)
  }
  return item
}

const kbdSymbols: Record<string, string> = {
  // the primary modifier of the platform: ⌘ on macOS, Ctrl elsewhere
  mod: modifierKey,
  meta: "⌘",
  ctrl: "⌃",
  alt: "⌥",
  shift: "⇧",
  enter: "↵",
  escape: "Esc",
  delete: "⌫",
  arrowleft: "←",
  arrowright: "→",
}

/** What a `Kbd` of `MenuItem.kbds` shows: ⌘ or Ctrl for `mod`, the symbol of a modifier, else the key in capitals */
export function kbdLabel(kbd: string): string {
  return kbdSymbols[kbd] ?? kbd.toUpperCase()
}

/**
 * The menu of a row inside its preview: an item that opens a form of the row, a dialog or a window closes the preview
 * first (`close`), a copy keeps it; a key hint stays only for the keys the preview handles itself (`keys`)
 */
export function toPreviewMenu(groups: MenuItemGroups, close: (() => void) | null, keys: ReadonlySet<string>): MenuItemGroups {
  const inPreview = (item: MenuItem): MenuItem => {
    const kbds = item.kbds?.every(kbd => keys.has(kbd)) === true ? item.kbds : undefined
    const keepsPreview = item.label.startsWith("Copy ")
    return {
      ...item,
      kbds,
      onSelect: keepsPreview
        ? item.onSelect
        : context => {
            close?.()
            item.onSelect(context)
          },
      ...(item.children !== undefined && { children: item.children.map(inPreview) }),
    }
  }
  return groups.map(group => group.map(inPreview))
}

/**
 * The `finalFocus` of a menu of a row, whose `popup` it was: an item that opens an inline form gives the focus to it; any
 * other gives it back to the trigger of the menu, so the keys of the row go on (Stop Waiting of the "⋯" by Enter among
 * them). True only while the focus is on the page or in this menu, its popup or a submenu of it (the popups of one root
 * of Base UI share its `data-rootownerid`): another menu that opened as this one closed keeps it. Escape on the "⋯" of
 * the island, then the right-click menu of a row within the exit of the first: the focus went back to the "⋯", whose
 * tooltip took the next Escape, so the right-click menu stayed open over the page (stack-island.spec.ts, 2026-10-30)
 */
export function focusBackFromMenu(popup: Element | null): boolean {
  const active = document.activeElement
  if (!(active instanceof HTMLElement) || active === document.body) {
    return true
  }
  const root = popup?.getAttribute("data-rootownerid")
  return root != null && active.closest("[data-rootownerid]")?.getAttribute("data-rootownerid") === root
}

import type { LucideIcon } from "lucide-react"
import { modifierKey } from "@/lib/platform"

/** Menu item; rendered by `MenuItems` in a context menu (`ContextMenuItems`) or a button's menu (`DropdownMenuItems`). */
export interface MenuItem {
  /**
   * Names the action across the surfaces that render it (a menu, the buttons of a popover or a footer), which pick
   * their items by it; the key of the item, else the label
   */
  id?: string
  label: string
  icon?: LucideIcon
  disabled?: boolean
  /** Keyboard shortcut hints, e.g. `["meta", "c"]` */
  kbds?: string[]
  color?: "default" | "destructive"
  /** Shown on hover beside the item: what it does, or why it is disabled */
  description?: string
  /** A checkbox item ("Auto-sync on Focus"): `onSelect` toggles it */
  checked?: boolean
  /** Not called for an item with `children` */
  onSelect: () => void
  /** Makes the item a submenu */
  children?: MenuItem[]
}

/** Groups are separated by a separator. */
export type MenuItemGroups = MenuItem[][]

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

import type { LucideIcon } from "lucide-react"

/** Context menu item; rendered by `ContextMenuItems`. */
export interface MenuItem {
  label: string
  icon?: LucideIcon
  disabled?: boolean
  /** Keyboard shortcut hints, e.g. `["meta", "c"]` */
  kbds?: string[]
  color?: "default" | "destructive"
  onSelect: () => void
}

/** Groups are separated by a separator. */
export type MenuItemGroups = MenuItem[][]

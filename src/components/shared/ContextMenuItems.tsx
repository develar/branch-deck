import {
  ContextMenuGroup,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuShortcut,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
} from "@/components/ui/context-menu"
import type { MenuComponents } from "@/components/shared/MenuItems"
import { MenuItems } from "@/components/shared/MenuItems"
import type { MenuItemGroups } from "@/lib/menu"

const components: MenuComponents = {
  Group: ContextMenuGroup,
  Item: ContextMenuItem,
  Separator: ContextMenuSeparator,
  Shortcut: ContextMenuShortcut,
  Sub: ContextMenuSub,
  SubTrigger: ContextMenuSubTrigger,
  SubContent: ContextMenuSubContent,
}

/** Renders groups of menu items (separated by separators) inside a `ContextMenuContent`. */
export function ContextMenuItems({ groups }: { groups: MenuItemGroups }) {
  return <MenuItems groups={groups} components={components} />
}

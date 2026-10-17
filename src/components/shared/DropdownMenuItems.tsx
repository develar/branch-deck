import {
  DropdownMenuCheckboxItem,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
} from "@/components/ui/dropdown-menu"
import type { MenuComponents } from "@/components/shared/MenuItems"
import { MenuItems } from "@/components/shared/MenuItems"
import type { MenuItemGroups } from "@/lib/menu"

const components: MenuComponents = {
  Group: DropdownMenuGroup,
  Item: DropdownMenuItem,
  Separator: DropdownMenuSeparator,
  Shortcut: DropdownMenuShortcut,
  Sub: DropdownMenuSub,
  SubTrigger: DropdownMenuSubTrigger,
  SubContent: DropdownMenuSubContent,
  CheckboxItem: DropdownMenuCheckboxItem,
}

/** Renders groups of menu items (separated by separators) inside a `DropdownMenuContent`, as `ContextMenuItems` does. */
export function DropdownMenuItems({ groups }: { groups: MenuItemGroups }) {
  return <MenuItems groups={groups} components={components} />
}

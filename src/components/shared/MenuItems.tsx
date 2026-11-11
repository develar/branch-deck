import { Fragment, useContext, useState } from "react"
import type { ComponentType, ReactNode } from "react"
import { cn } from "cn"
import { ChevronRightIcon } from "lucide-react"
import { Kbd, KbdGroup } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { MenuOpeningContext } from "@/hooks/useMenuOpenings"
import { FROM_MENU, frozenMenuGroups, hasMenuItems, kbdLabel, menuItemKey } from "@/lib/menu"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"

/**
 * The primitives a menu is built of: those of `context-menu.tsx` (the right-click menu) or of `dropdown-menu.tsx` (a
 * button's menu). Both are Base UI menus with the same parts.
 */
export interface MenuComponents {
  Group: ComponentType<{ children: ReactNode }>
  /** Without children when it is the `render` element of a tooltip trigger, which passes them */
  Item: ComponentType<{ disabled?: boolean; variant?: "default" | "destructive"; className?: string; onClick?: () => void; children?: ReactNode }>
  Separator: ComponentType
  Shortcut: ComponentType<{ children: ReactNode }>
  Sub: ComponentType<{ children: ReactNode }>
  SubTrigger: ComponentType<{ disabled?: boolean; className?: string; children: ReactNode }>
  SubContent: ComponentType<{ children: ReactNode }>
  /** A checkbox item (`MenuItem.checked`); a menu without one renders a plain item */
  CheckboxItem?: ComponentType<{ checked: boolean; disabled?: boolean; closeOnClick?: boolean; onCheckedChange: () => void; children?: ReactNode }>
}

/**
 * Renders groups of menu items, separated by separators, inside the content of a menu built of `components`. While the
 * menu shows, its list doesn't move (`frozenMenuGroups`): each item stays in its place as it changes, one that leaves
 * stays there disabled until the menu closes, and one that comes shows at the next open, so a fact that arrives under a
 * resting pointer (a listing of GitHub, a sync that finds a conflict) never moves the item a click is aimed at. A menu
 * opened again (`MenuOpeningContext`) or before its items came takes them anew. A group is known by its item that opens
 * a submenu, else by its first item (`groupKey`): an open submenu stays mounted under the pointer.
 */
export function MenuItems({ groups, components }: { groups: MenuItemGroups; components: MenuComponents }) {
  const { Group, Separator } = components
  const opening = useContext(MenuOpeningContext)
  const [opened, setOpened] = useState({ opening, groups })
  // the menu opened again (its popup kept as it faded out), or before its items came: the list starts from these
  if (opened.opening !== opening || (!hasMenuItems(opened.groups) && hasMenuItems(groups))) {
    setOpened({ opening, groups })
  }
  const nonEmpty = frozenMenuGroups(opened.groups, groups).filter(group => group.length > 0)
  return nonEmpty.map((group, groupIndex) => (
    <Fragment key={groupKey(group)}>
      {groupIndex > 0 && <Separator />}
      <Group>
        {group.map(item => (
          <MenuEntry key={itemKey(item)} item={item} components={components} />
        ))}
      </Group>
    </Fragment>
  ))
}

/** The key of a group of a menu: its item that opens a submenu (what may be open), else its first item */
function groupKey(group: readonly MenuItem[]): string {
  const item = group.find(entry => entry.children !== undefined) ?? group[0]
  return item === undefined ? "" : itemKey(item)
}

/** The key of an item among its siblings */
function itemKey(item: MenuItem): string {
  return menuItemKey(item)
}

function MenuEntry({ item, components }: { item: MenuItem; components: MenuComponents }) {
  const { Item, Shortcut, Sub, SubTrigger, SubContent, CheckboxItem } = components
  if (item.checked !== undefined && CheckboxItem !== undefined) {
    return (
      // closes as the other items and a native checkbox item do
      <CheckboxItem checked={item.checked} disabled={item.disabled} closeOnClick onCheckedChange={() => item.onSelect(FROM_MENU)}>
        <span className="truncate">{item.label}</span>
      </CheckboxItem>
    )
  }
  // a disabled submenu opens nothing: an item that says why on hover, as any disabled item
  if (item.children !== undefined && item.disabled !== true) {
    return (
      <Sub>
        {/* the generated context menu trigger has no gap between the icon and the label, unlike the items */}
        <SubTrigger disabled={item.disabled} className="gap-2">
          {item.icon && <item.icon />}
          <span className="truncate">{item.label}</span>
        </SubTrigger>
        <SubContent>
          {item.children.map(child => (
            <MenuEntry key={itemKey(child)} item={child} components={components} />
          ))}
        </SubContent>
      </Sub>
    )
  }

  // a running action has the spinner in place of its icon
  const content = (
    <>
      {item.running === true ? <Spinner /> : item.icon && <item.icon />}
      <span className="truncate">{item.label}</span>
      {item.children !== undefined && <ChevronRightIcon className="ml-auto" />}
      {item.kbds && item.kbds.length > 0 && (
        <Shortcut>
          <KbdGroup>
            {item.kbds.map(kbd => (
              <Kbd key={kbd}>{kbdLabel(kbd)}</Kbd>
            ))}
          </KbdGroup>
        </Shortcut>
      )}
    </>
  )
  if (item.description === undefined) {
    return (
      <Item disabled={item.disabled} variant={item.color ?? "default"} onClick={() => item.onSelect(FROM_MENU)}>
        {content}
      </Item>
    )
  }
  return (
    <Tooltip
      onOpenChange={(open, details) => {
        // Escape closes the menu too, not only the description
        if (!open && details.reason === "escape-key") {
          details.allowPropagation()
        }
      }}
    >
      <TooltipTrigger
        render={
          <Item
            disabled={item.disabled}
            variant={item.color ?? "default"}
            // a disabled item says why on hover: it keeps the pointer (Base UI ignores the click)
            className={cn(item.disabled && "data-disabled:pointer-events-auto")}
            onClick={() => item.onSelect(FROM_MENU)}
          />
        }
      >
        {content}
      </TooltipTrigger>
      <TooltipContent side="inline-end">{item.description}</TooltipContent>
    </Tooltip>
  )
}

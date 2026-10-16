import { Fragment } from "react"
import type { ComponentType, ReactNode } from "react"
import { cn } from "cn"
import { Kbd, KbdGroup } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"

/**
 * The primitives a menu is built of: those of `context-menu.tsx` (the right-click menu) or of `dropdown-menu.tsx` (a
 * button's menu). Both are Base UI menus with the same parts.
 */
export interface MenuComponents {
  Group: ComponentType<{ children: ReactNode }>
  /** Without children when it is the `render` element of a tooltip trigger, which passes them */
  Item: ComponentType<{ disabled?: boolean, variant?: "default" | "destructive", className?: string, onClick?: () => void, children?: ReactNode }>
  Separator: ComponentType
  Shortcut: ComponentType<{ children: ReactNode }>
  Sub: ComponentType<{ children: ReactNode }>
  SubTrigger: ComponentType<{ disabled?: boolean, className?: string, children: ReactNode }>
  SubContent: ComponentType<{ children: ReactNode }>
}

const kbdSymbols: Record<string, string> = {
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

/** Renders groups of menu items, separated by separators, inside the content of a menu built of `components`. */
export function MenuItems({ groups, components }: { groups: MenuItemGroups, components: MenuComponents }) {
  const { Group, Separator } = components
  const nonEmpty = groups.filter(group => group.length > 0)
  return nonEmpty.map((group, groupIndex) => (
    <Fragment key={groupIndex}>
      {groupIndex > 0 && <Separator />}
      <Group>
        {group.map(item => <MenuEntry key={item.label} item={item} components={components} />)}
      </Group>
    </Fragment>
  ))
}

function MenuEntry({ item, components }: { item: MenuItem, components: MenuComponents }) {
  const { Item, Shortcut, Sub, SubTrigger, SubContent } = components
  if (item.children !== undefined) {
    return (
      <Sub>
        {/* the generated context menu trigger has no gap between the icon and the label, unlike the items */}
        <SubTrigger disabled={item.disabled} className="gap-2">
          {item.icon && <item.icon />}
          <span className="truncate">{item.label}</span>
        </SubTrigger>
        <SubContent>
          {item.children.map(child => <MenuEntry key={child.label} item={child} components={components} />)}
        </SubContent>
      </Sub>
    )
  }

  const content = (
    <>
      {item.icon && <item.icon />}
      <span className="truncate">{item.label}</span>
      {item.kbds && item.kbds.length > 0 && (
        <Shortcut>
          <KbdGroup>
            {item.kbds.map(kbd => <Kbd key={kbd}>{kbdSymbols[kbd] ?? kbd.toUpperCase()}</Kbd>)}
          </KbdGroup>
        </Shortcut>
      )}
    </>
  )
  if (item.description === undefined) {
    return (
      <Item disabled={item.disabled} variant={item.color ?? "default"} onClick={item.onSelect}>
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
        render={(
          <Item
            disabled={item.disabled}
            variant={item.color ?? "default"}
            // a disabled item says why on hover: it keeps the pointer (Base UI ignores the click)
            className={cn(item.disabled && "data-disabled:pointer-events-auto")}
            onClick={item.onSelect}
          />
        )}
      >
        {content}
      </TooltipTrigger>
      <TooltipContent side="inline-end">{item.description}</TooltipContent>
    </Tooltip>
  )
}

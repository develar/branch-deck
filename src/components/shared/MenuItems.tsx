import { Fragment } from "react"
import type { ComponentType, ReactNode } from "react"
import { cn } from "cn"
import { ChevronRightIcon } from "lucide-react"
import { Kbd, KbdGroup } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { FROM_MENU, kbdLabel } from "@/lib/menu"
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

/** Renders groups of menu items, separated by separators, inside the content of a menu built of `components`. */
export function MenuItems({ groups, components }: { groups: MenuItemGroups; components: MenuComponents }) {
  const { Group, Separator } = components
  const nonEmpty = groups.filter(group => group.length > 0)
  return nonEmpty.map((group, groupIndex) => (
    <Fragment key={groupIndex}>
      {groupIndex > 0 && <Separator />}
      <Group>
        {group.map(item => (
          <MenuEntry key={item.id ?? item.label} item={item} components={components} />
        ))}
      </Group>
    </Fragment>
  ))
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
            <MenuEntry key={child.id ?? child.label} item={child} components={components} />
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

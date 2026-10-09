import { Fragment } from "react"
import { ContextMenuGroup, ContextMenuItem, ContextMenuSeparator, ContextMenuShortcut } from "@/components/ui/context-menu"
import { Kbd, KbdGroup } from "@/components/ui/kbd"
import type { MenuItemGroups } from "@/lib/menu"

const kbdSymbols: Record<string, string> = {
  meta: "⌘",
  ctrl: "⌃",
  alt: "⌥",
  shift: "⇧",
  enter: "↵",
  escape: "Esc",
  delete: "⌫",
}

/** Renders groups of menu items (separated by separators) inside a `ContextMenuContent`. */
export function ContextMenuItems({ groups }: { groups: MenuItemGroups }) {
  const nonEmpty = groups.filter(group => group.length > 0)
  return nonEmpty.map((group, groupIndex) => (
    <Fragment key={groupIndex}>
      {groupIndex > 0 && <ContextMenuSeparator />}
      <ContextMenuGroup>
        {group.map(item => (
          <ContextMenuItem
            key={item.label}
            disabled={item.disabled}
            variant={item.color ?? "default"}
            onClick={item.onSelect}
          >
            {item.icon && <item.icon />}
            <span className="truncate">{item.label}</span>
            {item.kbds && item.kbds.length > 0 && (
              <ContextMenuShortcut>
                <KbdGroup>
                  {item.kbds.map(kbd => <Kbd key={kbd}>{kbdSymbols[kbd] ?? kbd.toUpperCase()}</Kbd>)}
                </KbdGroup>
              </ContextMenuShortcut>
            )}
          </ContextMenuItem>
        ))}
      </ContextMenuGroup>
    </Fragment>
  ))
}

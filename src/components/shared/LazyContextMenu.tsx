import { useState } from "react"
import type { ReactElement, ReactNode } from "react"
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "@/components/ui/context-menu"
import { ContextMenuItems } from "@/components/shared/ContextMenuItems"
import type { MenuItemGroups } from "@/lib/menu"

/**
 * A right-click menu built when it opens (the menu of a commit in the column of a range review and in the tree of its
 * story); a menu without items does not open
 */
export function LazyContextMenu({ menu, render, onOpen, children }: { menu: () => MenuItemGroups; render: ReactElement; onOpen?: () => void; children?: ReactNode }) {
  const [groups, setGroups] = useState<MenuItemGroups>([])
  return (
    <ContextMenu
      onOpenChange={(open, eventDetails) => {
        if (open) {
          const next = menu()
          if (next.every(group => group.length === 0)) {
            eventDetails.cancel()
            return
          }
          onOpen?.()
          setGroups(next)
        }
      }}
    >
      <ContextMenuTrigger render={render}>{children}</ContextMenuTrigger>
      <ContextMenuContent finalFocus={false}>
        <ContextMenuItems groups={groups} />
      </ContextMenuContent>
    </ContextMenu>
  )
}

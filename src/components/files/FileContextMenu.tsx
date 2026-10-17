import { useState } from "react"
import type { ReactElement, ReactNode } from "react"
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "@/components/ui/context-menu"
import { ContextMenuItems } from "@/components/shared/ContextMenuItems"
import { fileMenuItems } from "@/lib/file-actions"
import type { FileActionTarget } from "@/lib/file-actions"
import type { MenuItem } from "@/lib/menu"

export interface FileContextMenuProps {
  /** The file or directory of the row or header, read when the menu opens */
  target: () => FileActionTarget
  /** ⌘C works where the menu is (a focused row): the menu shows the key */
  copyKey?: boolean
  /** The row or the header, never a block with a diff in it: the trigger takes every right click inside it */
  render: ReactElement
  /** The text of a header stays selectable; a row's is not */
  selectable?: boolean
  /** The menu opens: a row takes the focus, so ⌘C copies the same path */
  onOpen?: () => void
  children: ReactNode
}

/**
 * The right-click menu of a changed file or directory (`fileMenuItems`): the same actions wherever a file shows. One menu
 * per row or header, as the rows of a commit list have: a trigger around a diff would take the native menu of its text.
 */
export function FileContextMenu({ target, copyKey = false, render, selectable = false, onOpen, children }: FileContextMenuProps) {
  // built when the menu opens: a directory lists its files then
  const [items, setItems] = useState<MenuItem[]>([])
  return (
    <ContextMenu
      onOpenChange={open => {
        if (open) {
          onOpen?.()
          setItems(fileMenuItems(target(), { copyKey }))
        }
      }}
    >
      <ContextMenuTrigger render={render} className={selectable ? "select-text" : undefined}>
        {children}
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItems groups={[items]} />
      </ContextMenuContent>
    </ContextMenu>
  )
}

import { ArrowUpDownIcon, CodeIcon, ListTreeIcon } from "lucide-react"
import type { MenuItem } from "@/lib/menu"
import { describeMoveEnd, MOVE_END_LABEL } from "@/presenters/diff-marks"

/*
 * The menu of the code of a diff (openspec/specs/commit/symbol-navigation/spec.md): one builder, which the context menu of the
 * code renders, opened by the pointer on the name under it or by a menu key on the name at the selection; the keys of
 * its items work in the view whatever the menu (`DiffView`). On a line of a move, or the note at its end, it has the
 * item that shows the other end of the move (openspec/specs/commit/quiet-changes/spec.md, "Views").
 */

export interface CodeMenuContext {
  /** The name the menu opened on: under the pointer, or at the selection for a menu key; null where no name is */
  name: string | null
  goToDeclaration: () => void
  /** Null where the view has no Find Usages */
  findUsages: (() => void) | null
  /**
   * The end of a move the menu opened on, a line of a move or the note at its end: which end it is, what shows the
   * other one, and the key of the view that does the same, given only where the key takes the note of this end (null
   * where the view has no such key, or the key takes another note). Null on a row of no move: the menu has no such
   * item
   */
  moveEnd: { readonly at: "source" | "destination"; readonly go: () => void; readonly key: string | null } | null
}

export function codeMenuItems({ name, goToDeclaration, findUsages, moveEnd }: CodeMenuContext): MenuItem[] {
  const items: MenuItem[] = [
    {
      id: "go-to-declaration",
      label: "Go to Declaration",
      icon: CodeIcon,
      kbds: ["mod", "b"],
      disabled: name === null,
      description: name === null ? "No name here" : `Go to the declaration of ${name}`,
      onSelect: () => goToDeclaration(),
    },
  ]
  if (findUsages !== null) {
    items.push({
      id: "find-usages",
      label: "Find Usages",
      icon: ListTreeIcon,
      kbds: ["alt", "f7"],
      disabled: name === null,
      description: name === null ? "No name here" : `Find the usages of ${name}`,
      onSelect: () => findUsages(),
    })
  }
  if (moveEnd !== null) {
    items.push({
      id: "go-to-move-end",
      label: MOVE_END_LABEL,
      icon: ArrowUpDownIcon,
      ...(moveEnd.key !== null && { kbds: [moveEnd.key.toLowerCase()] }),
      description: describeMoveEnd(moveEnd.at),
      onSelect: () => moveEnd.go(),
    })
  }
  return items
}

/** How far inside the view the menu of a key opens where the selection gives no place */
const KEY_MENU_INSET = 24

/**
 * Where the menu of the code opens for a menu key: under the start of `place` (the name at the selection, else the
 * selection of the view), never outside `view`, whose scroll may have taken the place out of it; at the start of the
 * view where there is no place. Viewport coordinates.
 */
export function codeMenuKeyPoint(
  place: { readonly left: number; readonly bottom: number } | undefined,
  view: { readonly left: number; readonly top: number; readonly right: number; readonly bottom: number },
): { x: number; y: number } {
  if (place === undefined) {
    return { x: Math.min(view.left + KEY_MENU_INSET, view.right), y: Math.min(view.top + KEY_MENU_INSET, view.bottom) }
  }
  return { x: Math.min(Math.max(place.left, view.left), view.right), y: Math.min(Math.max(place.bottom, view.top), view.bottom) }
}

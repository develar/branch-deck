import { CodeIcon, ListTreeIcon } from "lucide-react"
import type { MenuItem } from "@/lib/menu"

/*
 * The menu of the code of a diff (openspec/specs/commit/symbol-navigation/spec.md): one builder, which the context menu of the
 * code renders, opened by the pointer on the name under it or by a menu key on the name at the selection; the keys of
 * its items work in the view whatever the menu (`DiffView`).
 */

export interface CodeMenuContext {
  /** The name the menu opened on: under the pointer, or at the selection for a menu key; null where no name is */
  name: string | null
  goToDeclaration: () => void
  /** Null where the view has no Find Usages */
  findUsages: (() => void) | null
}

export function codeMenuItems({ name, goToDeclaration, findUsages }: CodeMenuContext): MenuItem[] {
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

import { CodeIcon, ListTreeIcon } from "lucide-react"
import type { MenuItem } from "@/lib/menu"

/*
 * The menu of the code of a diff (spec/commit/symbol-navigation.spec.md): one builder, which the context menu of the
 * code renders; the keys work in the view whatever the menu (`DiffView`).
 */

export interface CodeMenuContext {
  /** The name under the pointer or at the selection; null where no name is */
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

import { CodeIcon } from "lucide-react"
import type { MenuItem } from "@/lib/menu"

/*
 * The menu of the code of a diff (spec/commit/symbol-navigation.spec.md): one builder, which the context menu of the
 * code renders; the keys work in the view whatever the menu (`DiffView`).
 */

export interface CodeMenuContext {
  /** The name under the pointer or at the selection; null where no name is */
  name: string | null
  goToDeclaration: () => void
}

export function codeMenuItems({ name, goToDeclaration }: CodeMenuContext): MenuItem[] {
  return [
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
}

import type { KeyboardEvent as ReactKeyboardEvent } from "react"
import { CopyIcon } from "lucide-react"
import { copyText } from "@/lib/clipboard"
import type { MenuItem } from "@/lib/menu"
import { isMac } from "@/lib/platform"

/** A changed file, or a directory of changed files, in a list or a tree of files */
export type FileActionTarget =
  | { readonly kind: "file"; readonly path: string }
  /** `files`: the paths of the changed files under it */
  | { readonly kind: "directory"; readonly path: string; readonly files: readonly string[] }

/**
 * The actions on a changed file or directory, for every surface that shows one (the trees of files, the headers of
 * diffs, the files of a pull request): Copy Path, and for a directory Copy Changed Paths, one per line, the name of the
 * action on a commit. `copyKey`: ⌘C works there (a focused row), so Copy Path shows it.
 */
export function fileMenuItems(target: FileActionTarget, { copyKey }: { copyKey: boolean }): MenuItem[] {
  const items: MenuItem[] = [{ id: "copy-path", label: "Copy Path", icon: CopyIcon, ...(copyKey && { kbds: ["mod", "c"] }), onSelect: () => void copyText(target.path) }]
  if (target.kind === "directory") {
    items.push({ id: "copy-changed-paths", label: "Copy Changed Paths", icon: CopyIcon, onSelect: () => void copyText(target.files.join("\n")) })
  }
  return items
}

/**
 * ⌘C (Ctrl+C elsewhere) on a focused row copies its path, as Copy Path does; not while text is selected, which the
 * native copy takes. The key of the layout, as the system's copy: not the physical key of the review's letters.
 */
export function isCopyPathKey(event: KeyboardEvent | ReactKeyboardEvent): boolean {
  const selection = window.getSelection()
  return (
    event.key.toLowerCase() === "c" &&
    (isMac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey) &&
    !event.altKey &&
    !event.shiftKey &&
    (selection === null || selection.isCollapsed)
  )
}

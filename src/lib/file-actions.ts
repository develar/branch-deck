import { CopyIcon } from "lucide-react"
import { copyText } from "@/lib/clipboard"
import type { MenuItem } from "@/lib/menu"

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

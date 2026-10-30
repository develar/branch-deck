import { CopyIcon } from "lucide-react"
import { copyText } from "@/lib/clipboard"
import type { MenuItem } from "@/lib/menu"

/** A changed file, or a directory of changed files, in a list or a tree of files */
export type FileActionTarget =
  | { readonly kind: "file"; readonly path: string }
  /** `files`: the paths of the changed files under it */
  | { readonly kind: "directory"; readonly path: string; readonly files: readonly string[] }
  /** A group of files that is no directory (the Mechanical group of a review): it has no path of its own */
  | { readonly kind: "group"; readonly files: readonly string[] }

/** What ⌘C on a row copies: its path, the changed paths of a group, one per line */
export function copiedText(target: FileActionTarget): string {
  return target.kind === "group" ? target.files.join("\n") : target.path
}

/**
 * The actions on a changed file or directory, for every surface that shows one (the trees of files, the headers of
 * diffs, the files of a pull request): Copy Path, and for a directory Copy Changed Paths, one per line, the name of the
 * action on a commit; a group has only Copy Changed Paths. `copyKey`: ⌘C works there (a focused row), so the item it
 * runs shows it.
 */
export function fileMenuItems(target: FileActionTarget, { copyKey }: { copyKey: boolean }): MenuItem[] {
  if (target.kind === "group") {
    return [{ id: "copy-changed-paths", label: "Copy Changed Paths", icon: CopyIcon, ...(copyKey && { kbds: ["mod", "c"] }), onSelect: () => void copyText(copiedText(target)) }]
  }
  const items: MenuItem[] = [{ id: "copy-path", label: "Copy Path", icon: CopyIcon, ...(copyKey && { kbds: ["mod", "c"] }), onSelect: () => void copyText(target.path) }]
  if (target.kind === "directory") {
    items.push({ id: "copy-changed-paths", label: "Copy Changed Paths", icon: CopyIcon, onSelect: () => void copyText(target.files.join("\n")) })
  }
  return items
}

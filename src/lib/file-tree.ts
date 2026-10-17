import type { CommitFile } from "@/lib/bindings"
import type { FileActionTarget } from "@/lib/file-actions"

/** A changed file of the tree, by its name in its directory */
export interface FileTreeFile {
  kind: "file"
  /** The path after the commit: unique in a commit */
  key: string
  name: string
  depth: number
  parentKey: string | undefined
  file: CommitFile
}

/** A directory of the tree; a chain of directories with one subdirectory each is one row, `plugins/air/backend` */
export interface FileTreeDirectory {
  kind: "directory"
  /** The full path with a trailing `/`, which no file path has */
  key: string
  name: string
  depth: number
  parentKey: string | undefined
  children: FileTreeNode[]
  /** Over the whole subtree */
  fileCount: number
  additions: number
  deletions: number
}

export type FileTreeNode = FileTreeFile | FileTreeDirectory

/** Above this many files, the directories below the top level start collapsed */
export const AUTO_COLLAPSE_FILE_COUNT = 100

const collator = new Intl.Collator("en", { numeric: true, sensitivity: "base" })

/** Directories before files, then by name as a person reads it: `file2` before `file10`, case-insensitive */
export function compareTreeNodes(a: FileTreeNode, b: FileTreeNode): number {
  if (a.kind !== b.kind) {
    return a.kind === "directory" ? -1 : 1
  }
  return collator.compare(a.name, b.name) || (a.name < b.name ? -1 : a.name > b.name ? 1 : 0)
}

interface MutableDirectory {
  directories: Map<string, MutableDirectory>
  files: CommitFile[]
}

/**
 * The files grouped by directory, as IntelliJ and Space show the changes of a commit: a directory with a single
 * subdirectory and no files is merged with it, so a deep path costs one row, and each directory sums its line counts.
 */
export function buildFileTree(files: readonly CommitFile[]): FileTreeNode[] {
  const root: MutableDirectory = { directories: new Map(), files: [] }
  for (const file of files) {
    const segments = file.path.split("/")
    let directory = root
    for (const segment of segments.slice(0, -1)) {
      let child = directory.directories.get(segment)
      if (child === undefined) {
        child = { directories: new Map(), files: [] }
        directory.directories.set(segment, child)
      }
      directory = child
    }
    directory.files.push(file)
  }
  return toNodes(root, "", 0, undefined)
}

function toNodes(directory: MutableDirectory, prefix: string, depth: number, parentKey: string | undefined): FileTreeNode[] {
  const nodes: FileTreeNode[] = []
  for (const [segment, child] of directory.directories) {
    // merge the chain of directories that hold nothing but one directory
    let name = segment
    let current = child
    while (current.files.length === 0 && current.directories.size === 1) {
      const [nextSegment, next] = current.directories.entries().next().value!
      name = `${name}/${nextSegment}`
      current = next
    }
    const key = `${prefix}${name}/`
    const children = toNodes(current, key, depth + 1, key)
    let fileCount = 0
    let additions = 0
    let deletions = 0
    for (const node of children) {
      if (node.kind === "file") {
        fileCount++
        additions += node.file.additions
        deletions += node.file.deletions
      } else {
        fileCount += node.fileCount
        additions += node.additions
        deletions += node.deletions
      }
    }
    nodes.push({ kind: "directory", key, name, depth, parentKey, children, fileCount, additions, deletions })
  }
  for (const file of directory.files) {
    nodes.push({ kind: "file", key: file.path, name: file.path.slice(prefix.length), depth, parentKey, file })
  }
  return nodes.sort(compareTreeNodes)
}

/** The files in the order of the tree: what the tree shows from top to bottom with every directory open */
/** What the actions of a row act on (`fileMenuItems`): a file by its path, a directory by its path and its files */
export function fileActionTarget(node: FileTreeNode): FileActionTarget {
  return node.kind === "file"
    ? { kind: "file", path: node.file.path }
    : { kind: "directory", path: node.key.slice(0, -1), files: flattenTreeFiles(node.children).map(file => file.path) }
}

export function flattenTreeFiles(nodes: readonly FileTreeNode[]): CommitFile[] {
  return nodes.flatMap(node => (node.kind === "file" ? [node.file] : flattenTreeFiles(node.children)))
}

/** The rows of the tree from top to bottom; the children of a collapsed directory are hidden */
export function getVisibleRows(nodes: readonly FileTreeNode[], collapsed: ReadonlySet<string>): FileTreeNode[] {
  const rows: FileTreeNode[] = []
  const visit = (list: readonly FileTreeNode[]) => {
    for (const node of list) {
      rows.push(node)
      if (node.kind === "directory" && !collapsed.has(node.key)) {
        visit(node.children)
      }
    }
  }
  visit(nodes)
  return rows
}

/** The directories that start collapsed: none, unless the commit has more than `AUTO_COLLAPSE_FILE_COUNT` files */
export function getInitiallyCollapsed(nodes: readonly FileTreeNode[], fileCount: number): Set<string> {
  const collapsed = new Set<string>()
  if (fileCount <= AUTO_COLLAPSE_FILE_COUNT) {
    return collapsed
  }
  const visit = (list: readonly FileTreeNode[]) => {
    for (const node of list) {
      if (node.kind === "directory") {
        if (node.depth > 0) {
          collapsed.add(node.key)
        }
        visit(node.children)
      }
    }
  }
  visit(nodes)
  return collapsed
}

/** `set` with `key` added, or removed if it was there */
export function toggleInSet(set: ReadonlySet<string>, key: string): ReadonlySet<string> {
  const next = new Set(set)
  if (!next.delete(key)) {
    next.add(key)
  }
  return next
}

/** `collapsed` without the directories above `key`, so its row shows; the same set when none of them is collapsed */
export function revealInTree(collapsed: ReadonlySet<string>, key: string): ReadonlySet<string> {
  // a directory key is its full path with a trailing slash, so the directories above a row are the keys it starts with
  const above = [...collapsed].filter(directory => key !== directory && key.startsWith(directory))
  if (above.length === 0) {
    return collapsed
  }
  const next = new Set(collapsed)
  for (const directory of above) {
    next.delete(directory)
  }
  return next
}

/**
 * The name of a file row: its name, and for a rename or a copy where it came from, relative to the directory both
 * paths share (`Old.kt` in the same directory, `api/Old.kt` from a sibling directory)
 */
export function formatTreeFileLabel(file: CommitFile): { name: string; from: string | null } {
  const name = file.path.slice(file.path.lastIndexOf("/") + 1)
  if (file.oldPath === null) {
    return { name, from: null }
  }
  const newSegments = file.path.split("/")
  const oldSegments = file.oldPath.split("/")
  let common = 0
  while (common < Math.min(newSegments.length, oldSegments.length) - 1 && newSegments[common] === oldSegments[common]) {
    common++
  }
  return { name, from: oldSegments.slice(common).join("/") }
}

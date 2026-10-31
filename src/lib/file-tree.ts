import type { CommitFile, DependencyChange, MechanicalKind, ReviewPlan } from "@/lib/bindings"
import type { FileActionTarget } from "@/lib/file-actions"
import { formatMechanicalGroup } from "@/presenters/review-plan"

/** A changed file of the tree, by its name in its directory */
export interface FileTreeFile {
  kind: "file"
  /** The path after the commit: unique in a commit */
  key: string
  name: string
  depth: number
  parentKey: string | undefined
  file: CommitFile
  /** Named by its path, not in the directories of its path: a file of a group */
  inGroup?: boolean
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
  /**
   * A group of files that is no directory: `mechanical`, the Mechanical group at the end of the tree of a review, or
   * the group of one kind in it
   */
  group?: MechanicalKind | "mechanical"
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

/** The key of the Mechanical group: a git path never starts with `/` */
export const MECHANICAL_GROUP_KEY = "//mechanical/"

/**
 * The kind Rust gave each mechanical file of a plan (`FileFacts.mechanical`), the kinds in the order of their units (the
 * order of the story, `MECHANICAL_ORDER` of `units.rs`), which the Mechanical group keeps
 */
export function mechanicalKindsOf(plan: Pick<ReviewPlan, "files" | "units">): Map<string, MechanicalKind> {
  const order = plan.units.flatMap(unit => (unit.kind.kind === "mechanical" ? [unit.kind.mechanical] : []))
  const rank = (kind: MechanicalKind) => (order.includes(kind) ? order.indexOf(kind) : order.length)
  const files = plan.files.flatMap(file => (file.mechanical === null ? [] : [[file.path, file.mechanical] as const]))
  return new Map(files.toSorted((a, b) => rank(a[1]) - rank(b[1])))
}

/**
 * The tree of a review with its plan: the files of real work as `buildFileTree` groups them, then the Mechanical group
 * with a group per kind (`mechanical`, the kind Rust gave each path, the kinds in its order: `mechanicalKindsOf`), each
 * file named by its path. The diffs and J and K follow this order, so the mechanical files come last
 */
export function buildReviewTree(files: readonly CommitFile[], mechanical: ReadonlyMap<string, MechanicalKind>): FileTreeNode[] {
  const kindOf = (file: CommitFile) => mechanical.get(file.path)
  const nodes = buildFileTree(files.filter(file => kindOf(file) === undefined))
  const folded = flattenTreeFiles(buildFileTree(files.filter(file => kindOf(file) !== undefined)))
  if (folded.length === 0) {
    return nodes
  }
  const groups: FileTreeDirectory[] = []
  for (const kind of new Set(mechanical.values())) {
    const ofKind = folded.filter(file => kindOf(file) === kind)
    if (ofKind.length > 0) {
      const key = `${MECHANICAL_GROUP_KEY}${kind}/`
      const children: FileTreeFile[] = ofKind.map(file => ({ kind: "file", key: file.path, name: file.path, depth: 2, parentKey: key, file, inGroup: true }))
      groups.push(groupNode(key, formatMechanicalGroup(kind, ofKind.length), 1, MECHANICAL_GROUP_KEY, children, kind))
    }
  }
  return [...nodes, groupNode(MECHANICAL_GROUP_KEY, formatMechanicalGroup(null, folded.length), 0, undefined, groups, "mechanical")]
}

function groupNode(key: string, name: string, depth: number, parentKey: string | undefined, children: FileTreeNode[], group: FileTreeDirectory["group"]): FileTreeDirectory {
  const files = flattenTreeFiles(children)
  const additions = files.reduce((sum, file) => sum + file.additions, 0)
  const deletions = files.reduce((sum, file) => sum + file.deletions, 0)
  return { kind: "directory", key, name, depth, parentKey, children, fileCount: files.length, additions, deletions, group }
}

/**
 * The row that shows the dependency changes of each build file, by its key: the directory row whose chain of
 * directories holds the directory of the build file (its module), else the row of the build file itself (a build file
 * at the root, a module whose only change is its build file, folded into the Mechanical group). A build file the tree
 * does not show has no row
 */
export function dependencyRows(nodes: readonly FileTreeNode[], dependencies: readonly DependencyChange[]): Map<string, DependencyChange[]> {
  const rows = new Map<string, DependencyChange[]>()
  const directories: FileTreeDirectory[] = []
  const files = new Set<string>()
  const visit = (list: readonly FileTreeNode[]) => {
    for (const node of list) {
      if (node.kind === "file") {
        files.add(node.key)
      } else {
        if (node.group === undefined) {
          directories.push(node)
        }
        visit(node.children)
      }
    }
  }
  visit(nodes)
  for (const change of dependencies) {
    const directory = change.manifest.slice(0, change.manifest.lastIndexOf("/") + 1)
    // the row whose merged chain holds the directory: its key starts with it, its parent's key is shorter
    const row =
      (directory === "" ? undefined : directories.find(node => node.key.startsWith(directory) && (node.parentKey ?? "").length < directory.length)?.key) ??
      (files.has(change.manifest) ? change.manifest : undefined)
    if (row !== undefined) {
      rows.set(row, [...(rows.get(row) ?? []), change])
    }
  }
  return rows
}

/**
 * What the actions of a row act on (`fileMenuItems`): a file by its path, a directory by its path and its files, a group
 * by its files
 */
export function fileActionTarget(node: FileTreeNode): FileActionTarget {
  if (node.kind === "file") {
    return { kind: "file", path: node.file.path }
  }
  const files = flattenTreeFiles(node.children).map(file => file.path)
  return node.group === undefined ? { kind: "directory", path: node.key.slice(0, -1), files } : { kind: "group", files }
}

/** The files in the order of the tree: what the tree shows from top to bottom with every directory open */
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

/**
 * The directories that start collapsed: the Mechanical group, and the directories below the top level when the commit
 * has more than `AUTO_COLLAPSE_FILE_COUNT` files
 */
export function getInitiallyCollapsed(nodes: readonly FileTreeNode[], fileCount: number): Set<string> {
  const collapsed = new Set<string>(nodes.some(node => node.key === MECHANICAL_GROUP_KEY) ? [MECHANICAL_GROUP_KEY] : [])
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

/** The keys of the directories and groups above the row `key` in `nodes`, outermost first; null when it has no row */
function keysAbove(nodes: readonly FileTreeNode[], key: string): string[] | null {
  for (const node of nodes) {
    if (node.key === key) {
      return []
    }
    if (node.kind === "directory") {
      const inner = keysAbove(node.children, key)
      if (inner !== null) {
        return [node.key, ...inner]
      }
    }
  }
  return null
}

/**
 * `collapsed` without the directories and groups above `key` in `nodes`, so its row shows; the same set when none of
 * them is collapsed
 */
export function revealInTree(collapsed: ReadonlySet<string>, key: string, nodes: readonly FileTreeNode[]): ReadonlySet<string> {
  const above = (keysAbove(nodes, key) ?? []).filter(directory => collapsed.has(directory))
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
export function formatTreeFileLabel(file: Pick<CommitFile, "path" | "oldPath">): { name: string; from: string | null } {
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

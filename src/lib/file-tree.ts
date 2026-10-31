import type { CommitFile, Concern, DependencyChange, MechanicalKind, ReviewPlan } from "@/lib/bindings"
import type { FileActionTarget } from "@/lib/file-actions"
import { formatAlsoIn, formatConcernEvidence, formatConcernName, formatHome, OTHER_CHANGES } from "@/presenters/review-concerns"
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

/**
 * A file in a concern that is not the one of its main row (`Concern.elsewhere`): it selects the file and shows its
 * part of this concern; no file of the tree, so it counts in no group and no position
 */
export interface FileTreeAlso {
  kind: "also"
  /** `alsoRowKey` of its concern and its file: no path and no directory */
  key: string
  /** Its path in a concern, its name in the directory of Other changes */
  name: string
  depth: number
  parentKey: string | undefined
  file: CommitFile
  /** `Concern.id` of the concern it shows the file in */
  concern: string
  /** "also in proxy", after its name */
  also: string
  /** In its hover, where its main row is: "Its own row is under grep → accio" */
  home: string
  inGroup: boolean
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
   * the group of one kind in it or in a concern; `concern`, a task of the range; `other`, what no evidence joined
   */
  group?: MechanicalKind | "mechanical" | "concern" | "other"
  /** Its hover: why the files of a concern are together */
  title?: string
}

export type FileTreeNode = FileTreeFile | FileTreeAlso | FileTreeDirectory

/** Above this many files, the directories below the top level start collapsed */
export const AUTO_COLLAPSE_FILE_COUNT = 100

const collator = new Intl.Collator("en", { numeric: true, sensitivity: "base" })

/** Directories before files, then by name as a person reads it: `file2` before `file10`, case-insensitive */
export function compareTreeNodes(a: FileTreeNode, b: FileTreeNode): number {
  if ((a.kind === "directory") !== (b.kind === "directory")) {
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
export function buildFileTree(files: readonly CommitFile[], parent?: { key: string; depth: number }): FileTreeNode[] {
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
  return toNodes(root, "", parent?.depth ?? 0, parent?.key)
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
      } else if (node.kind === "directory") {
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

/** The key of the group of what no evidence joined, before the Mechanical group */
export const OTHER_CHANGES_KEY = "//other/"

const CONCERN_KEY = "//concern/"

/** The key of the row of a concern */
export function concernRowKey(concern: string): string {
  return `${CONCERN_KEY}${concern}/`
}

/** The key of the row of `path` in the concern `concern` that is not its main row (`FileTreeAlso`) */
export function alsoRowKey(concern: string, path: string): string {
  return `${CONCERN_KEY}${concern}/also/${path}`
}

/**
 * The tree of a review with its plan (spec/branch/review-concerns.spec.md, "In the Review Window"): the concerns Rust
 * found (`concerns`), each a group of the files whose main row it holds, named by their path, a row "also in" for each
 * file of it whose main row is another concern's (`Concern.elsewhere`), then the mechanical ones of it (the generated
 * files that follow their source) in a group per kind; then what no evidence joined and the files of no concern as
 * `buildFileTree` groups them, under Other changes; then the Mechanical group with a group per kind (`mechanical`, the
 * kind Rust gave each path, the kinds in its order: `mechanicalKindsOf`), each file named by its path. Without a concern
 * but the rest, the files of real work as `buildFileTree` groups them, then the Mechanical group. The diffs and J and K
 * follow this order, so the mechanical files come last
 */
export function buildReviewTree(files: readonly CommitFile[], mechanical: ReadonlyMap<string, MechanicalKind>, concerns: readonly Concern[]): FileTreeNode[] {
  const tasks = concerns.filter(concern => concern.name.source !== "rest")
  if (tasks.length === 0) {
    return withMechanicalGroup(buildFileTree(files.filter(file => !mechanical.has(file.path))), files, mechanical)
  }
  const byPath = new Map(files.map(file => [file.path, file]))
  // the concern of the main row of each path of a concern
  const homes = new Map<string, Concern>()
  for (const concern of concerns) {
    const elsewhere = new Set(concern.elsewhere)
    for (const path of concern.paths.filter(it => !elsewhere.has(it))) {
      homes.set(path, concern)
    }
  }
  const alsoNode = (concern: Concern, file: CommitFile, row: Pick<FileTreeAlso, "name" | "depth" | "parentKey" | "inGroup">): FileTreeAlso => {
    const home = homes.get(file.path)
    return {
      kind: "also",
      key: alsoRowKey(concern.id, file.path),
      ...row,
      file,
      concern: concern.id,
      also: formatAlsoIn(concern.name),
      home: home === undefined ? "" : formatHome(home.name),
    }
  }
  /** `node` with the files of `elsewhere` in it as rows "also in" the concern `rest` */
  const asAlso = (node: FileTreeNode, rest: Concern, elsewhere: ReadonlySet<string>): FileTreeNode => {
    if (node.kind === "directory") {
      return { ...node, children: node.children.map(child => asAlso(child, rest, elsewhere)) }
    }
    return node.kind === "file" && elsewhere.has(node.file.path)
      ? alsoNode(rest, node.file, { name: node.name, depth: node.depth, parentKey: node.parentKey, inGroup: false })
      : node
  }
  // the paths whose main row a concern holds
  const held = new Set<string>()
  const nodes: FileTreeNode[] = []
  for (const concern of tasks) {
    const key = concernRowKey(concern.id)
    const elsewhere = new Set(concern.elsewhere)
    const children: FileTreeNode[] = []
    const folded: CommitFile[] = []
    for (const path of concern.paths) {
      const file = byPath.get(path)
      if (file === undefined) {
        continue
      }
      if (elsewhere.has(path)) {
        children.push(alsoNode(concern, file, { name: path, depth: 1, parentKey: key, inGroup: true }))
      } else if (mechanical.has(path)) {
        held.add(path)
        folded.push(file)
      } else {
        held.add(path)
        children.push({ kind: "file", key: path, name: path, depth: 1, parentKey: key, file, inGroup: true })
      }
    }
    children.push(...kindGroups(folded, mechanical, key, 1))
    if (children.length > 0) {
      nodes.push(groupNode(key, formatConcernName(concern.name), 0, undefined, children, "concern", formatConcernEvidence(concern.evidence) ?? undefined))
    }
  }
  const rest = concerns.find(concern => concern.name.source === "rest")
  const restElsewhere = new Set(rest?.elsewhere ?? [])
  const other = files.filter(file => restElsewhere.has(file.path) || (!held.has(file.path) && !mechanical.has(file.path)))
  if (other.length > 0) {
    const children = buildFileTree(other, { key: OTHER_CHANGES_KEY, depth: 1 }).map(node => (rest === undefined ? node : asAlso(node, rest, restElsewhere)))
    nodes.push(groupNode(OTHER_CHANGES_KEY, OTHER_CHANGES, 0, undefined, children, "other"))
  }
  return withMechanicalGroup(nodes, files, mechanical, held)
}

/** `nodes`, then the Mechanical group of the mechanical files no concern holds (`held`), when there is one */
function withMechanicalGroup(
  nodes: FileTreeNode[],
  files: readonly CommitFile[],
  mechanical: ReadonlyMap<string, MechanicalKind>,
  held: ReadonlySet<string> = new Set(),
): FileTreeNode[] {
  const folded = flattenTreeFiles(buildFileTree(files.filter(file => mechanical.has(file.path) && !held.has(file.path))))
  if (folded.length === 0) {
    return nodes
  }
  return [
    ...nodes,
    groupNode(MECHANICAL_GROUP_KEY, formatMechanicalGroup(null, folded.length), 0, undefined, kindGroups(folded, mechanical, MECHANICAL_GROUP_KEY, 1), "mechanical"),
  ]
}

/** A group per kind of the mechanical files `folded`, in the order of the kinds of `mechanical`, each file by its path */
function kindGroups(folded: readonly CommitFile[], mechanical: ReadonlyMap<string, MechanicalKind>, parentKey: string, depth: number): FileTreeDirectory[] {
  const groups: FileTreeDirectory[] = []
  for (const kind of new Set(mechanical.values())) {
    const ofKind = folded.filter(file => mechanical.get(file.path) === kind)
    if (ofKind.length > 0) {
      const key = `${parentKey}${kind}/`
      const children: FileTreeFile[] = ofKind.map(file => ({ kind: "file", key: file.path, name: file.path, depth: depth + 1, parentKey: key, file, inGroup: true }))
      groups.push(groupNode(key, formatMechanicalGroup(kind, ofKind.length), depth, parentKey, children, kind))
    }
  }
  return groups
}

function groupNode(
  key: string,
  name: string,
  depth: number,
  parentKey: string | undefined,
  children: FileTreeNode[],
  group: FileTreeDirectory["group"],
  title?: string,
): FileTreeDirectory {
  const files = flattenTreeFiles(children)
  const additions = files.reduce((sum, file) => sum + file.additions, 0)
  const deletions = files.reduce((sum, file) => sum + file.deletions, 0)
  return { kind: "directory", key, name, depth, parentKey, children, fileCount: files.length, additions, deletions, group, ...(title !== undefined && { title }) }
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
      } else if (node.kind === "directory") {
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
  if (node.kind !== "directory") {
    return { kind: "file", path: node.file.path }
  }
  const files = flattenTreeFiles(node.children).map(file => file.path)
  return node.group === undefined ? { kind: "directory", path: node.key.slice(0, -1), files } : { kind: "group", files }
}

/** The files in the order of the tree: what the tree shows from top to bottom with every directory open */
export function flattenTreeFiles(nodes: readonly FileTreeNode[]): CommitFile[] {
  return nodes.flatMap(node => (node.kind === "file" ? [node.file] : node.kind === "directory" ? flattenTreeFiles(node.children) : []))
}

/** A section of the story as the tree reads it (`NarrativeSection`) */
interface StorySection {
  readonly key: string
  readonly path: string
  /** `Concern.id` of the concern it is of, null for a section of no concern */
  readonly concern: string | null
  /** It starts collapsed */
  readonly folded: string | null
  /** A use left outside: no row of the tree */
  readonly ghost: object | null
}

/**
 * The row each section of the story selects as it comes to the top (`rowOf`) and the section each row shows
 * (`sectionOf`): the row "also in" of the file in the concern of the section, else the row of its file; a row shows the
 * first open section of it, else its first one. A use left outside has no row
 */
export function sectionRows(sections: readonly StorySection[], nodes: readonly FileTreeNode[]): { rowOf: Map<string, string>; sectionOf: Map<string, string> } {
  const also = new Set<string>()
  const visit = (list: readonly FileTreeNode[]) => {
    for (const node of list) {
      if (node.kind === "also") {
        also.add(node.key)
      } else if (node.kind === "directory") {
        visit(node.children)
      }
    }
  }
  visit(nodes)
  const rowOf = new Map<string, string>()
  const sectionOf = new Map<string, string>()
  const open = new Set<string>()
  for (const section of sections) {
    if (section.ghost !== null) {
      continue
    }
    const alsoKey = section.concern === null ? null : alsoRowKey(section.concern, section.path)
    const row = alsoKey !== null && also.has(alsoKey) ? alsoKey : section.path
    rowOf.set(section.key, row)
    if (!open.has(row) && (!sectionOf.has(row) || section.folded === null)) {
      sectionOf.set(row, section.key)
      if (section.folded === null) {
        open.add(row)
      }
    }
  }
  return { rowOf, sectionOf }
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
 * The groups of a review that start collapsed: the Mechanical group, and the mechanical files of each concern (the
 * generated files that follow their source)
 */
export function foldedGroups(nodes: readonly FileTreeNode[]): string[] {
  return nodes.flatMap(node => {
    if (node.kind !== "directory" || node.group === undefined) {
      return []
    }
    if (node.group === "concern") {
      return node.children.flatMap(child => (child.kind === "directory" && child.group !== undefined ? [child.key] : []))
    }
    return node.group === "mechanical" ? [node.key] : []
  })
}

/** The tree is that of a review with its groups: the concerns, Other changes, the Mechanical group */
export function hasReviewGroups(nodes: readonly FileTreeNode[]): boolean {
  return nodes.some(node => node.kind === "directory" && node.group !== undefined)
}

/**
 * The directories that start collapsed: the groups of a review that start so (`foldedGroups`), and the directories
 * below the top level when the commit has more than `AUTO_COLLAPSE_FILE_COUNT` files
 */
export function getInitiallyCollapsed(nodes: readonly FileTreeNode[], fileCount: number): Set<string> {
  const collapsed = new Set<string>(foldedGroups(nodes))
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

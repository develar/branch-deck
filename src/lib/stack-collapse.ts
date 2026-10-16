import { branchRowIn } from "@/lib/branch-row-keys"
import type { StackLanes } from "@/lib/stack-lanes"
import type { BranchTableRow } from "@/stores/branch-sync"

type BranchRowOf = Extract<BranchTableRow, { kind: "branch" }>

/** The root of a stack: the first row of its stack, at the level of its issue family (`depth` counts both) */
export function isStackRoot(row: BranchTableRow): row is BranchRowOf {
  return row.kind === "branch" && row.depth === row.familyIndent
}

/** The index after the last row of the stack on the root at `index`: the rows below it at its family level, deeper */
function stackEnd(rows: readonly BranchTableRow[], index: number): number {
  const root = rows[index] as BranchRowOf
  let end = index + 1
  while (end < rows.length) {
    const row = rows[end]!
    if (row.kind !== "branch" || row.familyIndent !== root.familyIndent || row.depth <= root.depth) {
      break
    }
    end++
  }
  return end
}

/** The root of the stack `name` is in, itself for a root; null for a branch without a row */
export function stackRootOf(rows: readonly BranchTableRow[], name: string): string | null {
  const found = branchRowIn(rows, name)
  if (found === null) {
    return null
  }
  const { index, row } = found
  for (let above = index; above >= 0; above--) {
    const candidate = rows[above]!
    if (candidate.kind === "issue") {
      return null
    }
    if (isStackRoot(candidate) && candidate.familyIndent === row.familyIndent) {
      return candidate.name
    }
  }
  return null
}

/** The size of the stack on each root among `rows` that has a branch stacked on it, the root included */
export function stackSizes(rows: readonly BranchTableRow[]): Map<string, number> {
  const sizes = new Map<string, number>()
  rows.forEach((row, index) => {
    if (isStackRoot(row)) {
      const size = stackEnd(rows, index) - index
      if (size > 1) {
        sizes.set(row.name, size)
      }
    }
  })
  return sizes
}

/** The branches of the stack on `root` among `rows`, the root first; empty for a branch that isn't a root here */
export function stackMembers(rows: readonly BranchTableRow[], root: string): string[] {
  const found = branchRowIn(rows, root)
  if (found === null || !isStackRoot(found.row)) {
    return []
  }
  return rows.slice(found.index, stackEnd(rows, found.index)).flatMap(it => it.kind === "branch" ? [it.name] : [])
}

export interface CollapsedRows {
  readonly rows: readonly BranchTableRow[]
  /** The collapsed roots, with the count of the branches each hides */
  readonly hidden: ReadonlyMap<string, number>
}

const NOTHING_HIDDEN: ReadonlyMap<string, number> = new Map()

/**
 * The rows without the branches stacked on a `collapsed` root, which stays as the header of its stack; the given rows
 * while no root among them is collapsed. A root with nothing stacked on it has nothing to hide.
 */
export function collapseStacks(rows: readonly BranchTableRow[], collapsed: ReadonlySet<string>): CollapsedRows {
  return collapseRuns(rows, collapsed, isStackRoot)
}

/** The rows without the branches below each `collapsed` row that `heads` (they are deeper, `stackEnd`) */
function collapseRuns(rows: readonly BranchTableRow[], collapsed: ReadonlySet<string>, heads: (row: BranchTableRow) => row is BranchRowOf): CollapsedRows {
  if (collapsed.size === 0) {
    return { rows, hidden: NOTHING_HIDDEN }
  }
  const shown: BranchTableRow[] = []
  const hidden = new Map<string, number>()
  let index = 0
  while (index < rows.length) {
    const row = rows[index]!
    shown.push(row)
    if (heads(row) && collapsed.has(row.name)) {
      const end = stackEnd(rows, index)
      if (end > index + 1) {
        hidden.set(row.name, end - index - 1)
        index = end
        continue
      }
    }
    index++
  }
  return hidden.size === 0 ? { rows, hidden: NOTHING_HIDDEN } : { rows: shown, hidden }
}

const collapsedLanesCache = new WeakMap<StackLanes, StackLanes>()

/** The lanes of a collapsed root: no line runs down from its node to the hidden rows; one object per lanes, for memo */
export function collapsedLanes(lanes: StackLanes): StackLanes {
  let known = collapsedLanesCache.get(lanes)
  if (known === undefined) {
    known = { ...lanes, down: false }
    collapsedLanesCache.set(lanes, known)
  }
  return known
}

/** The collapsed roots that still head a stack among `rows`: one renamed, merged or stacked on another branch leaves */
export function pruneCollapsedStacks(collapsed: readonly string[], rows: readonly BranchTableRow[]): string[] {
  const roots = stackSizes(rows)
  return [...new Set(collapsed)].filter(name => roots.has(name))
}

/** A stack of this many branches is an island of its own below the branch table (`stackIslandRoots`) */
export const STACK_ISLAND_MIN_BRANCHES = 5
/** …or of this many commits, the root's included: a short stack of long branches is as hard to read in the table */
export const STACK_ISLAND_MIN_COMMITS = 40

/**
 * The roots of the stacks among all `rows` (`getBranchTableRows(null)`, not the rows shown: neither a collapse nor a
 * filter moves a stack between the table and its island) that the main window shows as islands of their own: a stack
 * at the top level of the table, of `STACK_ISLAND_MIN_BRANCHES` branches or `STACK_ISLAND_MIN_COMMITS` commits. A stack
 * in an issue family, or on an issue branch that heads one (its family follows its stack), stays in the table.
 */
export function stackIslandRoots(rows: readonly BranchTableRow[], commitCountOf: (name: string) => number): Set<string> {
  const roots = new Set<string>()
  rows.forEach((row, index) => {
    if (!isStackRoot(row) || row.familyIndent !== 0) {
      return
    }
    const end = stackEnd(rows, index)
    const next = rows.at(end)
    if (end - index < 2 || (next?.kind === "branch" && next.familyIndent === 1)) {
      return
    }
    let commits = 0
    for (let member = index; member < end; member++) {
      commits += commitCountOf((rows[member] as BranchRowOf).name)
    }
    if (end - index >= STACK_ISLAND_MIN_BRANCHES || commits >= STACK_ISLAND_MIN_COMMITS) {
      roots.add(row.name)
    }
  })
  return roots
}

/**
 * The roots of the side stacks among `rows`, the branches of a stack forked off the lane of their parent with a branch
 * stacked on them, with the count of their rows, the root included: what a click on their chip collapses
 */
export function sideStackSizes(rows: readonly BranchTableRow[]): Map<string, number> {
  const sizes = new Map<string, number>()
  rows.forEach((row, index) => {
    if (isSideRoot(row)) {
      const size = stackEnd(rows, index) - index
      if (size > 1) {
        sizes.set(row.name, size)
      }
    }
  })
  return sizes
}

/** A branch of a stack forked off the lane of its parent: the root of a side stack when a branch is stacked on it */
function isSideRoot(row: BranchTableRow): row is BranchRowOf {
  return row.kind === "branch" && row.lanes?.forkFrom != null
}

/**
 * The rows without the branches stacked on a `collapsed` side stack root (`sideStackSizes`), which stays with its chip;
 * the given rows while none collapses
 */
export function collapseSideStacks(rows: readonly BranchTableRow[], collapsed: ReadonlySet<string>): CollapsedRows {
  return collapseRuns(rows, collapsed, isSideRoot)
}

/** The stacks and the side stacks collapsed, `collapseStacks` then `collapseSideStacks` */
function collapseBoth(rows: readonly BranchTableRow[], stacks: ReadonlySet<string>, sideStacks: ReadonlySet<string>): CollapsedRows {
  const outer = collapseStacks(rows, stacks)
  const inner = collapseSideStacks(outer.rows, sideStacks)
  return inner.hidden.size === 0 ? outer : { rows: inner.rows, hidden: new Map([...outer.hidden, ...inner.hidden]) }
}

/** A stack shown as an island of its own below the branch table */
export interface StackIsland {
  readonly root: string
  /** The rows of the stack: the filter's matches and their ancestors, or all */
  readonly rows: readonly BranchTableRow[]
  /** The rows shown, without the branches of its collapsed side stacks, and the roots they hide rows of */
  readonly shown: CollapsedRows
  /** Collapsed into its header: no row shows */
  readonly collapsed: boolean
}

export interface BranchTablesLayout {
  /** The rows of the branch table before the collapse, without the islands */
  readonly tableAll: readonly BranchTableRow[]
  /** The rows of the branch table as shown, and the roots its collapsed stacks hide rows of */
  readonly table: CollapsedRows
  /** The islands below the table, in table order, each with a row under a filter */
  readonly islands: readonly StackIsland[]
  /** The branch rows shown, in the order of the screen: the table, then the islands */
  readonly order: readonly string[]
}

function branchNamesOf(rows: readonly BranchTableRow[]): string[] {
  return rows.flatMap(row => row.kind === "branch" ? [row.name] : [])
}

/** What is collapsed: the roots of stacks (a stack of the table, an island) and of side stacks; null under a filter */
export interface TableCollapse {
  readonly stacks: ReadonlySet<string>
  readonly sideStacks: ReadonlySet<string>
}

/**
 * The branch table and the islands below it for `rows` (the filter's matches and their ancestors, or all): the stacks
 * on `islandRoots` move out of the table, whole; `collapsed` (null under a filter, which shows its matches in their
 * stacks) collapses a stack of the table into its root, an island into its header and a side stack into its root
 */
export function layoutBranchTables(rows: readonly BranchTableRow[], islandRoots: ReadonlySet<string>, collapsed: TableCollapse | null): BranchTablesLayout {
  const rest: BranchTableRow[] = []
  const islands: StackIsland[] = []
  let index = 0
  while (index < rows.length) {
    const row = rows[index]!
    if (isStackRoot(row) && row.familyIndent === 0 && islandRoots.has(row.name)) {
      const end = stackEnd(rows, index)
      const stackRows = rows.slice(index, end)
      islands.push({
        root: row.name,
        rows: stackRows,
        shown: collapsed === null ? { rows: stackRows, hidden: NOTHING_HIDDEN } : collapseSideStacks(stackRows, collapsed.sideStacks),
        collapsed: collapsed?.stacks.has(row.name) ?? false,
      })
      index = end
      continue
    }
    rest.push(row)
    index++
  }
  // the given rows while no stack is an island, so a memo of the rows holds
  const tableAll = islands.length === 0 ? rows : rest
  const table = collapsed === null ? { rows: tableAll, hidden: NOTHING_HIDDEN } : collapseBoth(tableAll, collapsed.stacks, collapsed.sideStacks)
  const order = [...branchNamesOf(table.rows), ...islands.flatMap(island => island.collapsed ? [] : branchNamesOf(island.shown.rows))]
  return { tableAll, table, islands, order }
}

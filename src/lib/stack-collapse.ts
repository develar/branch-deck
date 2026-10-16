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
  if (collapsed.size === 0) {
    return { rows, hidden: NOTHING_HIDDEN }
  }
  const shown: BranchTableRow[] = []
  const hidden = new Map<string, number>()
  let index = 0
  while (index < rows.length) {
    const row = rows[index]!
    shown.push(row)
    if (isStackRoot(row) && collapsed.has(row.name)) {
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

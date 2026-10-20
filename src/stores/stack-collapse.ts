import { useMemo } from "react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { isFilterActive } from "@/lib/branch-filter"
import { summarizePushState } from "@/lib/push-summary"
import { layoutBranchTables, pruneCollapsedStacks, sideStackSizes, stackMembers, stackRootOf } from "@/lib/stack-collapse"
import type { BranchTablesLayout, TableCollapse } from "@/lib/stack-collapse"
import { getBranchFilter, getShownBranches } from "@/stores/branch-filter"
import type { BranchSyncState, BranchTableRow } from "@/stores/branch-sync"
import {
  branchSyncStore,
  getBranchTableRows,
  getParkedTableRows,
  getStackIslandRoots,
  stackDescendantsIn,
  useBranchSync,
  useStackIslandRoots,
  useVisibleBranchTableRows,
} from "@/stores/branch-sync"
import { closeInline, inlineRowActionStore } from "@/stores/inline-row-action"
import { getRecentProjects, setCollapsedStacks, useRecentProjects } from "@/stores/settings/repositorySettings"

const NONE: readonly string[] = []

/** The stack roots collapsed in the branch table of the selected repository (`collapsedStacks`, per repository) */
export function useCollapsedStacks(): ReadonlySet<string> {
  const list = useRecentProjects(projects => projects[0]?.collapsedStacks ?? NONE)
  return useMemo(() => new Set(list), [list])
}

function collapsedList(): readonly string[] {
  return getRecentProjects()[0]?.collapsedStacks ?? NONE
}

export function getCollapsedStacks(): ReadonlySet<string> {
  return new Set(collapsedList())
}

/** The stacks of the table can collapse now: not under a filter, which shows its matches in their stacks */
export function canCollapseStacks(): boolean {
  return !isFilterActive(getBranchFilter())
}

function writeCollapsed(roots: readonly string[]): void {
  const path = getRecentProjects()[0]?.path
  if (path !== undefined) {
    setCollapsedStacks(path, pruneCollapsedStacks(roots, getBranchTableRows(null)))
  }
}

/**
 * Collapses the stack of `name` (any branch of it) into its root, or expands it; returns the root, null for a branch
 * outside a stack or under a filter. An inline form of a branch the collapse hides closes; the form under the header of
 * an island stays, the header shows it while collapsed.
 */
export function setStackCollapsed(name: string, collapsed: boolean): string | null {
  const rows = getBranchTableRows(null)
  const root = stackRootOf(rows, name)
  if (root === null || !canCollapseStacks() || stackMembers(rows, root).length < 2) {
    return null
  }
  if (collapsed) {
    const active = inlineRowActionStore.getState().activeInline
    if (active !== null && active.branchName !== root && stackMembers(rows, root).includes(active.branchName)) {
      closeInline()
    }
  }
  writeCollapsed([...collapsedList().filter(it => it !== root), ...(collapsed ? [root] : [])])
  return root
}

/**
 * Collapses or expands the stack of `name` from the chip, the keys or the menu; the focus moves to the root when the
 * collapse hides the focused row, to the header of an island, which hides its root too. Returns the root, null when
 * nothing changed.
 */
export function toggleStackCollapsed(name: string, collapsed: boolean): string | null {
  const focusedRow = document.activeElement?.closest<HTMLElement>("tr[data-branch-name], tr[data-branch-of]")
  const root = setStackCollapsed(name, collapsed)
  if (root !== null && collapsed && focusedRow != null) {
    const focused = focusedRow.dataset.branchName ?? focusedRow.dataset.branchOf
    if (isStackIsland(root)) {
      document.querySelector<HTMLElement>(`[data-stack-root="${CSS.escape(root)}"] [data-slot=collapsible-trigger]`)?.focus({ preventScroll: true })
    } else if (focused !== root) {
      document.querySelector<HTMLElement>(`tr[data-testid=branch-row][data-branch-name="${CSS.escape(root)}"]`)?.focus({ preventScroll: true })
    }
  }
  return root
}

/** The stack on `root` is an island of its own below the branch table (`stackIslandRoots`) */
export function isStackIsland(root: string): boolean {
  return getStackIslandRoots().has(root)
}

/** The stack `name` is in and whether it is collapsed, for its menu; null outside a stack or under a filter */
export function getStackCollapse(name: string): { root: string; collapsed: boolean } | null {
  const rows = getBranchTableRows(null)
  const root = stackRootOf(rows, name)
  if (root === null || !canCollapseStacks() || stackMembers(rows, root).length < 2) {
    return null
  }
  return { root, collapsed: collapsedList().includes(root) }
}

/**
 * Expands the stack that hides `name`, if one does, and the side stacks: a reveal, an inline form opened elsewhere. A
 * collapsed island hides its root too.
 */
export function expandStackOf(name: string): void {
  const root = stackRootOf(getBranchTableRows(null), name)
  if (root !== null && (root !== name || isStackIsland(root)) && collapsedList().includes(root)) {
    setStackCollapsed(root, false)
  }
  expandSideStacksOf(name)
}

/** A collapsed root was renamed: its stack stays collapsed under the new name */
export function renameCollapsedStack(from: string, to: string): void {
  const list = collapsedList()
  if (list.includes(from)) {
    const path = getRecentProjects()[0]?.path
    if (path !== undefined) {
      // the rows don't know the new name before the next sync: no pruning here
      setCollapsedStacks(
        path,
        list.map(it => (it === from ? to : it)),
      )
    }
  }
}

/** A collapsed root was stacked on another branch: it no longer heads a stack */
export function forgetCollapsedStack(root: string): void {
  if (collapsedList().includes(root)) {
    writeCollapsed(collapsedList().filter(it => it !== root))
  }
}

/** The side stacks the user collapsed (true) or expanded (false) in this session, by root; the others follow `isQuiet` */
const sideStackStore = createStore<{ readonly overrides: ReadonlyMap<string, boolean> }>(() => ({ overrides: new Map() }))

/** A side stack whose branches have nothing for the remote and nothing wrong: collapsed until the user expands it */
function isQuiet(state: BranchSyncState, root: string): boolean {
  const branches = [root, ...stackDescendantsIn(state, root)].flatMap(name => state.branches[name] ?? [])
  const push = summarizePushState(branches)
  return branches.every(branch => !branch.hasError && branch.status !== "Blocked") && push.branches.length === 0 && push.notLoaded === 0
}

/** The collapsed side stacks of all rows, one per line, so a store change notifies only when they change */
function collapsedSideStackKey(state: BranchSyncState, overrides: ReadonlyMap<string, boolean>): string {
  const roots = [...sideStackSizes(getBranchTableRows(null)).keys()]
  return roots.filter(root => overrides.get(root) ?? isQuiet(state, root)).join("\n")
}

function splitKey(key: string): ReadonlySet<string> {
  return new Set(key === "" ? [] : key.split("\n"))
}

/** The collapsed side stacks: the user's choice of this session, else the quiet ones */
export function useCollapsedSideStacks(): ReadonlySet<string> {
  const overrides = useStore(sideStackStore, state => state.overrides)
  const key = useBranchSync(state => collapsedSideStackKey(state, overrides))
  return useMemo(() => splitKey(key), [key])
}

export function getCollapsedSideStacks(): ReadonlySet<string> {
  return splitKey(collapsedSideStackKey(branchSyncStore.getState(), sideStackStore.getState().overrides))
}

/**
 * Collapses the side stack on `root` into its row, or expands it, for this session; the focus moves to the root when
 * the collapse hides the focused row. Returns whether `root` heads a side stack.
 */
export function toggleSideStackCollapsed(root: string, collapsed: boolean): boolean {
  if (!canCollapseStacks() || !sideStackSizes(getBranchTableRows(null)).has(root)) {
    return false
  }
  const focusedRow = document.activeElement?.closest<HTMLElement>("tr[data-branch-name], tr[data-branch-of]")
  sideStackStore.setState(state => ({ overrides: new Map([...state.overrides, [root, collapsed]]) }))
  const focused = focusedRow?.dataset.branchName ?? focusedRow?.dataset.branchOf
  if (collapsed && focused !== undefined && focused !== root) {
    document.querySelector<HTMLElement>(`tr[data-testid=branch-row][data-branch-name="${CSS.escape(root)}"]`)?.focus({ preventScroll: true })
  }
  return true
}

/**
 * The side stack ⌥← collapses from the row of `name`: the nearest one that holds it, expanded, its root itself
 * included; null when none does (⌥← collapses the whole stack then)
 */
export function sideStackToCollapse(name: string): string | null {
  const rows = getBranchTableRows(null)
  const sizes = sideStackSizes(rows)
  const collapsed = getCollapsedSideStacks()
  const index = rows.findIndex(row => row.kind === "branch" && row.name === name)
  for (let above = index; above >= 0; above--) {
    const row = rows[above]!
    if (row.kind !== "branch") {
      return null
    }
    const size = sizes.get(row.name)
    if (size !== undefined && above + size > index && !collapsed.has(row.name)) {
      return row.name
    }
    // the root of the stack: no side stack above
    if (row.depth === row.familyIndent) {
      return null
    }
  }
  return null
}

/** Expands the side stacks that hide `name`: a reveal, a key that steps to it */
export function expandSideStacksOf(name: string): void {
  const rows = getBranchTableRows(null)
  const index = rows.findIndex(row => row.kind === "branch" && row.name === name)
  const collapsed = getCollapsedSideStacks()
  for (const [root, size] of sideStackSizes(rows)) {
    const at = rows.findIndex(row => row.kind === "branch" && row.name === root)
    if (collapsed.has(root) && at < index && index < at + size) {
      toggleSideStackCollapsed(root, false)
    }
  }
}

/** The branch rows as shown, in the order of the screen: the table, the islands below it (`layoutBranchTables`), the Parked card */
export function getShownBranchTableOrder(): readonly string[] {
  const filtered = !canCollapseStacks()
  const rows = getBranchTableRows(filtered ? getShownBranches() : null)
  const order = layoutBranchTables(rows, getStackIslandRoots(), filtered ? null : { stacks: getCollapsedStacks(), sideStacks: getCollapsedSideStacks() }).order
  // the Parked card comes last, while it is open
  const parked = getRecentProjects()[0]?.parkedCollapsed === true ? [] : getParkedTableRows().flatMap(row => (row.kind === "branch" ? [row.name] : []))
  return parked.length === 0 ? order : [...order, ...parked]
}

export interface ShownBranchTables extends BranchTablesLayout {
  /** The rows before the collapse and the split into islands: the filter's matches and their ancestors, or all */
  readonly all: readonly BranchTableRow[]
}

/**
 * The branch table and the islands below it as shown: the rows of the `visible` branches under a filter (which ignores
 * the collapse, so a match deep in a collapsed stack shows), else without the branches stacked on a collapsed root and
 * the rows of a collapsed island
 */
export function useShownBranchTables(visible: ReadonlySet<string> | null): ShownBranchTables {
  const all = useVisibleBranchTableRows(visible)
  const islandRoots = useStackIslandRoots()
  const stacks = useCollapsedStacks()
  const sideStacks = useCollapsedSideStacks()
  return useMemo(() => {
    const collapsed: TableCollapse | null = visible === null ? { stacks, sideStacks } : null
    return { all, ...layoutBranchTables(all, islandRoots, collapsed) }
  }, [all, islandRoots, stacks, sideStacks, visible])
}

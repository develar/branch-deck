import { useMemo } from "react"
import { isFilterActive } from "@/lib/branch-filter"
import { layoutBranchTables, pruneCollapsedStacks, stackMembers, stackRootOf } from "@/lib/stack-collapse"
import type { BranchTablesLayout } from "@/lib/stack-collapse"
import type { BranchTableRow } from "@/stores/branch-sync"
import { getBranchTableRows, getStackIslandRoots, useStackIslandRoots, useVisibleBranchTableRows } from "@/stores/branch-sync"
import { getBranchFilter, getShownBranches } from "@/stores/branch-filter"
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
 * outside a stack or under a filter. An inline form of a branch the collapse hides closes.
 */
export function setStackCollapsed(name: string, collapsed: boolean): string | null {
  const rows = getBranchTableRows(null)
  const root = stackRootOf(rows, name)
  if (root === null || !canCollapseStacks() || stackMembers(rows, root).length < 2) {
    return null
  }
  if (collapsed) {
    const open = inlineRowActionStore.getState().activeInline?.branchName
    if (open !== undefined && open !== root && stackMembers(rows, root).includes(open)) {
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
    }
    else if (focused !== root) {
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
export function getStackCollapse(name: string): { root: string, collapsed: boolean } | null {
  const rows = getBranchTableRows(null)
  const root = stackRootOf(rows, name)
  if (root === null || !canCollapseStacks() || stackMembers(rows, root).length < 2) {
    return null
  }
  return { root, collapsed: collapsedList().includes(root) }
}

/**
 * Expands the stack that hides `name`, if one does: a reveal, an inline form opened elsewhere. A collapsed island hides
 * its root too.
 */
export function expandStackOf(name: string): void {
  const root = stackRootOf(getBranchTableRows(null), name)
  if (root !== null && (root !== name || isStackIsland(root)) && collapsedList().includes(root)) {
    setStackCollapsed(root, false)
  }
}

/** A collapsed root was renamed: its stack stays collapsed under the new name */
export function renameCollapsedStack(from: string, to: string): void {
  const list = collapsedList()
  if (list.includes(from)) {
    const path = getRecentProjects()[0]?.path
    if (path !== undefined) {
      // the rows don't know the new name before the next sync: no pruning here
      setCollapsedStacks(path, list.map(it => it === from ? to : it))
    }
  }
}

/** A collapsed root was stacked on another branch: it no longer heads a stack */
export function forgetCollapsedStack(root: string): void {
  if (collapsedList().includes(root)) {
    writeCollapsed(collapsedList().filter(it => it !== root))
  }
}

/** The branch rows as shown, in the order of the screen: the table, then the islands below it (`layoutBranchTables`) */
export function getShownBranchTableOrder(): readonly string[] {
  const filtered = !canCollapseStacks()
  const rows = getBranchTableRows(filtered ? getShownBranches() : null)
  return layoutBranchTables(rows, getStackIslandRoots(), filtered ? null : getCollapsedStacks()).order
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
  const collapsed = useCollapsedStacks()
  return useMemo(() => ({ all, ...layoutBranchTables(all, islandRoots, visible === null ? collapsed : null) }), [all, islandRoots, collapsed, visible])
}

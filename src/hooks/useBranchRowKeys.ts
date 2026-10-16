import type { KeyboardEvent, RefObject } from "react"
import { canPushNow } from "@/hooks/usePushButton"
import { branchReviewBlocker } from "@/lib/branch-review"
import { openBranchReview } from "@/lib/branch-review-window"
import { needsAttention } from "@/lib/branch-filter"
import { attentionRow, neighbourRow, parentRow } from "@/lib/branch-row-keys"
import { pushOrConfirm } from "@/lib/git/push"
import { canStartRobot } from "@/lib/patronus"
import { openPullRequest } from "@/lib/pull-request"
import type { BranchTableRow } from "@/stores/branch-sync"
import { branchSyncStore, getBranch, getBranchTableOrder, isBranchPushing, isStackPushing } from "@/stores/branch-sync"
import { selectBranches, setBranchTargetOpen } from "@/stores/commit-move"
import { isProcessing, openInline } from "@/stores/inline-row-action"
import { expandStackOf, toggleStackCollapsed } from "@/stores/stack-collapse"
import { getBranchRobot, getLandingAncestor, getLandingSpan, isPatronusAvailable } from "@/stores/patronus"
import { getPullRequest } from "@/stores/pull-requests"

export interface BranchRowKeysOptions {
  tableRef: RefObject<HTMLTableElement | null>
  /** The rows of the table as shown (the filter applied) */
  rows: readonly BranchTableRow[]
  expandedKeys: ReadonlySet<string>
  setExpanded: (name: string, expanded: boolean) => void
  toggleExpanded: (name: string) => void
  /** The row Space falls back on while the table has the focus: the row the focus moves to */
  lastFocusedRef: RefObject<string | null>
  /** The branch rows the filter shows, in table order, the rows of a collapsed stack too: what F2 steps through */
  unfoldedNames: readonly string[]
}

export interface BranchRowKeys {
  /** For a key without a modifier on the table or one of its branch rows */
  handleRowKey: (event: KeyboardEvent<HTMLTableElement>) => void
  /** ⌥← / ⌥→ on a branch row: collapses its stack into the root, which takes the focus, or expands it */
  handleStackKey: (event: KeyboardEvent<HTMLTableElement>) => void
  /** ↓ in the search of the filter bar */
  focusFirstRow: () => void
  /**
   * F2 (`step` 1) or Shift+F2 (-1): the focus moves to the next or the previous branch row that waits for the user
   * (`needsAttention`, the rows of Attention), expanding the collapsed stack it is in; nothing while none waits
   */
  focusAttentionRow: (step: 1 | -1) => void
}

/** P: pushes as the push button of the row does (`pushOrConfirm`), when that button is enabled */
function pushRow(name: string): void {
  const branch = getBranch(name)
  const isPushing = isBranchPushing(name) || isStackPushing(name)
  if (branch !== undefined && canPushNow(branch, { isSyncing: branchSyncStore.getState().isSyncing, isPushing, processing: isProcessing(name) })) {
    pushOrConfirm(name, "row")
  }
}

/**
 * S: opens the Safe Push form below the row when its "Safe Push…" is enabled (Patronus answered, the row has a push,
 * `canStartRobot`, no inline action running); the form focuses its start button, so Enter starts the robot
 */
function openSafePush(name: string): void {
  const branch = getBranch(name)
  if (branch === undefined || !isPatronusAvailable() || branch.hasError || branch.commitCount === 0 || isProcessing(name)) {
    return
  }
  if (canStartRobot(branch, getBranchRobot(name), getLandingAncestor(name), getLandingSpan(name)).ok) {
    openInline("safe-push", name)
  }
}

/** D: opens the review window of the branch, as "Review Changes" of its menu, when that is enabled */
function reviewRow(name: string): void {
  const branch = getBranch(name)
  if (branch !== undefined && branchReviewBlocker(branch) === null) {
    void openBranchReview(name)
  }
}

/** O: opens the pull request of the branch, as "Open Pull Request" of its menu */
function openRowPullRequest(name: string): void {
  const pullRequest = getPullRequest(name)
  if (pullRequest !== undefined) {
    openPullRequest(pullRequest)
  }
}

/**
 * M without a selection: selects the row and opens "Move to…" for it, as the item of its menu does, where the menu offers
 * it (a synced branch with commits, no error); with a selection `BranchTableCard` opens it for the selected rows
 */
function moveRow(name: string): void {
  const branch = getBranch(name)
  if (branch !== undefined && branch.commits.length > 0 && !branch.hasError && branch.status !== "Syncing") {
    selectBranches(name, "only", getBranchTableOrder())
    setBranchTargetOpen(true)
  }
}

/**
 * The keys of the focused branch row, for a key without a modifier pressed on the row (one of its buttons too) or on
 * the table: ↑/↓ move the focus between the branch rows (from the table to the first or the last), → expands the row,
 * ← collapses it or moves to the branch it is nested under, Enter on the row itself toggles it, P pushes it, S opens
 * its Safe Push form, D opens its review window, O opens its pull request, M opens "Move to…" for it. A handled key is
 * `preventDefault`ed.
 */
/** The branch waits for the user, as Attention of the filter bar counts it */
function isAttentionRow(name: string): boolean {
  const branch = getBranch(name)
  return branch !== undefined && needsAttention(branch, getBranchRobot(name), getPullRequest(name))
}

export function useBranchRowKeys({ tableRef, rows, expandedKeys, setExpanded, toggleExpanded, lastFocusedRef, unfoldedNames }: BranchRowKeysOptions): BranchRowKeys {
  const branchNames = rows.flatMap(it => it.kind === "branch" ? [it.name] : [])
  const focusRow = (name: string | null) => {
    const row = name === null ? null : tableRef.current?.querySelector<HTMLElement>(`tr[data-testid=branch-row][data-branch-name="${CSS.escape(name)}"]`)
    if (row == null) {
      return
    }
    lastFocusedRef.current = name
    row.focus({ preventScroll: true })
    row.scrollIntoView({ block: "nearest" })
  }

  const handleRowKey = (event: KeyboardEvent<HTMLTableElement>) => {
    const target = event.target as HTMLElement
    const row = target === event.currentTarget ? null : target.closest<HTMLElement>("tr[data-testid=branch-row]")
    const name = row?.dataset.branchName ?? null
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault()
      focusRow(neighbourRow(branchNames, name, event.key === "ArrowDown" ? 1 : -1))
      return
    }
    if (name === null) {
      return
    }
    if (event.key === "ArrowRight") {
      event.preventDefault()
      setExpanded(name, true)
    }
    else if (event.key === "ArrowLeft") {
      event.preventDefault()
      if (expandedKeys.has(name)) {
        setExpanded(name, false)
      }
      else {
        focusRow(parentRow(rows, name))
      }
    }
    // a button of the row keeps its Enter
    else if (event.key === "Enter" && target === row) {
      event.preventDefault()
      toggleExpanded(name)
    }
    // the physical key, in any keyboard layout
    else if (event.code === "KeyP") {
      event.preventDefault()
      pushRow(name)
    }
    else if (event.code === "KeyS") {
      event.preventDefault()
      openSafePush(name)
    }
    else if (event.code === "KeyD") {
      event.preventDefault()
      reviewRow(name)
    }
    else if (event.code === "KeyO") {
      event.preventDefault()
      openRowPullRequest(name)
    }
    else if (event.code === "KeyM") {
      event.preventDefault()
      moveRow(name)
    }
  }

  const handleStackKey = (event: KeyboardEvent<HTMLTableElement>) => {
    const name = (event.target as HTMLElement).closest<HTMLElement>("tr[data-testid=branch-row]")?.dataset.branchName
    if (name === undefined) {
      return
    }
    // Alt+← is Back in a browser view on Windows and Linux
    event.preventDefault()
    // a held key collapses once, not the next stack too
    if (!event.repeat) {
      const root = toggleStackCollapsed(name, event.key === "ArrowLeft")
      if (root !== null) {
        focusRow(root)
      }
    }
  }

  const focusAttentionRow = (step: 1 | -1) => {
    const focused = document.activeElement?.closest<HTMLElement>("tr[data-testid=branch-row]")?.dataset.branchName
    const name = attentionRow(unfoldedNames, isAttentionRow, focused ?? lastFocusedRef.current, step)
    if (name === null) {
      return
    }
    if (branchNames.includes(name)) {
      focusRow(name)
      return
    }
    // in a collapsed stack: its row is there once the stack expanded
    expandStackOf(name)
    requestAnimationFrame(() => focusRow(name))
  }

  return { handleRowKey, handleStackKey, focusFirstRow: () => focusRow(branchNames[0] ?? null), focusAttentionRow }
}

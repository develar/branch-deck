import type { KeyboardEvent, RefObject } from "react"
import { canPushNow } from "@/hooks/usePushButton"
import { requestSafePush } from "@/hooks/useSafePush"
import { needsAttention } from "@/lib/branch-filter"
import { branchReviewBlocker } from "@/lib/branch-review"
import { openBranchReviewWindow, prefetchBranchReview } from "@/lib/branch-review-window"
import { attentionRow, neighbourRow, parentRow } from "@/lib/branch-row-keys"
import { pushOrConfirm } from "@/lib/git/push"
import { openPullRequest } from "@/lib/pull-request"
import type { BranchTableRow } from "@/stores/branch-sync"
import { branchSyncStore, getBranch, getBranchTableOrder, isBranchPushing, isStackPushing } from "@/stores/branch-sync"
import { selectBranches, setBranchTargetOpen } from "@/stores/commit-move"
import { isProcessing } from "@/stores/inline-row-action"
import { getBranchRobot } from "@/stores/patronus"
import { getPullRequest } from "@/stores/pull-requests"
import { expandStackOf, getCollapsedSideStacks, isStackIsland, sideStackToCollapse, toggleSideStackCollapsed, toggleStackCollapsed } from "@/stores/stack-collapse"

export interface BranchRowKeysOptions {
  /** The rows as shown (the filter applied), in the order of the screen: the table, then the islands below it */
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
  /**
   * ⌥← / ⌥→ on a branch row: collapses the nearest expanded side stack that holds it, else its stack, into the root,
   * which takes the focus; expands a collapsed side stack on the row, else its stack
   */
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
  if (branch !== undefined && canPushNow(branch, { isPushing, processing: isProcessing(name) })) {
    pushOrConfirm(name, "row")
  }
}

/**
 * S: opens the popover of the row's Safe Push at its slot when its "Safe Push" is enabled (Patronus answered, the row
 * has a push, the next step's `safePushBlocker`), never a start by itself; its start takes the focus, so Enter starts the robot.
 * Disabled, S says why in an `info` toast, as the hover of the item does (`requestSafePush`).
 */
function openSafePush(name: string): void {
  void requestSafePush(name, { mode: "rebaseAutosquash", via: "key" })
}

/**
 * D: reviews the branch in its window, as "Review Changes" of its menu, when that is enabled. The keys that look at a
 * row (`usePreviewHotkeys`): the focused row, or the one under the pointer.
 */
export function reviewBranchRow(name: string): void {
  const branch = getBranch(name)
  if (branch !== undefined && branchReviewBlocker(branch) === null) {
    void openBranchReviewWindow(name)
  }
}

/** The pointer rests on a branch row or it got the focus: its review is read ahead, if it can be reviewed */
export function prefetchBranchRowReview(name: string): void {
  const branch = getBranch(name)
  if (branch !== undefined && branchReviewBlocker(branch) === null) {
    prefetchBranchReview(name)
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
  if (branch !== undefined && branch.commits.length > 0 && !branch.hasError && branch.status !== "Syncing" && !branchSyncStore.getState().isSyncing) {
    selectBranches(name, "only", getBranchTableOrder())
    setBranchTargetOpen(true)
  }
}

/**
 * The keys of the focused branch row, for a key without a modifier pressed on the row (one of its buttons too) or on
 * the table: ↑/↓ move the focus between the branch rows (from the table to the first or the last), → expands the row,
 * ← collapses it or moves to the branch it is nested under, Enter on the row itself toggles it, P pushes it, S opens
 * its Safe Push popover, D opens its review window, O opens its pull request, M opens "Move to…" for it. A handled key is
 * `preventDefault`ed.
 */
/** The branch waits for the user, as Attention of the filter bar counts it */
function isAttentionRow(name: string): boolean {
  const branch = getBranch(name)
  return branch !== undefined && needsAttention(branch, getBranchRobot(name), getPullRequest(name))
}

export function useBranchRowKeys({ rows, expandedKeys, setExpanded, toggleExpanded, lastFocusedRef, unfoldedNames }: BranchRowKeysOptions): BranchRowKeys {
  const branchNames = rows.flatMap(it => (it.kind === "branch" ? [it.name] : []))
  // in the table or in an island below it
  const focusRow = (name: string | null) => {
    const row = name === null ? null : document.querySelector<HTMLElement>(`tr[data-testid=branch-row][data-branch-name="${CSS.escape(name)}"]`)
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
    } else if (event.key === "ArrowLeft") {
      event.preventDefault()
      if (expandedKeys.has(name)) {
        setExpanded(name, false)
      } else {
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
    } else if (event.code === "KeyS") {
      event.preventDefault()
      openSafePush(name)
    } else if (event.code === "KeyO") {
      event.preventDefault()
      openRowPullRequest(name)
    } else if (event.code === "KeyM") {
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
      const collapse = event.key === "ArrowLeft"
      // as ← in a tree: the nearest side stack first, then the whole stack; → expands a collapsed side stack first
      const side = collapse ? sideStackToCollapse(name) : getCollapsedSideStacks().has(name) ? name : null
      if (side !== null && toggleSideStackCollapsed(side, collapse)) {
        focusRow(side)
        return
      }
      const root = toggleStackCollapsed(name, collapse)
      // a collapsed island hides its root too: its header has the focus
      if (root !== null && !(collapse && isStackIsland(root))) {
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

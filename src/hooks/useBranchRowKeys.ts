import type { KeyboardEvent, RefObject } from "react"
import { getBranchContextMenuItems } from "@/hooks/useBranchContextActions"
import { getMoveRowsNow } from "@/hooks/useBranchFilter"
import { requestSafePush } from "@/hooks/useSafePush"
import { branchReviewBlocker } from "@/lib/branch-review"
import { prefetchBranchReview } from "@/lib/branch-review-window"
import { moveRow, neighbourRow, parentRow } from "@/lib/branch-row-keys"
import { FROM_MENU, selectMenuItemById } from "@/lib/menu"
import { showToast } from "@/lib/toast-message"
import type { BranchTableRow } from "@/stores/branch-sync"
import { getBranch } from "@/stores/branch-sync"
import { getOperations } from "@/stores/operations"
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
   * F2 (`step` 1) or Shift+F2 (-1): the focus moves to the next or the previous branch row that is the user's move (the
   * rows of Your Move, a fix first), expanding the collapsed stack it is in; nothing without a move
   */
  focusMoveRow: (step: 1 | -1) => void
}

/**
 * A key of the row runs the item of the branch menu it is the hint of (`getBranchContextMenuItems`), as a click on it
 * does: the same action, disabled while it is, the running one joins (`operationItem`); a disabled one says why in an
 * `info` toast, as its hover does
 */
function selectRowItem(name: string, id: string, setExpanded: (name: string, expanded: boolean) => void): void {
  const branch = getBranch(name)
  if (branch === undefined) {
    return
  }
  selectMenuItemById(getBranchContextMenuItems(branch, setExpanded, getOperations()), id, FROM_MENU, item => {
    if (item.description !== undefined) {
      showToast({ type: "info", title: item.description })
    }
  })
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
  selectRowItem(name, "review-changes", () => {})
}

/** The pointer rests on a branch row or it got the focus: its review is read ahead, if it can be reviewed */
export function prefetchBranchRowReview(name: string): void {
  const branch = getBranch(name)
  if (branch !== undefined && branchReviewBlocker(branch) === null) {
    prefetchBranchReview(name)
  }
}

/**
 * The keys of the focused branch row, for a key without a modifier pressed on the row (one of its buttons too) or on
 * the table: ↑/↓ move the focus between the branch rows (from the table to the first or the last), → expands the row,
 * ← collapses it or moves to the branch it is nested under, Enter on the row itself toggles it, P pushes it, S opens
 * its Safe Push popover, D opens its review window, O opens its pull request, M opens "Move to…" for it: P, D, O and M
 * as the items of the branch menu they are the hints of (`selectRowItem`). A handled key is `preventDefault`ed.
 */
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
      selectRowItem(name, "push", setExpanded)
    } else if (event.code === "KeyS") {
      event.preventDefault()
      openSafePush(name)
    } else if (event.code === "KeyO") {
      event.preventDefault()
      selectRowItem(name, "open-pull-request", setExpanded)
    } else if (event.code === "KeyM") {
      event.preventDefault()
      selectRowItem(name, "move-to", setExpanded)
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

  const focusMoveRow = (step: 1 | -1) => {
    const focused = document.activeElement?.closest<HTMLElement>("tr[data-testid=branch-row]")?.dataset.branchName
    const name = moveRow(getMoveRowsNow(unfoldedNames), focused ?? lastFocusedRef.current, step)
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

  return { handleRowKey, handleStackKey, focusFirstRow: () => focusRow(branchNames[0] ?? null), focusMoveRow }
}

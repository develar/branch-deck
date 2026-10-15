import type { KeyboardEvent, RefObject } from "react"
import { canPushNow } from "@/hooks/usePushButton"
import { neighbourRow, parentRow } from "@/lib/branch-row-keys"
import { pushOrConfirm } from "@/lib/git/push"
import { canStartRobot } from "@/lib/patronus"
import type { BranchTableRow } from "@/stores/branch-sync"
import { branchSyncStore, getBranch, isBranchPushing, isStackPushing } from "@/stores/branch-sync"
import { isProcessing, openInline } from "@/stores/inline-row-action"
import { getBranchRobot, getLandingAncestor, getLandingSpan, isPatronusAvailable } from "@/stores/patronus"

export interface BranchRowKeysOptions {
  tableRef: RefObject<HTMLTableElement | null>
  /** The rows of the table as shown (the filter applied) */
  rows: readonly BranchTableRow[]
  expandedKeys: ReadonlySet<string>
  setExpanded: (name: string, expanded: boolean) => void
  toggleExpanded: (name: string) => void
  /** The row Space falls back on while the table has the focus: the row the focus moves to */
  lastFocusedRef: RefObject<string | null>
}

export interface BranchRowKeys {
  /** For a key without a modifier on the table or one of its branch rows */
  handleRowKey: (event: KeyboardEvent<HTMLTableElement>) => void
  /** ↓ in the search of the filter bar */
  focusFirstRow: () => void
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

/**
 * The keys of the focused branch row, for a key without a modifier pressed on the row (one of its buttons too) or on
 * the table: ↑/↓ move the focus between the branch rows (from the table to the first or the last), → expands the row,
 * ← collapses it or moves to the branch it is nested under, Enter on the row itself toggles it, P pushes it, S opens
 * its Safe Push form. A handled key is `preventDefault`ed.
 */
export function useBranchRowKeys({ tableRef, rows, expandedKeys, setExpanded, toggleExpanded, lastFocusedRef }: BranchRowKeysOptions): BranchRowKeys {
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
  }

  return { handleRowKey, focusFirstRow: () => focusRow(branchNames[0] ?? null) }
}

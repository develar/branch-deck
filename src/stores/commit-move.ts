import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import type { Commit } from "@/lib/bindings"
import { selectBranchRange, toggleBranch } from "@/lib/branch-selection"

/** Source key of the unassigned commits list; branch lists use the branch name (never empty). */
export const UNASSIGNED_SOURCE = ""

/** Selection owner of the branch table: its rows are selected, not commits */
export const BRANCH_TABLE_SOURCE = "\u0000branches"

/** Commits being dragged: rows of one list, or whole branches of the branch table */
export interface CommitDrag {
  /** The lists the commits come from: one list, or the dragged branches */
  sourceKeys: readonly string[]
  /** The dragged branches; empty for a drag of commit rows */
  branches: readonly string[]
  commits: readonly Commit[]
}

/** Commits dropped on the "new branch" zone, waiting for a name; resolved by id on every render */
export interface NewBranchDrop {
  sourceKeys: readonly string[]
  commitIds: readonly string[]
}

/**
 * A branch to show: its row expands and scrolls into view, and its list selects the commits (just moved into it).
 * Without commits, the row consumes the request.
 */
export interface RevealRequest {
  branchName: string
  commitIds: readonly string[]
}

export interface CommitMoveState {
  /** The list that holds the selection, `BRANCH_TABLE_SOURCE` for selected branches; selecting elsewhere clears it */
  selectionOwner: string | null
  /** Selected branch rows, in the order they were selected */
  selectedBranches: readonly string[]
  /** The row a Shift-click selects a range from */
  branchAnchor: string | null
  /** The "Move to…" form of the selected branches is open */
  isBranchTargetOpen: boolean
  drag: CommitDrag | null
  newBranchDrop: NewBranchDrop | null
  reveal: RevealRequest | null
}

export const commitMoveStore = createStore<CommitMoveState>(() => ({
  selectionOwner: null,
  selectedBranches: [],
  branchAnchor: null,
  isBranchTargetOpen: false,
  drag: null,
  newBranchDrop: null,
  reveal: null,
}))

export function useCommitMove<T>(selector: (state: CommitMoveState) => T): T {
  return useStore(commitMoveStore, selector)
}

/** Whether a drag may drop on `targetKey`: not on the one list it comes from */
export function isDroppableOn(drag: CommitDrag, targetKey: string): boolean {
  return !(drag.sourceKeys.length === 1 && drag.sourceKeys[0] === targetKey)
}

/** The commits dragged out of a list other than `targetKey`, i.e. droppable on it; null otherwise. */
export function useDroppableDrag(targetKey: string): CommitDrag | null {
  return useStore(commitMoveStore, state => state.drag !== null && isDroppableOn(state.drag, targetKey) ? state.drag : null)
}

export function useRevealRequest(branchName: string): RevealRequest | null {
  return useStore(commitMoveStore, state => state.reveal?.branchName === branchName ? state.reveal : null)
}

export function claimSelection(sourceKey: string): void {
  if (commitMoveStore.getState().selectionOwner === sourceKey) {
    return
  }
  // a commit list taking the selection clears the selected branches
  commitMoveStore.setState(sourceKey === BRANCH_TABLE_SOURCE
    ? { selectionOwner: sourceKey }
    : { selectionOwner: sourceKey, selectedBranches: [], branchAnchor: null, isBranchTargetOpen: false })
}

export function useIsBranchSelected(name: string): boolean {
  return useStore(commitMoveStore, state => state.selectedBranches.includes(name))
}

export function useSelectedBranches(): readonly string[] {
  return useStore(commitMoveStore, state => state.selectedBranches)
}

export function getSelectedBranches(): readonly string[] {
  return commitMoveStore.getState().selectedBranches
}

/**
 * Selects branch rows: `toggle` (Cmd/Ctrl-click) adds or removes one, `range` (Shift-click) the rows from the anchor
 * to `name` in `order` (the branch rows of the table), `only` selects just `name`
 */
export function selectBranches(name: string, mode: "toggle" | "range" | "only", order: readonly string[]): void {
  const { selectedBranches, branchAnchor } = commitMoveStore.getState()
  let selected: readonly string[]
  if (mode === "toggle") {
    selected = toggleBranch(selectedBranches, name)
  }
  else if (mode === "range") {
    selected = selectBranchRange(order, branchAnchor ?? name, name)
  }
  else {
    selected = [name]
  }
  setSelectedBranches(selected, mode === "range" ? (branchAnchor ?? name) : name)
}

/** Selects exactly `selected`, e.g. every row (Cmd+A) or the branches that are still there after a sync */
export function setSelectedBranches(selected: readonly string[], anchor?: string | null): void {
  const state = commitMoveStore.getState()
  const isSame = selected.length === state.selectedBranches.length && selected.every((name, index) => state.selectedBranches[index] === name)
  if (isSame && (anchor === undefined || anchor === state.branchAnchor)) {
    return
  }
  commitMoveStore.setState({
    selectedBranches: selected,
    branchAnchor: selected.length === 0 ? null : (anchor === undefined ? state.branchAnchor : anchor),
    // the form acts on the selection
    isBranchTargetOpen: selected.length > 0 && state.isBranchTargetOpen,
  })
  if (selected.length > 0) {
    claimSelection(BRANCH_TABLE_SOURCE)
  }
}

export function clearBranchSelection(): void {
  setSelectedBranches([])
}

/** Opens or closes the "Move to…" form of the selected branches */
export function setBranchTargetOpen(isOpen: boolean): void {
  commitMoveStore.setState({ isBranchTargetOpen: isOpen && commitMoveStore.getState().selectedBranches.length > 0 })
}

export function startCommitDrag(drag: CommitDrag): void {
  commitMoveStore.setState({ drag })
}

export function endCommitDrag(): void {
  commitMoveStore.setState({ drag: null })
}

export function setNewBranchDrop(drop: NewBranchDrop | null): void {
  commitMoveStore.setState({ newBranchDrop: drop })
}

export function requestReveal(reveal: RevealRequest | null): void {
  commitMoveStore.setState({ reveal })
}

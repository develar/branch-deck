import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
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

/**
 * A form of commits a list opens for another window (the menu of a commit in the review window): "Move to…", the
 * squash dialog or the reword of the commit in its preview; the list of `branchName` consumes it once it has them
 */
export interface CommitFormRequest {
  branchName: string
  commitIds: readonly string[]
  form: "move" | "squash" | "reword"
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
  /** Where the pointer started the drag (client coordinates): the drag bar over it takes no drop until it left */
  dragOrigin: { readonly x: number; readonly y: number } | null
  /** Why the target under the pointer refuses the drag ("Cannot move to auth: …"), shown by the drag bar */
  hoverRefusal: string | null
  newBranchDrop: NewBranchDrop | null
  reveal: RevealRequest | null
  form: CommitFormRequest | null
}

export const commitMoveStore = createStore<CommitMoveState>(() => ({
  selectionOwner: null,
  selectedBranches: [],
  branchAnchor: null,
  isBranchTargetOpen: false,
  drag: null,
  dragOrigin: null,
  hoverRefusal: null,
  newBranchDrop: null,
  reveal: null,
  form: null,
}))

export function useCommitMove<T>(selector: (state: CommitMoveState) => T): T {
  return useStore(commitMoveStore, selector)
}

/** The drag that goes on, null otherwise */
export function getCommitDrag(): CommitDrag | null {
  return commitMoveStore.getState().drag
}

/** Whether a drag may drop on `targetKey`: not on the one list it comes from */
export function isDroppableOn(drag: CommitDrag, targetKey: string): boolean {
  return !(drag.sourceKeys.length === 1 && drag.sourceKeys[0] === targetKey)
}

/** The commits dragged out of a list other than `targetKey`, i.e. droppable on it; null otherwise. */
export function useDroppableDrag(targetKey: string): CommitDrag | null {
  return useStore(commitMoveStore, state => (state.drag !== null && isDroppableOn(state.drag, targetKey) ? state.drag : null))
}

export function useRevealRequest(branchName: string): RevealRequest | null {
  return useStore(commitMoveStore, state => (state.reveal?.branchName === branchName ? state.reveal : null))
}

export function useCommitFormRequest(branchName: string): CommitFormRequest | null {
  return useStore(commitMoveStore, state => (state.form?.branchName === branchName ? state.form : null))
}

export function claimSelection(sourceKey: string): void {
  if (commitMoveStore.getState().selectionOwner === sourceKey) {
    return
  }
  // a commit list taking the selection clears the selected branches
  commitMoveStore.setState(
    sourceKey === BRANCH_TABLE_SOURCE ? { selectionOwner: sourceKey } : { selectionOwner: sourceKey, selectedBranches: [], branchAnchor: null, isBranchTargetOpen: false },
  )
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
  } else if (mode === "range") {
    selected = selectBranchRange(order, branchAnchor ?? name, name)
  } else {
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
    branchAnchor: selected.length === 0 ? null : anchor === undefined ? state.branchAnchor : anchor,
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

/** A drag starts at `origin` (`beginCommitDrag`), its commits follow (`startCommitDrag`) */
export function setDragOrigin(origin: { x: number; y: number }): void {
  commitMoveStore.setState({ dragOrigin: origin })
}

export function startCommitDrag(drag: CommitDrag): void {
  commitMoveStore.setState({ drag, hoverRefusal: null })
}

export function endCommitDrag(): void {
  if (commitMoveStore.getState().drag !== null) {
    commitMoveStore.setState({ drag: null, dragOrigin: null, hoverRefusal: null })
  }
}

/** The refusal of the target under the pointer, or none (`clearHoverRefusal` clears only its own) */
export function setHoverRefusal(refusal: string): void {
  commitMoveStore.setState({ hoverRefusal: refusal })
}

export function clearHoverRefusal(refusal: string): void {
  if (commitMoveStore.getState().hoverRefusal === refusal) {
    commitMoveStore.setState({ hoverRefusal: null })
  }
}

export function setNewBranchDrop(drop: NewBranchDrop | null): void {
  commitMoveStore.setState({ newBranchDrop: drop })
}

export function requestReveal(reveal: RevealRequest | null): void {
  commitMoveStore.setState({ reveal })
}

/** Opens a form of commits of a branch in its list, which expands for it (`requestReveal`); null once consumed */
export function requestCommitForm(form: CommitFormRequest | null): void {
  commitMoveStore.setState(form === null ? { form } : { form, reveal: { branchName: form.branchName, commitIds: form.commitIds } })
}

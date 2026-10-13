import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import type { Commit } from "@/lib/bindings"

/** Source key of the unassigned commits list; branch lists use the branch name (never empty). */
export const UNASSIGNED_SOURCE = ""

/** Commits being dragged out of a list */
export interface CommitDrag {
  sourceKey: string
  commits: readonly Commit[]
}

/** Commits just moved into a branch: the branch row expands and scrolls into view, the commits are selected. */
export interface RevealRequest {
  branchName: string
  commitIds: readonly string[]
}

export interface CommitMoveState {
  /** The list that holds the commit selection; selecting in another list clears it */
  selectionOwner: string | null
  drag: CommitDrag | null
  reveal: RevealRequest | null
}

export const commitMoveStore = createStore<CommitMoveState>(() => ({
  selectionOwner: null,
  drag: null,
  reveal: null,
}))

export function useCommitMove<T>(selector: (state: CommitMoveState) => T): T {
  return useStore(commitMoveStore, selector)
}

/** The commits dragged out of a list other than `targetKey`, i.e. droppable on it; null otherwise. */
export function useDroppableDrag(targetKey: string): CommitDrag | null {
  return useStore(commitMoveStore, state => state.drag !== null && state.drag.sourceKey !== targetKey ? state.drag : null)
}

export function useRevealRequest(branchName: string): RevealRequest | null {
  return useStore(commitMoveStore, state => state.reveal?.branchName === branchName ? state.reveal : null)
}

export function claimSelection(sourceKey: string): void {
  if (commitMoveStore.getState().selectionOwner !== sourceKey) {
    commitMoveStore.setState({ selectionOwner: sourceKey })
  }
}

export function startCommitDrag(drag: CommitDrag): void {
  commitMoveStore.setState({ drag })
}

export function endCommitDrag(): void {
  commitMoveStore.setState({ drag: null })
}

export function requestReveal(reveal: RevealRequest | null): void {
  commitMoveStore.setState({ reveal })
}

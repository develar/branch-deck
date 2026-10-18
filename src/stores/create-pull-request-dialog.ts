import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"

interface CreatePullRequestDialogState {
  /** The branch whose dialog the window shows, or the root of the whole stack (`wholeStack`); null while it is closed */
  readonly branchName: string | null
  /** The dialog of the pull request of the whole stack of the root `branchName` (its stack branch) */
  readonly wholeStack: boolean
  /** Counts the openings: each starts with a new text */
  readonly generation: number
}

/**
 * The "Create pull request" dialog of the window: "Create Pull Request…" of the branch menu, the push toast, the branch
 * preview, the Safe Push form and the review window open it; "Create Pull Request of the Stack…" of the stack's actions
 * opens it for the whole stack.
 */
export const createPullRequestDialogStore = createStore<CreatePullRequestDialogState>(() => ({ branchName: null, wholeStack: false, generation: 0 }))

export function useCreatePullRequestBranch(): string | null {
  return useStore(createPullRequestDialogStore, state => state.branchName)
}

/** The open dialog is of the whole stack of the root `useCreatePullRequestBranch` names */
export function useCreatePullRequestWholeStack(): boolean {
  return useStore(createPullRequestDialogStore, state => state.wholeStack)
}

export function useCreatePullRequestGeneration(): number {
  return useStore(createPullRequestDialogStore, state => state.generation)
}

export function openCreatePullRequestDialog(branchName: string): void {
  createPullRequestDialogStore.setState(state => ({ branchName, wholeStack: false, generation: state.generation + 1 }))
}

/** The dialog of the pull request of the whole stack of `root`: its stack branch into the baseline */
export function openStackPullRequestDialog(root: string): void {
  createPullRequestDialogStore.setState(state => ({ branchName: root, wholeStack: true, generation: state.generation + 1 }))
}

export function closeCreatePullRequestDialog(): void {
  createPullRequestDialogStore.setState({ branchName: null })
}

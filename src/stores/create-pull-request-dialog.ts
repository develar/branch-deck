import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"

interface CreatePullRequestDialogState {
  /** The branch whose dialog the window shows; null while it is closed */
  readonly branchName: string | null
  /** Counts the openings: each starts with a new text */
  readonly generation: number
}

/**
 * The "Create pull request" dialog of the window: "Create Pull Request…" of the branch menu, the push toast, the branch
 * preview, the Safe Push form and the review window open it.
 */
export const createPullRequestDialogStore = createStore<CreatePullRequestDialogState>(() => ({ branchName: null, generation: 0 }))

export function useCreatePullRequestBranch(): string | null {
  return useStore(createPullRequestDialogStore, state => state.branchName)
}

export function useCreatePullRequestGeneration(): number {
  return useStore(createPullRequestDialogStore, state => state.generation)
}

export function openCreatePullRequestDialog(branchName: string): void {
  createPullRequestDialogStore.setState(state => ({ branchName, generation: state.generation + 1 }))
}

export function closeCreatePullRequestDialog(): void {
  createPullRequestDialogStore.setState({ branchName: null })
}

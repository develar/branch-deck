import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { getSelectedProject } from "@/stores/repository"
import { repositorySettingsStore } from "@/stores/settings/repositorySettings"

interface BranchPreviewState {
  /** The repository the preview is of: another one selected closes it */
  readonly repositoryPath: string | null
  /** The shown branch, null while closed; the preview follows it by name across a sync */
  readonly name: string | null
}

/** The branch preview dialog of the branch table, opened by Space on a row; the review of a branch is its window */
export const branchPreviewStore = createStore<BranchPreviewState>(() => ({ repositoryPath: null, name: null }))

// a preview belongs to its repository: another one selected closes it, and switching back doesn't bring it again
repositorySettingsStore.subscribe(() => {
  const { repositoryPath, name } = branchPreviewStore.getState()
  if (name !== null && getSelectedProject()?.path !== repositoryPath) {
    closeBranchPreview()
  }
})

export function useBranchPreview(): BranchPreviewState {
  return useStore(branchPreviewStore)
}

/** Shows the preview of `name` */
export function openBranchPreview(name: string): void {
  branchPreviewStore.setState({ repositoryPath: getSelectedProject()?.path ?? null, name })
}

export function showBranchInPreview(name: string): void {
  branchPreviewStore.setState({ name })
}

export function closeBranchPreview(): void {
  branchPreviewStore.setState({ name: null })
}

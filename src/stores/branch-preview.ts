import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { BranchReviewFocus } from "@/lib/branch-review"
import { getSelectedProject } from "@/stores/repository"
import { repositorySettingsStore } from "@/stores/settings/repositorySettings"

/** The review of the branch in the preview dialog: where it starts, and whether Escape goes back to the preview */
export interface BranchPreviewReview {
  readonly focus: BranchReviewFocus
  /** D in the preview opened it: Escape goes back there; from a row, a menu or a form Escape closes the dialog */
  readonly fromPreview: boolean
  /** A new value per open: the same branch reviewed again starts over */
  readonly token: number
}

interface BranchPreviewState {
  /** The repository the preview is of: another one selected closes it */
  readonly repositoryPath: string | null
  /** The shown branch, null while closed; the preview follows it by name across a sync */
  readonly name: string | null
  /** The review of the branch; null for its preview */
  readonly review: BranchPreviewReview | null
}

/**
 * The branch preview dialog of the branch table and its review mode: opened by Space on a row (the preview), by D on a
 * row, "Review Changes" of the menu and the code-owner block of the Safe Push form (the review), and by D in the preview
 */
export const branchPreviewStore = createStore<BranchPreviewState>(() => ({ repositoryPath: null, name: null, review: null }))

// a preview belongs to its repository: another one selected closes it, and switching back doesn't bring it again
repositorySettingsStore.subscribe(() => {
  const { repositoryPath, name } = branchPreviewStore.getState()
  if (name !== null && getSelectedProject()?.path !== repositoryPath) {
    closeBranchPreview()
  }
})

let nextToken = 1

export function useBranchPreview(): BranchPreviewState {
  return useStore(branchPreviewStore)
}

/** Shows the preview of `name`; another branch of the preview (↑/↓) keeps the mode */
export function openBranchPreview(name: string): void {
  branchPreviewStore.setState({ repositoryPath: getSelectedProject()?.path ?? null, name, review: null })
}

export function showBranchInPreview(name: string): void {
  branchPreviewStore.setState({ name })
}

/** Reviews `name` in the dialog at `focus` */
export function openBranchReviewDialog(name: string, focus: BranchReviewFocus, fromPreview: boolean): void {
  branchPreviewStore.setState({ repositoryPath: getSelectedProject()?.path ?? null, name, review: { focus, fromPreview, token: nextToken++ } })
}

/** Back from the review to the preview of the branch */
export function showBranchPreview(): void {
  branchPreviewStore.setState({ review: null })
}

export function closeBranchPreview(): void {
  branchPreviewStore.setState({ name: null, review: null })
}

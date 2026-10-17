import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"

/**
 * The branch whose Request Review… popover is open from outside its trigger: the item of the branch menu opens the one
 * of the row's next step (`NextStepSlot`), which mounts it around its button or, for another step, an anchor
 */
export const requestReviewPopoverStore = createStore<{ readonly branchName: string | null }>(() => ({ branchName: null }))

/** Whether the Request Review… popover of the branch is open */
export function useRequestReviewOpen(branchName: string): boolean {
  return useStore(requestReviewPopoverStore, state => state.branchName === branchName)
}

/** Opens the Request Review… popover of the branch, after the menu that asks for it closed */
export function openRequestReview(branchName: string): void {
  requestAnimationFrame(() => requestReviewPopoverStore.setState({ branchName }))
}

export function setRequestReviewOpen(branchName: string, open: boolean): void {
  requestReviewPopoverStore.setState(state => (open ? { branchName } : state.branchName === branchName ? { branchName: null } : state))
}

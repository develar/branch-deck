import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { PushBranchOptions } from "@/lib/push-run"

/** The push button that shows a confirm: the one of the branch row, or the one in the footer of the branch preview */
export type PushConfirmTarget = "row" | "preview"

/**
 * A push whose plan asks first (`Confirmation::ForcePushDrops` of `prepare` of the push: it drops remote commits the
 * branch lacks), from its button, the menu, P in the preview or the action of the remote-moved toast: the push button of
 * `target` opens its description with the dropped commits and the Force Push that pushes them over (`force`).
 */
export interface PushConfirmRequest {
  branchName: string
  target: PushConfirmTarget
  /** The options of that push: the remote head it leases on, the Safe Push it restarts */
  options: PushBranchOptions
}

interface PushConfirmState {
  request: PushConfirmRequest | null
}

export const pushConfirmStore = createStore<PushConfirmState>(() => ({ request: null }))

/** Opens the confirm of a push in the push button of `target`; a newer request replaces an unanswered one */
export function requestPushConfirm(branchName: string, target: PushConfirmTarget, options: PushBranchOptions = {}): void {
  pushConfirmStore.setState({ request: { branchName, target, options } })
}

/** The request is answered or dismissed; a newer one stays */
export function clearPushConfirm(request: PushConfirmRequest): void {
  pushConfirmStore.setState(state => (state.request === request ? { request: null } : state))
}

/** The unanswered request for the push button of this branch at `target`; none without a target */
export function usePushConfirmRequest(branchName: string, target: PushConfirmTarget | undefined): PushConfirmRequest | null {
  return useStore(pushConfirmStore, state => {
    const request = state.request
    return request !== null && target !== undefined && request.branchName === branchName && request.target === target ? request : null
  })
}

import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import type { PatronusRobot } from "@/lib/bindings"

/**
 * What the user chose when a push would abort the running Safe Push of the branch: cancel it, push and restart it on the
 * pushed commit; cancel it and push (the push is for a new robot, which replaces it); push anyway; or go back
 */
export type PushGuardAnswer = "cancel-push-restart" | "cancel-push" | "push" | "back"

export interface PushGuardRequest {
  readonly branchName: string
  readonly robot: PatronusRobot
  /**
   * The push is for a Safe Push started right after it (the Safe Push form): the running one is cancelled, not
   * restarted, or the branch would run twice
   */
  readonly startsRobot: boolean
}

interface PushGuardState {
  /** The question shown, null when none */
  readonly request: PushGuardRequest | null
}

export const pushGuardStore = createStore<PushGuardState>(() => ({ request: null }))

let resolveRequest: ((answer: PushGuardAnswer) => void) | null = null

export function usePushGuardRequest(): PushGuardRequest | null {
  return useStore(pushGuardStore, state => state.request)
}

/** Asks the user (the push guard dialog); a question still open is answered "back" */
export function askPushGuard(request: PushGuardRequest): Promise<PushGuardAnswer> {
  resolveRequest?.("back")
  return new Promise((resolve) => {
    resolveRequest = resolve
    pushGuardStore.setState({ request })
  })
}

export function answerPushGuard(answer: PushGuardAnswer): void {
  const resolve = resolveRequest
  resolveRequest = null
  pushGuardStore.setState({ request: null })
  resolve?.(answer)
}

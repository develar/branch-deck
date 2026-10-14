import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import type { PatronusRobot } from "@/lib/bindings"

/** What the user chose when a push would abort the running Safe Push of the branch */
export type PushGuardAnswer = "cancel-push-restart" | "push" | "back"

export interface PushGuardRequest {
  readonly branchName: string
  readonly robot: PatronusRobot
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
export function askPushGuard(branchName: string, robot: PatronusRobot): Promise<PushGuardAnswer> {
  resolveRequest?.("back")
  return new Promise((resolve) => {
    resolveRequest = resolve
    pushGuardStore.setState({ request: { branchName, robot } })
  })
}

export function answerPushGuard(answer: PushGuardAnswer): void {
  const resolve = resolveRequest
  resolveRequest = null
  pushGuardStore.setState({ request: null })
  resolve?.(answer)
}

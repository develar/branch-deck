import { useState } from "react"
import { emit, listen } from "@tauri-apps/api/event"
import { useTauriListen } from "@/hooks/useTauriListen"
import { logError, notifyError } from "@/lib/errors"
import { windowLinkEvents } from "@/lib/sub-window-link"
import type { LinkedWindowReply, LinkedWindowRequest } from "@/lib/sub-window-link"

/*
 * The sub-window's side of its link with the main window (`@/lib/sub-window-link`): the state the main window sends,
 * the requests it runs with their answers, the toasts and errors of those requests.
 */

/** The answers the window waits for, by the id of its request */
const replies = new Map<string, (ok: boolean) => void>()
/** The labels whose answers are listened to: once per window, for its lifetime */
const listenedLabels = new Set<string>()
let nextRequestId = 0

function listenForReplies(label: string): Promise<void> {
  if (listenedLabels.has(label)) {
    return Promise.resolve()
  }
  listenedLabels.add(label)
  return listen<LinkedWindowReply>(windowLinkEvents(label).reply, ({ payload }) => {
    replies.get(payload.id)?.(payload.ok)
    replies.delete(payload.id)
  }).then(
    () => undefined,
    (error: unknown) => {
      listenedLabels.delete(label)
      logError(`Cannot listen for the answers to ${label}`, error)
    },
  )
}

export type DistributiveOmit<T, K extends PropertyKey> = T extends unknown ? Omit<T, K> : never

/**
 * Asks the main window; resolves to whether it did it (a refusal was told by its feedback). No timeout: a write such
 * as a Safe Push may take a minute.
 */
export function askMainWindow<Request extends LinkedWindowRequest>(label: string, request: DistributiveOmit<Request, "id">): Promise<boolean> {
  const id = `${Date.now()}-${nextRequestId++}`
  return listenForReplies(label).then(
    () =>
      new Promise<boolean>(resolve => {
        replies.set(id, resolve)
        emit(windowLinkEvents(label).request, { ...request, id }).catch((error: unknown) => {
          replies.delete(id)
          notifyError("Cannot reach the main window", error)
          resolve(false)
        })
      }),
  )
}

/** The state the main window sends for the window's repository, from the one the window opened with */
export function useLinkedState<State extends { readonly repositoryPath: string }>(label: string, initial: State): State {
  const [state, setState] = useState(initial)
  useTauriListen<State>(windowLinkEvents(label).state, payload => {
    if (payload.repositoryPath === initial.repositoryPath) {
      setState(payload)
    }
  })
  return state
}

/** Shows the toasts and the errors of the requests the main window ran for the window */
export function useLinkedFeedback<Feedback>(label: string, show: (feedback: Feedback) => void): void {
  useTauriListen<Feedback>(windowLinkEvents(label).feedback, show)
}

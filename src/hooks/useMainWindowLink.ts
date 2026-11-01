import { useState } from "react"
import { emit, listen } from "@tauri-apps/api/event"
import { useTauriListen } from "@/hooks/useTauriListen"
import { logError, notifyError } from "@/lib/errors"
import { windowLinkEvents } from "@/lib/sub-window-link"
import type { LinkRequestEvents, LinkedWindowReply, LinkedWindowRequest } from "@/lib/sub-window-link"

/*
 * The sub-window's side of its link with the main window (`@/lib/sub-window-link`): the state the main window sends,
 * the requests it runs with their answers, the toasts and errors of those requests.
 */

/** The answers the window waits for, by the id of its request */
const replies = new Map<string, (ok: boolean) => void>()
/** The events of the answers listened to: once per window, for its lifetime */
const listenedReplies = new Set<string>()
let nextRequestId = 0

function listenForReplies(replyEvent: string): Promise<void> {
  if (listenedReplies.has(replyEvent)) {
    return Promise.resolve()
  }
  listenedReplies.add(replyEvent)
  return listen<LinkedWindowReply>(replyEvent, ({ payload }) => {
    replies.get(payload.id)?.(payload.ok)
    replies.delete(payload.id)
  }).then(
    () => undefined,
    (error: unknown) => {
      listenedReplies.delete(replyEvent)
      logError(`Cannot listen for the answers of ${replyEvent}`, error)
    },
  )
}

export type DistributiveOmit<T, K extends PropertyKey> = T extends unknown ? Omit<T, K> : never

/**
 * Asks the main window; resolves to whether it did it (a refusal was told by its feedback). No timeout: a write such
 * as a Safe Push may take a minute.
 */
export function askMainWindow<Request extends LinkedWindowRequest>(label: string, request: DistributiveOmit<Request, "id">): Promise<boolean> {
  return askOn(windowLinkEvents(label), request)
}

/** `askMainWindow` on the events of a protocol of its own (the runs of the review window, `review/run`) */
export function askOn<Request extends LinkedWindowRequest>(events: Pick<LinkRequestEvents, "request" | "reply">, request: DistributiveOmit<Request, "id">): Promise<boolean> {
  const id = `${Date.now()}-${nextRequestId++}`
  return listenForReplies(events.reply).then(
    () =>
      new Promise<boolean>(resolve => {
        replies.set(id, resolve)
        emit(events.request, { ...request, id }).catch((error: unknown) => {
          replies.delete(id)
          notifyError("Cannot reach the main window", error)
          resolve(false)
        })
      }),
  )
}

/**
 * The state the main window sends for the window's repository, from the one the window opened with; with
 * `anyRepository` the state of whichever repository the main window shows (a window that follows its selection)
 */
export function useLinkedState<State extends { readonly repositoryPath: string }>(label: string, initial: State, { anyRepository = false } = {}): State {
  const [state, setState] = useState(initial)
  useTauriListen<State>(windowLinkEvents(label).state, payload => {
    if (anyRepository || payload.repositoryPath === initial.repositoryPath) {
      setState(payload)
    }
  })
  return state
}

/** Shows the toasts and the errors of the requests the main window ran for the window */
export function useLinkedFeedback<Feedback>(label: string, show: (feedback: Feedback) => void): void {
  useTauriListen<Feedback>(windowLinkEvents(label).feedback, show)
}

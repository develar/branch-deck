import { emit, listen } from "@tauri-apps/api/event"
import type { UnlistenFn } from "@tauri-apps/api/event"
import { logError } from "@/lib/errors"
import { openSubWindow } from "@/lib/window-management"

/*
 * The link between the main window and a sub-window of one kind (one label, one window reused for the next object): the
 * main window opens it and knows it is open until it is destroyed, tells it its state when that changed, and runs what
 * it asks for, with an answer and a toast or an error shown where the user is. The window side is in
 * `@/hooks/useMainWindowLink`. Event names are prefixed with the label: two kinds of window never hear each other.
 */

/** The events of the link of `label` */
export function windowLinkEvents(label: string) {
  return {
    /** main → window: the state of the window's object, when it changed */
    state: `${label}/state`,
    /** window → main: a write or a notice, with an id */
    request: `${label}/request`,
    /** main → window: whether a request was done, by its id */
    reply: `${label}/reply`,
    /** main → window: a toast or an error of a request, shown in the window (the main window recorded it in Activity) */
    feedback: `${label}/feedback`,
  } as const
}

/** What the window shows: at least its repository, which its diffs read and every event is filtered by */
export interface LinkedWindowShown {
  readonly repositoryPath: string
}

/** A request of the window */
export interface LinkedWindowRequest {
  readonly id: string
  readonly repositoryPath: string
}

export interface LinkedWindowReply {
  readonly id: string
  /** The request was done; a refused one was told by its feedback */
  readonly ok: boolean
}

export interface WindowLinkOptions<Shown extends LinkedWindowShown, State> {
  readonly label: string
  /** The state the window shows of `shown`, sent whenever it changed (`broadcast`) */
  readonly buildState?: (shown: Shown) => State
  /** The window was closed (destroyed) */
  readonly onClose?: (shown: Shown) => void
}

export interface WindowLink<Shown extends LinkedWindowShown, State> {
  readonly label: string
  /** What the open window shows; null while it is closed */
  readonly shown: () => Shown | null
  /** The window shows something else now (it moved to another object itself) */
  readonly setShown: (shown: Shown) => void
  /** The window is open on an object of `repositoryPath`: the opens of its kind go there */
  readonly isOpenFor: (repositoryPath: string) => boolean
  /** The state of what the window shows, recorded as sent: for the data of an open */
  readonly takeState: () => State | undefined
  /**
   * Opens the window on `shown` (or shows it in the open one, which starts over with `data`); a failed open forgets it
   * and rejects
   */
  readonly open: (shown: Shown, window: { url: string; title: string; data: object }) => Promise<void>
  /** Sends the state when it changed since it was sent; nothing while the window is closed */
  readonly broadcast: () => void
  /**
   * The handler of the window's requests: runs each, then answers it. `feedback` sends a toast or an error to the
   * window.
   */
  readonly serve: <Request extends LinkedWindowRequest, Feedback>(
    run: (request: Request, feedback: (feedback: Feedback) => void) => Promise<boolean> | boolean,
  ) => (request: Request) => void
  /** Forgets the window (tests) */
  readonly reset: () => void
}

export function createWindowLink<Shown extends LinkedWindowShown, State = never>(options: WindowLinkOptions<Shown, State>): WindowLink<Shown, State> {
  const { label, buildState, onClose } = options
  const events = windowLinkEvents(label)
  let shown: Shown | null = null
  /** The state last sent, as JSON: a change of nothing sends nothing */
  let sentState: string | null = null
  /** Removes the listener of the close; null while nothing listens */
  let unlistenClose: UnlistenFn | null = null
  let listening = false

  const forget = () => {
    shown = null
    sentState = null
  }

  // once per open of a closed window: the close removes it, the next open listens again
  const listenForClose = () => {
    listening = true
    listen(
      "tauri://destroyed",
      () => {
        const closed = shown
        forget()
        listening = false
        unlistenClose?.()
        unlistenClose = null
        if (closed !== null) {
          onClose?.(closed)
        }
      },
      { target: { kind: "WebviewWindow", label } },
    )
      .then(unlisten => {
        if (listening) {
          unlistenClose = unlisten
        } else {
          unlisten()
        }
      })
      .catch((error: unknown) => {
        listening = false
        logError(`Cannot listen for the close of ${label}`, error)
      })
  }

  const takeState = (): State | undefined => {
    if (shown === null || buildState === undefined) {
      return undefined
    }
    const state = buildState(shown)
    sentState = JSON.stringify(state)
    return state
  }

  return {
    label,
    shown: () => shown,
    setShown: next => {
      shown = next
    },
    isOpenFor: repositoryPath => shown?.repositoryPath === repositoryPath,
    takeState,
    open: async (next, window) => {
      shown = next
      try {
        await openSubWindow({ windowId: label, url: window.url, title: window.title, data: window.data })
      } catch (error) {
        forget()
        throw error
      }
      if (!listening) {
        listenForClose()
      }
    },
    broadcast: () => {
      if (shown === null || buildState === undefined) {
        return
      }
      const state = buildState(shown)
      const json = JSON.stringify(state)
      if (json === sentState) {
        return
      }
      sentState = json
      emit(events.state, state).catch((error: unknown) => logError(`Cannot tell ${label} its state`, error))
    },
    serve: run => request => {
      const feedback = (value: unknown) => emit(events.feedback, value).catch((error: unknown) => logError(`Cannot tell ${label} how it went`, error))
      const reply = (ok: boolean) => {
        const answer: LinkedWindowReply = { id: request.id, ok }
        emit(events.reply, answer).catch((error: unknown) => logError(`Cannot answer ${label}`, error))
      }
      Promise.resolve()
        .then(() => run(request, value => void feedback(value)))
        .then(reply, (error: unknown) => {
          logError(`Cannot run the request of ${label}`, error)
          reply(false)
        })
    },
    reset: () => {
      forget()
      unlistenClose?.()
      unlistenClose = null
      listening = false
    },
  }
}

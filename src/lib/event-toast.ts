import type { ReactNode } from "react"
import { toast } from "@/components/ui/toast"
import { ACTION_TOAST_TIMEOUT_MS, TOAST_TIMEOUT_MS } from "@/lib/toast-message"

/*
 * One toast per event of an object (docs/contributing/ux-writing.md#one-toast-per-event): a toast is keyed by what it
 * tells, the later states of the event replace it in place, and its actions are always those of the current state.
 * The robot that merged auth, then the update that landed it, is one toast that says "auth merged into master" with
 * Update Project, then "Updating…", then "auth landed in origin/master": never two toasts of one landing, never an
 * Update Project after the update.
 */

/** The objects and events the toasts tell, one key each: a later state names the toast of an earlier one by it */
export const toastKeys = {
  /** A Patronus robot: started, restarted, canceling, failing, failed, merged */
  robot: (repositoryPath: string, robotId: string) => `robot\n${repositoryPath}\n${robotId}`,
  /**
   * The robots of a subject (a branch, a whole stack, a robot of no branch) in one push mode: a restart or a new start
   * is a newer robot of it, which replaces the toast of the older one
   */
  robotSubject: (repositoryPath: string, subject: string, pushMode: string) => `robot-subject\n${repositoryPath}\n${subject}\n${pushMode}`,
  /** A merge into the baseline of the repository that its update brings in: the outcome of the update replaces it */
  merged: (repositoryPath: string) => `merged\n${repositoryPath}`,
  /** An update of the repository: applied (with what it landed), or up to date */
  update: (repositoryPath: string) => `update\n${repositoryPath}`,
  /** A push of a branch, or one the remote refused because it moved */
  push: (repositoryPath: string, branchName: string) => `push\n${repositoryPath}\n${branchName}`,
  /** The approvals of a subject (a branch, a whole stack) that waited for them: its start replaces the toast */
  approval: (repositoryPath: string, subject: string) => `approval\n${repositoryPath}\n${subject}`,
  /** A pull request of someone else: the request of the user's review, the review, the Safe Push of it */
  review: (repositoryPath: string, pullRequest: number) => `review\n${repositoryPath}\n${pullRequest}`,
  /** The requests of several pull requests one answer told: one toast of them */
  reviewRequests: (repositoryPath: string, pullRequests: readonly number[]) => `review-requests\n${repositoryPath}\n${pullRequests.join(" ")}`,
  /** The auto-merge of a pull request of the user: enabled, then merged by GitHub (`NewsEvent::AutoMerged`) */
  autoMerge: (repositoryPath: string, pullRequest: number) => `auto-merge\n${repositoryPath}\n${pullRequest}`,
  /** A YouTrack issue: marked Fixed, back to its state */
  issue: (key: string) => `issue\n${key}`,
} as const

/** An action of a toast; `running` while the operation it starts runs, from any surface: disabled, its toast stays */
export type EventToastAction = { readonly label: string; readonly onClick: () => void; readonly running?: undefined } | { readonly label: string; readonly running: true }

/** What the toast says in the current state of its event */
export interface EventToastState {
  readonly type: "success" | "info" | "warning" | "error"
  readonly title: string
  readonly description?: ReactNode
  /** The primary action */
  readonly action?: EventToastAction
  /** The action after the primary one, below the text ("Open in Patronus") */
  readonly secondaryAction?: EventToastAction
  /** ms; by default `ACTION_TOAST_TIMEOUT_MS` with an action, `TOAST_TIMEOUT_MS` without; 0 stays until closed */
  readonly timeout?: number
}

/** A store the state of a toast reads: its changes render the toast again */
export interface ToastStateSource {
  subscribe(listener: () => void): () => void
}

export interface EventToast {
  /** The object and its event (`toastKeys`): one open toast per key */
  readonly key: string
  /** More keys the toast answers to: a later state of any of them replaces it ("the update that brings this merge in") */
  readonly aliases?: readonly string[]
  /** The keys of the earlier states this one replaces: the toast of the first in place, the others close */
  readonly supersedes?: readonly string[]
  /**
   * The state as the stores have it now: at the show, and again on every change of `watch`; null when the object is
   * gone (a branch deleted, another repository shown, a run that ended without an outcome of its own), which closes it
   */
  readonly render: () => EventToastState | null
  readonly watch?: readonly ToastStateSource[]
  /**
   * Once a state showed a running action, the first state without one closes the toast: the run ended, and its outcome
   * says so itself (a toast that replaced this one as a later state of its event, a dialog, an error toast)
   */
  readonly closesAfterRun?: boolean
}

interface Entry {
  readonly id: string
  readonly event: EventToast
  readonly keys: ReadonlySet<string>
  readonly order: number
  /** The state shown: a click runs the action of the latest render */
  state: EventToastState
  /** A state of it showed a running action */
  ran: boolean
  signature: string
  unwatch: () => void
}

/** The open toasts of events, by toast id */
const entries = new Map<string, Entry>()
let sequence = 0

/**
 * Shows the toast of an event: the open toast of its key, or of a key it supersedes, takes the new state in place (the
 * newest of them, the others close), else a new toast shows. Returns the id of the toast, null when its state is none.
 */
export function showEventToast(event: EventToast): string | null {
  return place(event, false)
}

/**
 * A later state of an event that is no news of its own (a landing a sync found after an update elsewhere): replaces
 * the open toast of one of its keys, and shows nothing when none is open. Returns the id of the toast it replaced.
 */
export function replaceEventToast(event: EventToast): string | null {
  return place(event, true)
}

/** Closes the open toasts of the key */
export function closeEventToasts(key: string): void {
  for (const entry of entriesOf([key])) {
    dismiss(entry)
  }
}

/** Whether a toast of the key is open */
export function isEventToastOpen(key: string): boolean {
  return entriesOf([key]).length > 0
}

/** Closes every toast of an event and forgets them (tests) */
export function resetEventToasts(): void {
  // a deleted entry of a Map is skipped by its iteration
  for (const entry of entries.values()) {
    dismiss(entry)
  }
  sequence = 0
}

function entriesOf(keys: readonly string[]): Entry[] {
  return [...entries.values()].filter(entry => keys.some(key => entry.keys.has(key)))
}

function place(event: EventToast, replaceOnly: boolean): string | null {
  // the newest first: it is the one in front
  const found = entriesOf([event.key, ...(event.supersedes ?? [])]).sort((a, b) => b.order - a.order)
  if (replaceOnly && found.length === 0) {
    return null
  }
  const [kept, ...superseded] = found
  for (const entry of superseded) {
    dismiss(entry)
  }
  const state = event.render()
  if (state === null) {
    if (kept !== undefined) {
      dismiss(kept)
    }
    return null
  }
  if (kept !== undefined) {
    kept.unwatch()
    entries.delete(kept.id)
  }
  const entry: Entry = {
    id: kept?.id ?? `event-toast-${++sequence}`,
    event,
    keys: new Set([event.key, ...(event.aliases ?? [])]),
    order: ++sequence,
    state,
    ran: isRunning(state),
    signature: signatureOf(state),
    unwatch: () => {},
  }
  entries.set(entry.id, entry)
  // an open id takes the new state in place, its timer started again
  toast.add({ id: entry.id, ...optionsOf(entry), onClose: () => forget(entry) })
  const unsubscribes = (event.watch ?? []).map(source => source.subscribe(() => renderAgain(entry)))
  entry.unwatch = () => {
    for (const unsubscribe of unsubscribes) {
      unsubscribe()
    }
  }
  return entry.id
}

/** A store the toast reads changed: its state again, updated in place when it differs, closed when it is none */
function renderAgain(entry: Entry): void {
  if (entries.get(entry.id) !== entry) {
    return
  }
  const state = entry.event.render()
  if (state === null || (entry.event.closesAfterRun === true && entry.ran && !isRunning(state))) {
    dismiss(entry)
    return
  }
  entry.state = state
  entry.ran ||= isRunning(state)
  const signature = signatureOf(state)
  if (signature !== entry.signature) {
    entry.signature = signature
    // the timer runs on unless the timeout changed: a run that ended gives the toast its whole time again
    toast.update(entry.id, optionsOf(entry))
  }
}

function dismiss(entry: Entry): void {
  forget(entry)
  toast.close(entry.id)
}

function forget(entry: Entry): void {
  if (entries.get(entry.id) === entry) {
    entry.unwatch()
    entries.delete(entry.id)
  }
}

function isRunning(state: EventToastState): boolean {
  return state.action?.running === true || state.secondaryAction?.running === true
}

function timeoutOf(state: EventToastState): number {
  if (isRunning(state)) {
    return 0
  }
  return state.timeout ?? (state.action !== undefined || state.secondaryAction !== undefined ? ACTION_TOAST_TIMEOUT_MS : TOAST_TIMEOUT_MS)
}

/**
 * The options of the toast manager for the state of the entry; every field set, so an update in place drops what the
 * state no longer has. A click runs the action of the latest render, which may differ from the one shown in its
 * object only
 */
function optionsOf(entry: Entry) {
  const { state } = entry
  const { action, secondaryAction } = state
  return {
    type: state.type,
    title: state.title,
    description: state.description,
    timeout: timeoutOf(state),
    actionProps: action === undefined ? undefined : action.running === true ? { children: action.label } : { children: action.label, onClick: () => runAction(entry.state.action) },
    data: {
      secondaryAction:
        secondaryAction === undefined
          ? undefined
          : { label: secondaryAction.label, onClick: () => runAction(entry.state.secondaryAction), disabled: secondaryAction.running === true },
      running: action?.running === true,
    },
  }
}

function runAction(action: EventToastAction | undefined): void {
  if (action !== undefined && action.running !== true) {
    action.onClick()
  }
}

/** The ids of the node descriptions, so the same node is the same description */
const nodeIds = new WeakMap<object, number>()

/** What the user sees of a state: a render that says the same keeps the toast as it is */
function signatureOf(state: EventToastState): string {
  const { description } = state
  let shownDescription: unknown = description
  if (typeof description === "object" && description !== null) {
    shownDescription = nodeIds.get(description) ?? nodeIds.set(description, ++sequence).get(description)
  }
  const { action, secondaryAction } = state
  return JSON.stringify([state.type, state.title, shownDescription, action?.label, action?.running, secondaryAction?.label, secondaryAction?.running, timeoutOf(state)])
}

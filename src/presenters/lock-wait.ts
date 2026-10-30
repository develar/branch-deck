// The page's words of a holder of the repository lock (`LockHolderInfo`, as the record beside the lock names it,
// `repository_lock.rs`): whom an operation waits for (`OperationStep.waitingForLock`, every window reads it in the view),
// its Stop Waiting, the toast of a wait so stopped, and who holds the lock a command was refused by
// (`ErrorDetails.locked`, `describeError`); and of the holder of what an operation of the page needs, an operation of
// another process as its record in the operations registry of the repository names it (`OperationHolder`,
// `operations::registry_file`), which made it `busy` (`ErrorDetails.busy`, `describeError`). Every window words its
// errors by them (`src/lib/errors.ts`): the module imports no other presenter.
import type { HolderTool, LockHolderInfo, OperationHolder, OperationId, OperationKind, RunningOperation } from "@/lib/bindings"
import type { MenuItem } from "@/lib/menu"
import type { ToastMessage } from "@/lib/toast-message"

/**
 * How long a holder has held the lock, or run an operation, as `LockHolder` of `repository_lock.rs` says it: "40 s",
 * "12 min 3 s"
 */
function formatHeldFor(seconds: number): string {
  return seconds >= 60 ? `${Math.floor(seconds / 60)} min ${seconds % 60} s` : `${seconds} s`
}

/** Who holds the repository: `bd` by its command line, the app by its name */
function holderName(holder: LockHolderInfo): string {
  return holder.tool === "bd" ? `\`${holder.command}\`` : "Branch Deck"
}

/** Who holds the repository, with the pid that tells two of them apart: "`bd push auth` (pid 4242) has held the repository for 40 s" */
export function presentLockHolder(holder: LockHolderInfo): string {
  return `${holderName(holder)} (pid ${holder.pid}) has held the repository for ${formatHeldFor(holder.held_for)}`
}

/** What an operation of `kind` does, in the words of a sentence */
const KIND_WORDS: Record<OperationKind, string> = {
  safe_push: "Safe Push",
  dry_run: "Dry Run",
  push: "push",
  restart_robot: "restart of a robot",
  cancel_robot: "cancel of a robot",
  retarget: "retarget of a pull request",
  create_pull_request: "creation of the pull request",
  link_stack: "link of a stack of GitHub",
  request_review: "review request",
  close_pull_request: "close of a pull request",
  disable_auto_merge: "cancel of an auto-merge",
  close_landed_pull_requests: "close of the pull requests a robot landed",
  submit_review: "review of a pull request",
  set_issue_state: "change of the state of an issue",
  forget_start: "forget of a start",
  rewrite: "rewrite",
  fetch_baseline: "fetch of the baseline",
  update_project: "update of the project",
  dismiss_landed: "dismissal of the landed commits",
  fetch_review: "fetch of a pull request to review",
  pick: "pick of commits",
}

/** Who runs an operation of another process: `bd`, or another Branch Deck */
export function toolName(tool: HolderTool): string {
  return tool === "bd" ? "`bd`" : "Branch Deck"
}

/**
 * Who runs the operation, with the pid that tells two of them apart: "`bd` (pid 4242) has run the Safe Push of auth for
 * 40 s"; of a start whose answer was lost, that it holds until Patronus tells or the start is forgotten
 */
export function presentOperationHolder(holder: OperationHolder): string {
  const what = holder.subject === null ? KIND_WORDS[holder.kind] : `${KIND_WORDS[holder.kind]} of ${holder.subject}`
  if (holder.unknown === true) {
    return `Cannot tell whether the ${what} that ${toolName(holder.tool)} (pid ${holder.pid}) sent started: it holds until Patronus lists its robot, or until Forget This Start`
  }
  return `${toolName(holder.tool)} (pid ${holder.pid}) has run the ${what} for ${formatHeldFor(holder.held_for)}`
}

/**
 * Whom an operation waits for while another holder has the repository lock, in the words the rewrites of the app say it
 * ("Waiting for `bd pick` (pid 4242, for 12 s)"); a holder no record names: "Waiting for another holder of the
 * repository lock"
 */
export function presentLockWait(holder: LockHolderInfo | null): string {
  if (holder === null) {
    return "Waiting for another holder of the repository lock"
  }
  return `Waiting for ${holderName(holder)} (pid ${holder.pid}, for ${formatHeldFor(holder.held_for)})`
}

/** Whom `operation` waits for while another holder has the repository lock, its step in the view; null while it doesn't */
export function presentOperationLockWait(operation: RunningOperation | undefined): string | null {
  return operation?.step.kind === "waitingForLock" ? presentLockWait(operation.step.holder) : null
}

/** An operation of the session a surface shows while it runs (`useSurfaceOperation`, `waitingOf`), as Stop Waiting reads it */
export interface WaitingOperation {
  /** The operation of the session, once its channel named it; null before */
  readonly op: OperationId | null
  /** Whom it waits for while another holder has the repository lock (`presentLockWait`); null while it waits for nobody */
  readonly lockWait: string | null
}

/** `operation` of the view as Stop Waiting reads it: one of another process, which no window here stops, never waits */
export function waitingOf(operation: RunningOperation | undefined): WaitingOperation | undefined {
  return operation === undefined || operation.origin.kind !== "thisProcess" ? undefined : { op: operation.id, lockWait: presentOperationLockWait(operation) }
}

/**
 * Stop Waiting of an operation of the session while it waits for another holder of the repository lock: the one item
 * every surface that shows the waiting operation renders (the "⋯" of its row, the surface that started it), which stops
 * it by its id (`stop`: `cancelIntent` of `src/model/intents.ts`): it then changes nothing and says so ("Canceled
 * squashing 2 commits before it started"); null while it waits for nobody
 */
export function presentStopWaiting(operation: WaitingOperation | undefined, stop: (op: OperationId) => Promise<unknown>): MenuItem | null {
  const op = operation?.op ?? null
  if (op === null || operation?.lockWait == null) {
    return null
  }
  return {
    id: "stop-waiting",
    label: "Stop Waiting",
    description: `${operation.lockWait}; stop and change nothing`,
    onSelect: () => void stop(op),
  }
}

/**
 * The toast of an action whose wait for the repository lock the user stopped: "Canceled the push of auth before it
 * started", "Canceled squashing 2 commits before it started"; `action` names it in the middle of a sentence
 */
export function canceledBeforeStart(action: string, nothing = "Nothing changed"): ToastMessage {
  return { type: "info", title: `Canceled ${action} before it started`, description: nothing }
}

/** The busy state of an operation ("Squashing 2 commits") as the action in the middle of a sentence */
export function actionOfLabel(label: string): string {
  return label.charAt(0).toLowerCase() + label.slice(1)
}

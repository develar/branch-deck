// The page's words of a holder of the repository lock (`LockHolderInfo`, as the record beside the lock names it,
// `repository_lock.rs`): whom an operation waits for (`OperationStep.waitingForLock`, every window reads it in the view),
// its Stop Waiting, the toast of a wait so stopped, and who holds the lock a command was refused by
// (`ErrorDetails.locked`, `describeError`).
import type { LockHolderInfo, OperationId, RunningOperation } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import type { MenuItem } from "@/lib/menu"
import type { ToastMessage } from "@/lib/toast-message"

/**
 * How long a holder has held the lock, or run an operation, as `LockHolder` of `repository_lock.rs` says it: "40 s",
 * "12 min 3 s"
 */
export function formatHeldFor(seconds: number): string {
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

/** An operation of the session the page shows while it runs (a record of `runOperation`), as Stop Waiting reads it */
export interface WaitingOperation {
  /** The operation of the session, once its channel named it; null before */
  readonly op: OperationId | null
  /** Whom it waits for while another holder has the repository lock (`presentLockWait`); null while it waits for nobody */
  readonly lockWait: string | null
}

/**
 * Stop Waiting of an operation of the session while it waits for another holder of the repository lock: the one item
 * every surface that shows the waiting operation renders (the "⋯" of its row, the surface that started it), which stops
 * it (`cancel_operation`): it then changes nothing and says so ("Canceled squashing 2 commits before it started"); null
 * while it waits for nobody
 */
export function presentStopWaiting(operation: WaitingOperation | undefined): MenuItem | null {
  const op = operation?.op ?? null
  if (op === null || operation?.lockWait == null) {
    return null
  }
  return {
    id: "stop-waiting",
    label: "Stop Waiting",
    description: `${operation.lockWait}; stop and change nothing`,
    onSelect: () => void commands.cancelOperation({ op }).catch(() => {}),
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

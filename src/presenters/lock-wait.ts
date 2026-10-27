// The page's words of a holder of the repository lock (`LockHolderInfo`, as the record beside the lock names it,
// `repository_lock.rs`): whom an operation waits for (`OperationStep.waitingForLock`, every window reads it in the view)
// and who holds the lock a command was refused by (`ErrorDetails.locked`, `describeError`).
import type { LockHolderInfo, RunningOperation } from "@/lib/bindings"

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

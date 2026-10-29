// The rewrites of the page, which the engine runs (the intent `rewrite`, `operations::rewrite`; docs/design/operations.md,
// "Rewrites"): Move (a drag, Move to…, Ungroup, Remove Prefix), Rename, Stack On, Unstack, Park, Unpark, Reword, Squash,
// Squash Fixups, Amend, Add Issue Reference, Apply Fix, Unapply, Reapply, Pick Remote Commits, the deletes of an archived
// and an unapplied branch, and the Undo of a rewrite by the token its outcome gave. The session admits each: a second
// click of the same rewrite joins it, another waits for it in the queue of the session and runs on what it left, a
// rewrite runs during a sync; the operation ends once it asked the sync that shows it, and its answer comes once that
// sync ended, so the toast with Undo shows beside what it changed; a delete asks no sync, its answer comes with the
// patch that removed its row. Every window shows it from the view (`RepositoryView.operations`: its rows busy, the
// actions Rust lays it over, whom it waits for while another holder has the repository lock, Stop Waiting:
// `cancel_operation`); the window that starts it knows only where it started (`startIndicator`: the surface's spinner
// from the click to the answer). The page decides nothing of it: it words the failure and the outcome.
import type { ApiError, OperationEvent, Outcome, RewriteIntent, RewriteResult, TableWitness, UndoToken } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { showToast } from "@/lib/toast-message"
import { executeRewrite } from "@/model/intents"
import { getSessionWitness } from "@/model/store"
import { actionOfLabel, canceledBeforeStart } from "@/presenters/lock-wait"
import { presentRewrite } from "@/presenters/rewrite"
import { startIndicator } from "@/stores/indicators"

/** Where a rewrite starts and what it says while it runs */
export interface RewriteRunSpec {
  /** The repository of the click: the rewrite and its Undo go there, whichever the main window shows when they run */
  readonly repositoryPath: string
  /**
   * The surface that started it (`SelectContext.surface`): it shows the spinner until the rewrite answered; null or
   * none for a menu, a key or a toast, whose row shows it on its "⋯"
   */
  readonly surface?: string | null
  /** Its busy state: the label of the rewrite by default ("Unstacking oauth") */
  readonly label?: string
  /**
   * Runs once its core wrote what it changed (the operation ended, `done` on its channel), before the sync it asked
   * shows it: a preview follows its commit to the id the rewrite gave it while that commit is still in the table
   */
  readonly onWritten?: (outcome: Outcome) => void
}

/** What a rewrite that ran answered: its outcome, what its core did, and how it is undone (null: it has no undo) */
export interface RewriteDone {
  readonly outcome: Outcome
  readonly result: RewriteResult
  readonly undo: UndoToken | null
}

/** A rewrite of `R` without the table it is named on (each kind of rewrite but the undo names one) */
export type Unwitnessed<R extends RewriteIntent = RewriteIntent> = R extends { readonly witness: TableWitness } ? Omit<R, "witness"> : never

/** The rewrite `rewrite` named on the table the window shows of `repositoryPath` (`getSessionWitness`) */
export function witnessed<R extends RewriteIntent = RewriteIntent>(repositoryPath: string, rewrite: Unwitnessed<R>): R {
  const named: RewriteIntent = { ...rewrite, witness: getSessionWitness(repositoryPath) } as RewriteIntent
  return named as R
}

/**
 * Runs `rewrite` in its repository (`executeRewrite`) from `spec.surface`, which shows its spinner from the click until
 * the sync it asked showed what it changed; a failure is an error toast titled `failureTitle` (the failure words of the
 * rewrite by default), a stopped wait an info toast ("Canceled moving 1 commit to docs before it started"). Resolves to
 * what it did, null after a failure or a stop; a second start of the same rewrite joins the first in the session and
 * resolves with its outcome
 */
export async function runRewriteIntent(spec: RewriteRunSpec, rewrite: RewriteIntent, failureTitle?: string): Promise<RewriteDone | null> {
  const words = presentRewrite(rewrite)
  const label = spec.label ?? words.label
  const indicator = startIndicator(spec.surface, label)
  const onEvent = (event: OperationEvent) => {
    if (event.kind === "done") {
      spec.onWritten?.(event.outcome)
    } else if (event.kind === "step") {
      indicator.named(event.op)
    }
  }
  const run = await executeRewrite(spec.repositoryPath, rewrite, onEvent).finally(indicator.done)
  if (run.kind === "failed") {
    tellFailure(run.error, label, failureTitle ?? words.failure)
    return null
  }
  const { outcome } = run
  if (outcome.result.kind !== "rewrite") {
    return null
  }
  const result: RewriteResult = outcome.result
  return { outcome, result, undo: outcome.undo }
}

function tellFailure(error: ApiError, label: string, failureTitle: string): void {
  if (error.code === "canceled") {
    // stopped before it started: nothing changed
    showToast(canceledBeforeStart(actionOfLabel(label)))
  } else {
    notifyError(failureTitle, error)
  }
}

/**
 * The Undo of a rewrite by its token (the rewrite `undo`) in `repositoryPath`, the repository the rewrite was made in,
 * whichever the main window shows at the click: queued behind a rewrite that runs, as any rewrite; `onWritten` once it
 * moved the refs back, before the sync shows it. Resolves to whether it undid it: the toast and the Activity entry of
 * the rewrite offer their Undo again after a failure (`showUndoToast`)
 */
export async function undoRewrite(repositoryPath: string, token: UndoToken, label: string, failureTitle: string, onWritten?: () => void): Promise<boolean> {
  const spec: RewriteRunSpec = { label, repositoryPath, ...(onWritten !== undefined && { onWritten }) }
  const done = await runRewriteIntent(spec, { rewrite: "undo", token }, failureTitle)
  return done !== null
}

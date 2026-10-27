// The rewrites of the page that the engine runs (the intent `rewrite`, `operations::rewrite`; docs/design/operations.md,
// "Rewrites"): Move (a drag, Move to…, Ungroup, Remove Prefix), Rename, Stack On, Unstack, Park, Unpark, and the Undo
// of a rewrite by the token its outcome gave. The session admits each: a second click of the same rewrite joins it,
// another waits for it in the queue of the session and runs on what it left, a rewrite runs during a sync; the operation
// ends once it asked the sync that shows it, and its answer comes once that sync ended, so the toast with Undo shows
// beside what it changed. The window that starts it shows it while it runs (a record of `runOperation` the session
// admits, never queued in the page): its surface or its row busy, and from the steps of its operation whom it waits for
// while another holder has the repository lock (Stop Waiting: `cancel_operation`) or which rewrite it waits behind. The
// page decides nothing of it: it words the failure and the outcome.
import type { ApiError, OperationEvent, OperationId, Outcome, RewriteIntent, RewriteResult, TableWitness, UndoToken } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { actionOfLabel, canceledBeforeStart } from "@/lib/lock-wait"
import { executeRewrite } from "@/lib/run-intent"
import { showToast } from "@/lib/toast-message"
import { getSessionWitness } from "@/model/store"
import { presentLockWait } from "@/presenters/lock-wait"
import { presentRewrite } from "@/presenters/rewrite"
import { branchSyncStore } from "@/stores/branch-sync"
import type { OperationSpec } from "@/stores/operations"
import { runOperation, setSessionStep } from "@/stores/operations"

/** A rewrite as the page shows it while it runs: its operation without the kind; the label of the rewrite by default */
export interface RewriteRunSpec extends Omit<OperationSpec, "kind" | "label" | "repositoryPath" | "admittedBySession"> {
  /** The repository of the click: the rewrite and its Undo go there, whichever the main window shows when they run */
  readonly repositoryPath: string
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

/** The busy state of the operation a step names, by its id: the rewrites this window started */
const labels = new Map<OperationId, string>()

/** The busy state of the operation `op` a rewrite waits behind: one of this window, else as the view names a rewrite */
function labelOf(op: OperationId): string {
  const own = labels.get(op)
  if (own !== undefined) {
    return own
  }
  const intent = branchSyncStore.getState().operations[op]?.intent
  return intent?.kind === "rewrite" ? presentRewrite(intent).label : "another rewrite"
}

/**
 * Runs `rewrite` in its repository (`executeRewrite`) as the operation `spec` of the page: its surfaces busy from the
 * click until the sync it asked showed what it changed, saying what it waits for; a failure is an error toast titled
 * `failureTitle` (the failure words of the rewrite by default), a stopped wait an info toast ("Canceled moving 1 commit
 * to docs before it started"). Resolves to what it did, null after a failure or a stop; a second start of the same
 * action with the same parameters resolves with the first
 */
export async function runRewriteIntent(spec: RewriteRunSpec, rewrite: RewriteIntent, failureTitle?: string): Promise<RewriteDone | null> {
  const words = presentRewrite(rewrite)
  const label = spec.label ?? words.label
  const { onWritten: _written, ...operation } = spec
  const done = await runOperation({ ...operation, kind: "rewrite", label, admittedBySession: true }, async key => {
    let op: OperationId | null = null
    const onEvent = (event: OperationEvent) => {
      if (event.kind === "done") {
        spec.onWritten?.(event.outcome)
        return
      }
      if (event.kind !== "step") {
        return
      }
      op = event.op
      labels.set(event.op, label)
      const { step } = event
      const stop = () => void commands.cancelOperation({ op: event.op }).catch(() => {})
      setSessionStep(
        key,
        {
          lockWait: step.kind === "waitingForLock" ? presentLockWait(step.holder) : null,
          waitsFor: step.kind === "queued" ? labelOf(step.behind) : null,
        },
        stop,
      )
    }
    const run = await executeRewrite(spec.repositoryPath, rewrite, onEvent).finally(() => {
      if (op !== null) {
        labels.delete(op)
      }
    })
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
  })
  return done ?? null
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
  const spec: RewriteRunSpec = { id: "undo", subject: `undo:${token.token}`, objects: [], label, repositoryPath, ...(onWritten !== undefined && { onWritten }) }
  const done = await runRewriteIntent(spec, { rewrite: "undo", token }, failureTitle)
  return done !== null
}

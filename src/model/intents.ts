// The intents of a repository as the page runs them (docs/design/rust-domain-authority.md, "Планирование и исполнение
// операций"): `prepare` answers its plan in the session of the repository, `execute` runs that plan on its fingerprint,
// `cancel_operation` stops a run; the page calls the engine only here (`intent-call` of `scripts/lint-arch.mjs`). The
// session admits a run (a second click joins it, one that needs what another holds waits for it), shows its steps in
// the view of every window and the moves of its rows. The page decides nothing of it: it words the plan's refusal and
// the outcome. A start of a Safe Push or a Dry Run asks its confirmations first (`startPlanned` of `useSafePush.ts`);
// the intents run here ask none. A rewrite names the table it was asked on in its intent and plans itself in the
// session (`executeRewrite`): no `prepare`.
import { Channel } from "@tauri-apps/api/core"
import type { ApiError, ExecuteParams, Intent, OperationEvent, OperationId, Outcome, Plan, Reason, RewriteIntent } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import type { CommandResult } from "@/lib/command"
import { whenSessionViewAt } from "@/model/store"
import { formatReason } from "@/presenters/reasons"
import { refreshActivity } from "@/stores/activity"
import { setOperationPushProgress } from "@/stores/branch-sync"

/** How a run of an intent went */
export type IntentRun =
  /** It ran: its plan and how each effect went; the view of the session has what it changed */
  | { readonly kind: "ran"; readonly plan: Plan; readonly outcome: Outcome }
  /** Its plan refuses it: nothing was done */
  | { readonly kind: "refused"; readonly plan: Plan; readonly reason: Reason }
  /** The session has no plan of it: the facts it decides on are not there yet, or its object is gone */
  | { readonly kind: "noPlan" }
  /** It failed before its first effect, or the plan changed under it: nothing was done */
  | { readonly kind: "failed"; readonly error: ApiError }

/** The plan of an intent that can run (`planIntent`), or why not */
export type PlannedIntent = { readonly kind: "planned"; readonly plan: Plan } | Exclude<IntentRun, { readonly kind: "ran" }>

/** The plan of `intent` in the session of `repositoryPath` (`prepare`): refused by its first blocker, none, or failed */
export async function planIntent(repositoryPath: string, intent: Intent): Promise<PlannedIntent> {
  const prepared = await commands.prepare({ repositoryPath, intent })
  if (prepared.status === "error") {
    return { kind: "failed", error: prepared.error }
  }
  const plan = prepared.data
  if (plan === null) {
    return { kind: "noPlan" }
  }
  const blocker = plan.blockers[0]
  return blocker === undefined ? { kind: "planned", plan } : { kind: "refused", plan, reason: blocker }
}

/**
 * Runs `intent` in the session of `repositoryPath`: its plan (`planIntent`), refused by its first blocker, else run on
 * its fingerprint with what it asks accepted (`executePlan`)
 */
export async function runIntent(repositoryPath: string, intent: Intent, onEvent: (event: OperationEvent) => void = () => {}): Promise<IntentRun> {
  const planned = await planIntent(repositoryPath, intent)
  return planned.kind === "planned" ? executePlan(repositoryPath, planned.plan, onEvent) : planned
}

/**
 * `execute` of `params`, its events to `events`: the operation ended in the journal of its repository before it
 * answered, so Activity reads the log again (`refreshActivity`) and has what it did, in the words of its toast
 */
export async function executeIntent(params: ExecuteParams, events: Channel<OperationEvent>): Promise<CommandResult<Outcome>> {
  const executed = await commands.execute(params, events)
  void refreshActivity()
  return executed
}

/**
 * Runs `plan` in the session of `repositoryPath` on its fingerprint, with what it asks accepted (`executeIntent`);
 * resolves once the view of the session has what the run changed (`versionAfter`), so what the page shows next reads
 * it. The push buttons of the branches its push sends show how far it is (`operationPushProgress`); `onEvent` hears its
 * steps and that progress too
 */
export async function executePlan(repositoryPath: string, plan: Plan, onEvent: (event: OperationEvent) => void = () => {}): Promise<IntentRun> {
  const events = new Channel<OperationEvent>()
  const progress = operationPushProgress()
  events.onmessage = event => {
    progress.onEvent(event)
    onEvent(event)
  }
  const executed = await executeIntent({ repositoryPath, intent: plan.intent, fingerprint: plan.fingerprint, accepted: plan.confirmations }, events).finally(progress.done)
  if (executed.status === "error") {
    return { kind: "failed", error: executed.error }
  }
  await whenSessionViewAt(repositoryPath, executed.data.versionAfter)
  return { kind: "ran", plan, outcome: executed.data }
}

/**
 * Stops the operation `op` of the session (`cancel_operation`): a wait for another holder of the repository lock ends
 * with nothing changed, and no effect starts after it; an effect that runs (a push that sends) goes to its end. Resolves
 * whether the session still had it; an IPC failure (the window closes) answers false
 */
export function cancelIntent(op: OperationId): Promise<boolean> {
  return commands.cancelOperation({ op }).catch(() => false)
}

/** How a run of a rewrite went (`executeRewrite`) */
export type RewriteExecution = { readonly kind: "ran"; readonly outcome: Outcome } | { readonly kind: "failed"; readonly error: ApiError }

/**
 * Runs the rewrite `rewrite` in the session of `repositoryPath` (`execute` of the intent `rewrite`), without a
 * `prepare`: the rewrite names the table it was asked on (`TableWitness`) and asks nothing to accept, and the session
 * plans it once admitted, also a session that is not open (the Undo of a repository the window left). Resolves once the
 * view of the window has what the sync it asked showed (`versionAfter`), its steps to `onEvent` meanwhile
 */
export async function executeRewrite(repositoryPath: string, rewrite: RewriteIntent, onEvent: (event: OperationEvent) => void): Promise<RewriteExecution> {
  const events = new Channel<OperationEvent>()
  events.onmessage = onEvent
  const executed = await executeIntent({ repositoryPath, intent: { kind: "rewrite", ...rewrite }, fingerprint: "" }, events)
  if (executed.status === "error") {
    return { kind: "failed", error: executed.error }
  }
  await whenSessionViewAt(repositoryPath, executed.data.versionAfter)
  return { kind: "ran", outcome: executed.data }
}

/**
 * How far the push of an operation this window runs is, as its channel tells it: the push buttons of the branches it
 * pushes show it (`setOperationPushProgress`; the view says which, `pushOperationIn`, and whom a wait for the repository
 * lock is for, `useBranchPushProgress`); `done` forgets it
 */
export function operationPushProgress(): { readonly onEvent: (event: OperationEvent) => void; readonly done: () => void } {
  let op: OperationId | null = null
  const onEvent = (event: OperationEvent) => {
    if (event.kind === "progress") {
      op = event.op
      setOperationPushProgress(event.op, { percent: event.percent, line: event.line, transferred: null, speed: null })
    }
  }
  return {
    onEvent,
    done: () => {
      if (op !== null) {
        setOperationPushProgress(op, null)
      }
    },
  }
}

/** The first effect of `outcome` that failed, with its error; null when none did */
export function failedEffect(outcome: Outcome): { readonly effect: Outcome["effects"][number]["effect"]; readonly error: ApiError } | null {
  for (const { effect, status } of outcome.effects) {
    if (status.kind === "failed") {
      return { effect, error: status.error }
    }
  }
  return null
}

/** Each effect of `outcome` that failed, with its error, in their order: of an operation whose effects go on after one fails (the closes) */
export function failedEffects(outcome: Outcome): readonly { readonly effect: Outcome["effects"][number]["effect"]; readonly error: ApiError }[] {
  return outcome.effects.flatMap(({ effect, status }) => (status.kind === "failed" ? [{ effect, error: status.error }] : []))
}

/** Why an effect of `outcome` was skipped because the facts changed before it (`plan_changed`); null when none was */
export function skippedReason(outcome: Outcome): ApiError | null {
  for (const { status } of outcome.effects) {
    if (status.kind === "skipped" && status.reason != null) {
      return status.reason
    }
  }
  return null
}

/**
 * Why a run did not get to what it was for, as an error toast or an info toast says it: what its plan refuses, the
 * session without a plan of it (`noPlan`, in the caller's words), what failed, the first effect that failed, the facts
 * that changed before an effect; null for a run whose effects all went
 */
export function runFailure(run: IntentRun, noPlan: string): ApiError | string | null {
  switch (run.kind) {
    case "refused":
      return formatReason(run.reason)
    case "noPlan":
      return noPlan
    case "failed":
      return run.error
    case "ran":
      return failedEffect(run.outcome)?.error ?? skippedReason(run.outcome)
  }
}

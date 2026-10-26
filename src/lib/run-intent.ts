// The run of an intent Rust plans and runs (docs/design/rust-domain-authority.md, "Планирование и исполнение
// операций"): `prepare` answers its plan in the session of the repository, `execute` runs that plan on its fingerprint;
// the session admits it (a second click joins it, one that needs what another holds waits for it), shows its steps in
// the view of every window and the moves of its rows. The page decides nothing of it: it words the plan's refusal and
// the outcome. A start of a Safe Push or a Dry Run asks its confirmations first (`startPlanned` of `useSafePush.ts`);
// the intents run here ask none.
import { Channel } from "@tauri-apps/api/core"
import type { ApiError, Intent, OperationEvent, OperationId, Outcome, Plan, Reason } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { whenSessionViewAt } from "@/model/store"
import { formatReason } from "@/presenters/reasons"
import { recordPush, recordRetarget } from "@/stores/activity"
import type { RemoteStatus } from "@/stores/branch-sync"
import { getBranch, setOperationPushProgress } from "@/stores/branch-sync"
import { getPullRequest } from "@/stores/pull-requests"
import { getEffectiveBranchPrefix } from "@/stores/repository"

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
 * Runs `plan` in the session of `repositoryPath` on its fingerprint, with what it asks accepted (`execute`); resolves
 * once the view of the session has what the run changed (`versionAfter`), so what the page shows next reads it, and
 * Activity has its retargets and its push (`recordEffects`). The push buttons of the branches its push sends show how
 * far it is (`operationPushProgress`); `onEvent` hears its steps and that progress too
 */
export async function executePlan(repositoryPath: string, plan: Plan, onEvent: (event: OperationEvent) => void = () => {}): Promise<IntentRun> {
  const before = pushedBefore(plan)
  const events = new Channel<OperationEvent>()
  const progress = operationPushProgress()
  events.onmessage = event => {
    progress.onEvent(event)
    onEvent(event)
  }
  const executed = await commands.execute({ repositoryPath, intent: plan.intent, fingerprint: plan.fingerprint, accepted: plan.confirmations }, events).finally(progress.done)
  if (executed.status === "error") {
    return { kind: "failed", error: executed.error }
  }
  await whenSessionViewAt(repositoryPath, executed.data.versionAfter)
  recordEffects(executed.data, repositoryPath, before)
  return { kind: "ran", plan, outcome: executed.data }
}

/**
 * How far the push of an operation this window runs is, as its channel tells it: the push buttons of the branches it
 * pushes show it (`setOperationPushProgress`; the view says which, `pushOperationIn`), and while it waits for another
 * holder of the repository lock whom for, with Stop Waiting (`cancel_operation`: nothing pushed); `done` forgets it
 */
export function operationPushProgress(): { readonly onEvent: (event: OperationEvent) => void; readonly done: () => void } {
  let op: OperationId | null = null
  let waiting = false
  const onEvent = (event: OperationEvent) => {
    if (event.kind === "step") {
      waiting = event.step.kind === "waitingForLock"
    } else if (event.kind === "progress") {
      op = event.op
      const stop = event.op
      setOperationPushProgress(event.op, {
        percent: event.percent,
        line: event.line,
        transferred: null,
        speed: null,
        waiting,
        ...(waiting && { stopWaiting: () => void commands.cancelOperation({ op: stop }) }),
      })
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

/** A base as the rows name it: a virtual branch by its short name (`auth` of `user/virtual/auth`), another as it is */
function shortBase(base: string): string {
  const virtual = `${getEffectiveBranchPrefix()}/virtual/`
  return base.startsWith(virtual) ? base.slice(virtual.length) : base
}

/** The remote status of each branch the push of `plan` sends, before it: what Activity says the push changed */
export function pushedBefore(plan: Plan): ReadonlyMap<string, RemoteStatus | null> {
  const push = plan.effects.find(effect => effect.kind === "push")
  return new Map(push?.kind === "push" ? push.branches.map(name => [name, getBranch(name)?.remoteStatus ?? null] as const) : [])
}

/**
 * Activity of the effects of a run that have no toast of their own, each with the operation: the retargets of the pull
 * requests into a virtual branch (D11) and the push
 */
export function recordEffects(outcome: Outcome, repositoryPath: string, before: ReadonlyMap<string, RemoteStatus | null>): void {
  for (const { effect, status } of outcome.effects) {
    if (status.kind !== "done") {
      continue
    }
    if (effect.kind === "retarget") {
      const url = getPullRequest(effect.branch)?.url
      if (url !== undefined) {
        recordRetarget(`Retargeted #${effect.number} to ${effect.to}`, `${effect.branch} · it went into ${shortBase(effect.from)}`, url)
      }
    } else if (effect.kind === "push") {
      for (const name of effect.branches) {
        recordPush(repositoryPath, name, before.get(name) ?? null, null)
      }
    }
  }
}

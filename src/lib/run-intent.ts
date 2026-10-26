// The run of an intent Rust plans and runs (docs/design/rust-domain-authority.md, "Планирование и исполнение
// операций"): `prepare` answers its plan in the session of the repository, `execute` runs that plan on its fingerprint;
// the session admits it (a second click joins it, one that needs what another holds waits for it), shows its steps in
// the view of every window and the moves of its rows. The page decides nothing of it: it words the plan's refusal and
// the outcome. A start of a Safe Push or a Dry Run asks its confirmations first (`startPlanned` of `useSafePush.ts`);
// the intents run here ask none.
import { Channel } from "@tauri-apps/api/core"
import type { ApiError, Intent, OperationEvent, Outcome, Plan, Reason } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { whenSessionViewAt } from "@/model/store"
import { formatReason } from "@/presenters/reasons"
import { recordPush, recordRetarget } from "@/stores/activity"
import type { RemoteStatus } from "@/stores/branch-sync"
import { getBranch } from "@/stores/branch-sync"
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

/**
 * Runs `intent` in the session of `repositoryPath`: its plan (`prepare`), refused by its first blocker, else run on its
 * fingerprint (`execute`); resolves once the view of the session has what the run changed (`versionAfter`), so what the
 * page shows next reads it, and Activity has its retargets and its push (`recordEffects`). `onEvent` hears its steps
 * and the progress of its push
 */
export async function runIntent(repositoryPath: string, intent: Intent, onEvent: (event: OperationEvent) => void = () => {}): Promise<IntentRun> {
  const prepared = await commands.prepare({ repositoryPath, intent })
  if (prepared.status === "error") {
    return { kind: "failed", error: prepared.error }
  }
  const plan = prepared.data
  if (plan === null) {
    return { kind: "noPlan" }
  }
  const blocker = plan.blockers[0]
  if (blocker !== undefined) {
    return { kind: "refused", plan, reason: blocker }
  }
  const before = pushedBefore(plan)
  const events = new Channel<OperationEvent>()
  events.onmessage = onEvent
  const executed = await commands.execute({ repositoryPath, intent: plan.intent, fingerprint: plan.fingerprint, accepted: plan.confirmations }, events)
  if (executed.status === "error") {
    return { kind: "failed", error: executed.error }
  }
  await whenSessionViewAt(repositoryPath, executed.data.versionAfter)
  recordEffects(executed.data, repositoryPath, before)
  return { kind: "ran", plan, outcome: executed.data }
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

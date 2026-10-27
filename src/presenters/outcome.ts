// The page's words of how a start of a Safe Push or a Dry Run ended (`Outcome` of `execute`, its `LandResult`) and of
// what refused it before its first effect (the `ApiError` of `execute`): the toast of the event of its robot and the
// Activity entry, never a rule. The toast is the next state of the event of the subject's robots
// (docs/guidelines/ux-writing.md#one-toast-per-event): it takes the place of the toast of the approvals it starts on
// and of the robot it replaces; a refusal of the landing gate offers its fix and Start Anyway, a conflict with the
// newest target Update Project, a start the facts changed under "Start Safe Push" again.
import type { ApiError, EffectRecord, Intent, LandingAllowances, LandingFix, Outcome, PatronusPushMode, PatronusRobot, StartNote } from "@/lib/bindings"
import { describeRobot, formatPushMode } from "@/lib/patronus"
import { describeLandingRefusals } from "@/presenters/landing"

/** What ended a start, as its toast says it */
export type LandToast =
  /** The robot started: the toast of the robot, with Open in Patronus and Cancel Safe Push */
  | { readonly kind: "started"; readonly title: string; readonly description: string; readonly robot: PatronusRobot }
  /** Patronus accepted the comment on the pull request and lists no robot of it yet: the toast opens the comment */
  | { readonly kind: "commented"; readonly title: string; readonly description: string; readonly number: number; readonly url: string }
  /**
   * The robot did not start: an effect failed (`error`), or the facts changed after the push (`again`: "Start Safe
   * Push", a new plan without the push or the cancel it made; `canceled` the robot it canceled, which Open in Patronus
   * opens)
   */
  | {
      readonly kind: "failed"
      readonly title: string
      readonly description: string
      readonly error: ApiError | null
      readonly again: boolean
      readonly canceled: PatronusRobot | null
    }
  /** The user stopped it before its next effect: nothing more was done */
  | { readonly kind: "stopped"; readonly title: string; readonly description: string }
  /**
   * Its start went out and the answer was lost, or the bot did not answer its comment within the wait: whether its
   * robot started is not known; it holds its subject until Patronus lists the robot or the user forgets it on the row.
   * `url` opens where to look (the pull request of its comment), null: Patronus
   */
  | { readonly kind: "unknown"; readonly title: string; readonly description: string; readonly url: string | null }

/** What refused a start before its first effect, as its toast says it */
export type LandRefusal =
  /** The landing gate: the fix of the first refusal and Start Anyway, the same start with the overrides */
  | { readonly kind: "landing"; readonly title: string; readonly reason: string; readonly fix: LandingFix | null; readonly anyway: LandingAllowances }
  /** Patronus can't rebase what lands onto the newest target: Update Project resolves it locally */
  | { readonly kind: "target"; readonly title: string; readonly error: ApiError }
  /** What the plan was decided on changed before its first effect: "Start Safe Push" plans it again */
  | { readonly kind: "changed"; readonly title: string; readonly error: ApiError }
  /** The user stopped it while it waited for the repository lock: nothing was pushed or started */
  | { readonly kind: "stopped"; readonly title: string; readonly description: string }
  | { readonly kind: "error"; readonly title: string; readonly error: ApiError }

/** "Cannot start the Safe Push of auth" */
function refusedTitle(mode: PatronusPushMode, subject: string): string {
  return `Cannot start the ${formatPushMode(mode)} of ${subject}`
}

/** A note of the checks before the effects, as a sentence after the description */
function noteWords(note: StartNote, subject: string): string {
  switch (note.kind) {
    case "targetFetchFailed":
      return `The fetch of ${note.target} failed, so the check used the last fetched ${note.target}: ${note.error}`
    case "targetNotChecked":
      return `Not checked whether Patronus can rebase ${subject} onto ${note.target}: ${note.error}`
    case "landingNotChecked":
      return `Not checked whether ${subject} uses a name only another branch adds: ${note.error}`
  }
}

/** The description with the notes of the checks, each a sentence, and the retargets GitHub refused on the way */
function withNotes(description: string, notes: readonly StartNote[], effects: readonly EffectRecord[], subject: string): string {
  const refused = effects.flatMap(record => (record.effect.kind === "retarget" && record.status.kind === "failed" ? [record.status.error.message] : []))
  return [description, ...notes.map(note => noteWords(note, subject)), ...refused].join(". ")
}

/**
 * The toast of the outcome of the start `intent` of `subject` ("auth", "the whole air stack"): the robot it started, the
 * comment Patronus accepted, or why its robot did not start, the effects before it named as they went. The entry of
 * Activity of the start says the same (`presentActivity`, over the journal of the repository)
 */
export function presentLandOutcome(outcome: Pick<Outcome, "effects" | "result">, intent: Intent, subject: string): LandToast {
  const mode = intent.kind === "land" ? intent.mode : "rebaseAutosquash"
  const modeName = formatPushMode(mode)
  const result = outcome.result.kind === "land" ? outcome.result : null
  const notes = result?.notes ?? []
  const failed = (description: string, error: ApiError | null, more: Partial<Extract<LandToast, { kind: "failed" }>> = {}): LandToast => ({
    kind: "failed",
    title: refusedTitle(mode, subject),
    description: withNotes(description, notes, outcome.effects, subject),
    error,
    again: false,
    canceled: result?.canceled ?? null,
    ...more,
  })
  const stopped = outcome.effects.find(record => record.status.kind === "failed" && record.effect.kind !== "retarget")
  if (stopped?.status.kind === "failed") {
    return failed(stopped.status.error.message, stopped.status.error)
  }
  const start = outcome.effects.find(record => record.effect.kind === "startRobot")
  if (start?.status.kind === "skipped") {
    const reason = start.status.reason
    const pushed = outcome.effects.some(record => record.effect.kind === "push" && record.status.kind === "done")
    if (reason?.code === "canceled") {
      const description = pushed ? "The push stays" : "Nothing was pushed or started"
      return { kind: "stopped", title: `Canceled the ${modeName} of ${subject} before its robot started`, description: withNotes(description, notes, outcome.effects, subject) }
    }
    const canceled = result?.canceled ?? null
    const after =
      canceled === null ? (pushed ? "; the push stays" : "") : `; the ${formatPushMode(canceled.pushMode)} it replaced was canceled${pushed ? " and the push stays" : ""}`
    return failed(`${reason?.message ?? "What it was planned on changed"}${after}`, reason ?? null, { again: true })
  }
  const title = `${modeName} started`
  if (result?.robot != null) {
    const route = start?.effect.kind === "startRobot" ? start.effect.route : null
    const profile = route?.kind === "api" ? (route.profile?.name ?? null) : null
    return { kind: "started", title, description: withNotes(describeRobot(result.robot, profile), notes, outcome.effects, subject), robot: result.robot }
  }
  const comment = result?.comment ?? null
  if (start?.status.kind === "unknown") {
    const why =
      comment !== null && !comment.accepted ? `Patronus did not answer the comment on #${comment.number} within a minute` : (result?.lost?.message ?? "Its answer was lost")
    return {
      kind: "unknown",
      title: `Cannot tell whether the ${modeName} of ${subject} started`,
      description: withNotes(`${why}. It holds ${subject} until Patronus lists its robot, or until Forget This Start on the row`, notes, outcome.effects, subject),
      url: comment?.url ?? null,
    }
  }
  if (comment !== null && comment.accepted) {
    const description = `Patronus accepted the comment on #${comment.number}: the robot shows once Patronus lists it`
    return { kind: "commented", title, description: withNotes(description, notes, outcome.effects, subject), number: comment.number, url: comment.url }
  }
  return failed("Patronus did not start the robot", null)
}

/**
 * What refused the start of `subject` in `mode` before its first effect, by the code of `execute`'s error: the landing
 * gate with its refusals (`details.type === "landing"`), the newest target, a changed plan, a stopped wait
 */
export function presentLandRefusal(error: ApiError, mode: PatronusPushMode, subject: string, allow: LandingAllowances = {}): LandRefusal {
  const title = refusedTitle(mode, subject)
  if ((error.code === "missing_declaration" || error.code === "dangling_reference") && error.details?.type === "landing") {
    const refused = describeLandingRefusals(error.details.refusals, subject, allow)
    if (refused !== null) {
      return { kind: "landing", title, ...refused }
    }
  }
  switch (error.code) {
    case "target_conflict":
      return { kind: "target", title, error }
    case "plan_changed":
      return { kind: "changed", title, error }
    case "canceled":
      return { kind: "stopped", title: `Canceled the ${formatPushMode(mode)} of ${subject} before it started`, description: "Nothing was pushed or started" }
    default:
      return { kind: "error", title, error }
  }
}

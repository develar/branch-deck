// The page's words of how a start of a Safe Push, a Dry Run or a push into the target ended (`Outcome` of `execute`, its `LandResult`) and of
// what refused it before its first effect (the `ApiError` of `execute`): the toast of the event of its robot and the
// Activity entry, never a rule. The toast is the next state of the event of the subject's robots
// (docs/guidelines/ux-writing.md#one-toast-per-event): it takes the place of the toast of the approvals it starts on
// and of the robot it replaces; a refusal of the landing gate offers its fix and Start Anyway, a conflict with the
// newest target Update Project, a start the facts changed under "Start Safe Push" again. A push of the copy into the
// target (Push to master) is one toast too: what it pushed, then the update that follows it in place
// (`presentPushOutcome`, `describeFollowingUpdate`); a target that moved meanwhile a notice, a refusal of the server
// its words.
import type {
  ApiError,
  EffectRecord,
  Intent,
  LandingAllowances,
  LandingFix,
  Outcome,
  PatronusPushMode,
  PatronusRobot,
  PlannedEffect,
  StartNote,
  UpdateProjectResult,
} from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { appliedUpdateToast } from "@/lib/landed"
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
  /** A push of the copy into the target landed it ("Pushed auth into master · 3 commits") */
  | { readonly kind: "pushed"; readonly title: string; readonly description: string }
  /**
   * A push of the copy into the target found the target moved since the copy was built on it: nothing was pushed, the
   * baseline was fetched, and the update that follows builds the copy anew on it, which the step offers to push again
   */
  | { readonly kind: "moved"; readonly title: string; readonly description: string }

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

/**
 * "Cannot start the Safe Push of auth"; of a push into the target `pushInto` ("master"), "Cannot push auth into master"
 */
function refusedTitle(mode: PatronusPushMode, subject: string, pushInto: string | null = null): string {
  return pushInto === null ? `Cannot start the ${formatPushMode(mode)} of ${subject}` : `Cannot push ${subject} into ${pushInto}`
}

/** The push of the copy into the target an outcome or a plan names (`PlannedEffect::PushBaseline`): its target */
export function pushTargetOf(effects: readonly PlannedEffect[]): string | null {
  for (const effect of effects) {
    if (effect.kind === "pushBaseline") {
      return effect.target
    }
  }
  return null
}

/**
 * How a push of the copy of `subject` into the target ended: what it landed, the target that moved meanwhile (the
 * notice that the copy is rebuilt on it), or why it failed: the server's words of a refusal; `commits` the commits it
 * landed, as the page counted them at the start (null: not known, in Activity)
 */
export function presentPushOutcome(outcome: Pick<Outcome, "effects">, subject: string, commits: number | null = null): LandToast {
  const record = outcome.effects.find(it => it.effect.kind === "pushBaseline")
  const target = record?.effect.kind === "pushBaseline" ? record.effect.target : "the target"
  const title = refusedTitle("rebaseAutosquash", subject, target)
  const status = record?.status
  if (status?.kind === "done") {
    const pushed = `Pushed ${subject} into ${target}${commits === null ? "" : ` · ${formatCommitCount(commits)}`}`
    return { kind: "pushed", title: pushed, description: `It landed without a robot and without the CI of a pull request` }
  }
  if (status?.kind === "failed") {
    if (status.error.code === "baseline_moved") {
      return {
        kind: "moved",
        title: `${target} moved: ${subject} is rebuilt on the new commits`,
        // Rust's words say whether the fetch of the target and the sync that rebuilds the copy on it went
        description: status.error.message,
      }
    }
    return { kind: "failed", title, description: status.error.message, error: status.error, again: false, canceled: null }
  }
  if (status?.kind === "skipped" && status.reason?.code === "canceled") {
    return { kind: "stopped", title: `Canceled the push of ${subject} into ${target}`, description: "Nothing was pushed" }
  }
  const failed = outcome.effects.find(it => it.status.kind === "failed")
  const error = failed?.status.kind === "failed" ? failed.status.error : null
  const reason = status?.kind === "skipped" ? status.reason : null
  return {
    kind: "failed",
    title,
    description: error?.message ?? reason?.message ?? "Nothing was pushed",
    error: error ?? reason,
    again: reason !== null && error === null,
    canceled: null,
  }
}

/** The Update Project that follows a push into the target that landed, as the toast of the push says it */
export type FollowingUpdate =
  /** It runs: its fetch, its check, its move */
  | { readonly kind: "running" }
  /** It ended (`UpdateProjectResult`): applied, up to date, or refused by a conflict or local changes */
  | { readonly kind: "ended"; readonly result: UpdateProjectResult }
  /** It failed, or was refused before its first effect (`error`), or did not run (null) */
  | { readonly kind: "failed"; readonly error: ApiError | null }

/**
 * The words of the update that follows a push into the target, below the title of the push: its state, and whether
 * the toast offers Update Project (the update did not move the checked out branch)
 */
export function describeFollowingUpdate(update: FollowingUpdate): { readonly description: string; readonly offersUpdate: boolean } {
  switch (update.kind) {
    case "running":
      return { description: "Updating the project…", offersUpdate: false }
    case "failed":
      return {
        description: update.error === null ? "Update Project drops the pushed commits from the branch" : `Cannot update the project: ${update.error.message}`,
        offersUpdate: true,
      }
    case "ended": {
      const { check, applied, landed } = update.result
      switch (check.state.status) {
        case "upToDate":
          return { description: `${check.branch} is up to date with ${check.upstream}`, offersUpdate: false }
        case "clean": {
          if (applied === null) {
            return { description: `Update Project moves ${check.branch} onto ${check.upstream}`, offersUpdate: true }
          }
          const toast = appliedUpdateToast(check, applied, landed)
          return { description: toast.description === undefined ? toast.title : `${toast.title} · ${toast.description}`, offersUpdate: false }
        }
        case "conflict":
          return { description: `The update of ${check.branch} conflicts with ${check.upstream}: Update Project shows where`, offersUpdate: true }
        case "blocked":
          return { description: `Local changes stand in the way of the update of ${check.branch}: Update Project names them`, offersUpdate: true }
      }
    }
  }
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
export function presentLandOutcome(outcome: Pick<Outcome, "effects" | "result">, intent: Intent, subject: string, commits: number | null = null): LandToast {
  if (pushTargetOf(outcome.effects.map(record => record.effect)) !== null) {
    return presentPushOutcome(outcome, subject, commits)
  }
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
 * gate with its refusals (`details.type === "landing"`), the newest target, a changed plan, a stopped wait; `pushInto`
 * the target of a push of the copy into it ("master"), which the titles name
 */
export function presentLandRefusal(error: ApiError, mode: PatronusPushMode, subject: string, allow: LandingAllowances = {}, pushInto: string | null = null): LandRefusal {
  const title = refusedTitle(mode, subject, pushInto)
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
      return {
        kind: "stopped",
        title: pushInto === null ? `Canceled the ${formatPushMode(mode)} of ${subject} before it started` : `Canceled the push of ${subject} into ${pushInto}`,
        description: pushInto === null ? "Nothing was pushed or started" : "Nothing was pushed",
      }
    default:
      return { kind: "error", title, error }
  }
}

import { Channel } from "@tauri-apps/api/core"
import type {
  AirLaunchAnswer,
  AirLaunchRequest,
  AirMode,
  AirModel,
  AirSessionActivity,
  AirSessionStatus,
  AirStartKind,
  AirTarget,
  AirTargetsAnswer,
  AirWatchEnd,
} from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { commandFailure } from "@/lib/errors"

/*
 * Start in Air (spec/branch/start-in-air.spec.md): the state of the dialog over the agents of the IDE (`targets`), the
 * request of a launch, the choice the dialog remembers per repository, the words of a refusal of the IDE, and the
 * words of the status of the session a launch started (`watch`). The IDE decides what it accepts; the dialog only
 * offers what the targets list and names what the IDE answered.
 */

/**
 * What the dialog remembers per repository (`ProjectMetadata.airLaunch`): the last agent, model, effort, mode, Plan and
 * plan effort, and start; a choice of an older version has no mode, Plan and plan effort
 */
export interface AirLaunchChoice {
  launchTargetId: string
  interactionSurfaceId: string
  /** The model id; null for a target without models */
  model: string | null
  /** The effort id; null keeps the default of the agent */
  effort: string | null
  /** The mode id; null keeps the default of the agent */
  mode?: string | null
  plan?: boolean
  /** The effort id of Plan; null keeps the default of the agent */
  planEffort?: string | null
  start: AirStartKind
}

/** The pickers of the dialog */
export interface AirLaunchForm {
  /** The chosen target (`targetKey`) */
  target: string
  model: string | null
  effort: string | null
  /** A mode of `modesOf` the target; kept while Plan is a mode and on, but not sent */
  mode: string | null
  /** The Plan switch, only for a target that can start in Plan */
  plan: boolean
  /** Only while Plan is on */
  planEffort: string | null
  start: AirStartKind
}

/** One key of a target: two targets can have the same agent on another surface */
export function targetKey(target: Pick<AirTarget, "launchTargetId" | "interactionSurfaceId">): string {
  return `${target.launchTargetId}\n${target.interactionSurfaceId}`
}

export function findTarget(targets: readonly AirTarget[], key: string): AirTarget | undefined {
  return targets.find(target => targetKey(target) === key)
}

function modelsOf(target: AirTarget): AirModel[] {
  return target.models ?? []
}

/** The efforts the model takes; none for a target without models: then only the default of the agent is offered */
export function effortsOf(target: AirTarget, model: string | null): string[] {
  return modelsOf(target).find(it => it.id === model)?.reasoningEfforts ?? []
}

/** The modes of the picker: the mode of kind `plan` is the Plan switch */
export function modesOf(target: AirTarget): AirMode[] {
  return (target.modes ?? []).filter(it => it.kind !== "plan")
}

/** Plan of the target is a mode: with Plan on, the launch names no other mode */
export function isPlanMode(target: AirTarget): boolean {
  return (target.modes ?? []).some(it => it.kind === "plan")
}

/** The mode picker is off: Plan is on, and Plan of the target is a mode */
export function isModeLocked(form: AirLaunchForm, target: AirTarget): boolean {
  return form.plan && isPlanMode(target)
}

/** The mode kept while the target offers it, else the default of the agent */
function modeFor(target: AirTarget, mode: string | null | undefined): string | null {
  return mode != null && modesOf(target).some(it => it.id === mode) ? mode : null
}

/** Plan kept while the target can start in Plan; its effort only with Plan and while the model takes it */
function planFor(target: AirTarget, model: string | null, plan: boolean | undefined, planEffort: string | null | undefined): Pick<AirLaunchForm, "plan" | "planEffort"> {
  const on = plan === true && target.plan === true
  return { plan: on, planEffort: on ? effortFor(target, model, planEffort ?? null) : null }
}

/** The model a target starts with: the remembered one while the target still lists it, else its default, else its first */
function modelFor(target: AirTarget, remembered: string | null): string | null {
  const models = modelsOf(target)
  return (models.find(it => it.id === remembered) ?? models.find(it => it.isDefault === true) ?? models[0])?.id ?? null
}

/** The effort kept while the model takes it, else the default of the agent */
function effortFor(target: AirTarget, model: string | null, effort: string | null): string | null {
  return effort !== null && effortsOf(target, model).includes(effort) ? effort : null
}

/**
 * The pickers as the dialog opens: the remembered target while the IDE lists it, else the first; its remembered model
 * and effort while they are still offered. None without a target
 */
export function initialForm(targets: readonly AirTarget[], remembered: AirLaunchChoice | undefined): AirLaunchForm | null {
  const target = (remembered === undefined ? undefined : findTarget(targets, targetKey(remembered))) ?? targets[0]
  if (target === undefined) {
    return null
  }
  const sameTarget = remembered !== undefined && targetKey(remembered) === targetKey(target)
  const model = modelFor(target, sameTarget ? remembered.model : null)
  return {
    target: targetKey(target),
    model,
    effort: effortFor(target, model, sameTarget ? remembered.effort : null),
    mode: modeFor(target, sameTarget ? remembered.mode : null),
    ...planFor(target, model, sameTarget ? remembered.plan : false, sameTarget ? remembered.planEffort : null),
    start: remembered?.start ?? "run",
  }
}

/** Another target: its default model, the effort and the plan effort while that model takes them, the mode and Plan while the target offers them */
export function withTarget(form: AirLaunchForm, targets: readonly AirTarget[], key: string): AirLaunchForm {
  const target = findTarget(targets, key)
  if (target === undefined) {
    return form
  }
  const model = modelFor(target, null)
  return { ...form, target: key, model, effort: effortFor(target, model, form.effort), mode: modeFor(target, form.mode), ...planFor(target, model, form.plan, form.planEffort) }
}

/** Another model: the effort and the plan effort while the model takes them */
export function withModel(form: AirLaunchForm, target: AirTarget, model: string): AirLaunchForm {
  return { ...form, model, effort: effortFor(target, model, form.effort), ...planFor(target, model, form.plan, form.planEffort) }
}

/** Plan on or off: the plan effort starts at the default of the agent */
export function withPlan(form: AirLaunchForm, target: AirTarget, plan: boolean): AirLaunchForm {
  return { ...form, ...planFor(target, form.model, plan, null) }
}

/** The request of a launch; a draft without a prompt sends none, Plan of a mode sends no other mode */
export function launchRequest(project: string, form: AirLaunchForm, target: AirTarget, prompt: string, title: string): AirLaunchRequest {
  const text = prompt.trim() === "" ? null : prompt
  return {
    project,
    launchTargetId: target.launchTargetId,
    interactionSurfaceId: target.interactionSurfaceId,
    model: form.model,
    effort: form.effort,
    mode: isModeLocked(form, target) ? null : form.mode,
    plan: form.plan,
    planEffort: form.plan ? form.planEffort : null,
    prompt: text,
    title,
    start: form.start,
  }
}

/** What the dialog remembers of a launch the IDE accepted */
export function choiceOf(form: AirLaunchForm, target: AirTarget): AirLaunchChoice {
  return {
    launchTargetId: target.launchTargetId,
    interactionSurfaceId: target.interactionSurfaceId,
    model: form.model,
    effort: form.effort,
    mode: form.mode,
    plan: form.plan,
    planEffort: form.planEffort,
    start: form.start,
  }
}

/** A refusal of the IDE, and the form and the prompt it refused: the dialog sends again only after a change */
export interface AirRefusal {
  code: string
  message: string | null
  form: AirLaunchForm
  prompt: string
}

/** Why Start is off now, in the words of the dialog; null when it can start */
export function formatStartHint(form: AirLaunchForm, prompt: string, refusal: AirRefusal | null): string | null {
  if (form.start === "run" && prompt.trim() === "") {
    return "Write a prompt to run the session, or start a draft"
  }
  const same = refusal !== null && refusal.prompt === prompt && JSON.stringify(refusal.form) === JSON.stringify(form)
  return same ? "Change a choice of the agent or the prompt to try again" : null
}

const EFFORT_NAMES: Record<string, string> = { minimal: "Minimal", low: "Low", medium: "Medium", high: "High", xhigh: "Extra High", max: "Max" }

/** "High" of `high`; an effort of a later model as its id */
export function formatEffort(effort: string): string {
  return EFFORT_NAMES[effort] ?? effort
}

/** "GPT-5.5 (preview)": the name with what tells two models of the same name apart */
export function formatModel(model: AirModel): string {
  return model.qualifier == null ? model.name : `${model.name} (${model.qualifier})`
}

/** The IDE of the answer: "IU-263.1234" */
export function formatIde(ide: { product: string; build: string }): string {
  return ide.build === "" ? ide.product : ide.build
}

const REFUSALS: Record<string, string> = {
  DENIED: "The IDE denied Branch Deck, or nobody answered its question in time",
  UNAVAILABLE: "The IDE cannot ask for consent: it runs without a window",
  BUSY: "The IDE answers another request now. Try again in a moment",
  RATE_LIMITED: "Branch Deck asked the IDE too often in the last minute. Try again later",
  UNSUPPORTED_VERSION: "The IDE speaks another version of the launch protocol. Update Air or Branch Deck",
  UNKNOWN_METHOD: "The IDE does not know this request. Update Air",
  BAD_REQUEST: "The IDE cannot read the request of Branch Deck",
  NO_PROJECT: "No project open in the IDE holds the repository",
  UNTRUSTED_PROJECT: "The IDE does not trust the project. Trust it in the IDE first",
  TARGET_UNAVAILABLE: "The agent is not available in the IDE now",
  MODEL_UNAVAILABLE: "The agent has no such model now",
  MODEL_AMBIGUOUS: "Several models of the agent have this name",
  INVALID_REASONING_EFFORT: "Air does not know this effort",
  EMPTY_PROMPT: "The prompt is empty",
  LAUNCH_FAILED: "The session did not start",
  MODE_UNAVAILABLE: "The agent has no such mode now",
  MODE_CONFLICT: "The agent cannot start in Plan with another mode",
  PLAN_UNAVAILABLE: "The agent cannot start in Plan now",
  INVALID_PLAN_REASONING_EFFORT: "Air does not know this plan effort",
  NO_SESSION: "The IDE has no such session",
}

/** A refusal of the IDE by its code; the message of the IDE only where the code says to read it, or for an unknown code */
export function formatRefusal(code: string, message: string | null): string {
  const words = REFUSALS[code]
  if (words === undefined) {
    return message === null ? `The IDE refused the request (${code})` : `The IDE refused the request (${code}): ${message}`
  }
  return code === "LAUNCH_FAILED" && message !== null ? `${words}: ${message}` : words
}

/** The agents of the IDE that has `project` open; the IDE may ask the user first */
export async function readAirTargets(project: string): Promise<AirTargetsAnswer> {
  const result = await commands.airLaunchTargets({ project })
  if (result.status === "error") {
    throw commandFailure(result.error)
  }
  return result.data
}

/** Starts the session in the IDE, or opens a draft of it; the IDE may ask the user first */
export async function launchInAir(request: AirLaunchRequest): Promise<AirLaunchAnswer> {
  const result = await commands.airLaunch(request)
  if (result.status === "error") {
    throw commandFailure(result.error)
  }
  return result.data
}

/** What the toast of a watched session says, in the words of its activity */
export interface AirSessionToast {
  type: "loading" | "info" | "success"
  title: string
  /** The session is done: the watch stops */
  done: boolean
}

/** The toast of the status `activity` of the session of `agent`: "working", "waits for you" or "done" */
export function formatSessionStatus(agent: string, activity: AirSessionActivity): AirSessionToast {
  switch (activity) {
    case "ready":
    case "unread":
      return { type: "success", title: `${agent} is done in Air`, done: true }
    case "needs_input":
    case "draft":
      return { type: "info", title: `${agent} waits for you in Air`, done: false }
    case "reviewing":
      return { type: "info", title: `${agent} waits for your review in Air`, done: false }
    default:
      // `processing`, and an activity of a later protocol
      return { type: "loading", title: `${agent} works in Air…`, done: false }
  }
}

/** The toast of a watch that ended before the session was done; null when the toast keeps the last status */
export function formatWatchEnd(agent: string, end: AirWatchEnd): { type: "info" | "warning"; title: string; description?: string } | null {
  switch (end.status) {
    case "stopped":
      return null
    case "refused":
      return end.code === "NO_SESSION"
        ? { type: "info", title: `The session of ${agent} is gone in Air` }
        : { type: "warning", title: `Cannot follow ${agent} in Air`, description: formatRefusal(end.code, end.message) }
    case "ended":
      return {
        type: "info",
        title: `Stopped following ${agent} in Air`,
        description: end.reason === "timeout" ? "The IDE ends a watch after 4 hours" : `The IDE ended the watch (${end.reason})`,
      }
    case "closed":
      return { type: "info", title: `Stopped following ${agent} in Air`, description: "The IDE closed the connection" }
    case "noIde":
      return { type: "info", title: `Stopped following ${agent} in Air`, description: "No IDE with Air has the repository open now" }
  }
}

/**
 * Watches the session `sessionId` of the IDE that has `project` open: `onStatus` gets each status, now and after each
 * change, until the IDE ends the watch or `stopAirWatch`
 */
export async function watchAirSession(project: string, sessionId: string, onStatus: (status: AirSessionStatus) => void): Promise<AirWatchEnd> {
  const statuses = new Channel<AirSessionStatus>()
  statuses.onmessage = onStatus
  const result = await commands.airWatchSession({ project, sessionId }, statuses)
  if (result.status === "error") {
    throw commandFailure(result.error)
  }
  return result.data
}

/** Stops the watch of the session `sessionId` of this window; the watch answers `stopped` */
export function stopAirWatch(sessionId: string): void {
  void commands.stopAirWatch(sessionId)
}

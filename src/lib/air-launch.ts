import type { AirLaunchAnswer, AirLaunchRequest, AirModel, AirStartKind, AirTarget, AirTargetsAnswer } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { commandFailure } from "@/lib/errors"

/*
 * Start in Air (spec/branch/start-in-air.spec.md): the state of the dialog over the agents of the IDE (`targets`), the
 * request of a launch, the choice the dialog remembers per repository, and the words of a refusal of the IDE. The IDE
 * decides what it accepts; the dialog only offers what the targets list and names what the IDE answered.
 */

/** What the dialog remembers per repository (`ProjectMetadata.airLaunch`): the last agent, model, effort and start */
export interface AirLaunchChoice {
  launchTargetId: string
  interactionSurfaceId: string
  /** The model id; null for a target without models */
  model: string | null
  /** The effort id; null keeps the default of the agent */
  effort: string | null
  start: AirStartKind
}

/** The pickers of the dialog */
export interface AirLaunchForm {
  /** The chosen target (`targetKey`) */
  target: string
  model: string | null
  effort: string | null
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
    start: remembered?.start ?? "run",
  }
}

/** Another target: its default model, the effort while that model takes it */
export function withTarget(form: AirLaunchForm, targets: readonly AirTarget[], key: string): AirLaunchForm {
  const target = findTarget(targets, key)
  if (target === undefined) {
    return form
  }
  const model = modelFor(target, null)
  return { ...form, target: key, model, effort: effortFor(target, model, form.effort) }
}

/** Another model: the effort while the model takes it */
export function withModel(form: AirLaunchForm, target: AirTarget, model: string): AirLaunchForm {
  return { ...form, model, effort: effortFor(target, model, form.effort) }
}

/** The request of a launch; a draft without a prompt sends none */
export function launchRequest(project: string, form: AirLaunchForm, target: AirTarget, prompt: string, title: string): AirLaunchRequest {
  const text = prompt.trim() === "" ? null : prompt
  return {
    project,
    launchTargetId: target.launchTargetId,
    interactionSurfaceId: target.interactionSurfaceId,
    model: form.model,
    effort: form.effort,
    prompt: text,
    title,
    start: form.start,
  }
}

/** What the dialog remembers of a launch the IDE accepted */
export function choiceOf(form: AirLaunchForm, target: AirTarget): AirLaunchChoice {
  return { launchTargetId: target.launchTargetId, interactionSurfaceId: target.interactionSurfaceId, model: form.model, effort: form.effort, start: form.start }
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
  return same ? "Change the agent, the model, the effort or the prompt to try again" : null
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

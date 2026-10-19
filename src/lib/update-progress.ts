import type { TransferProgress } from "@/lib/bindings"

/*
 * One progress for Update Project, from the click to its end: the fetch, the wait for the repository lock, the check,
 * the update of the files and the sync of an applied update. The share of a step is its usual time in the repository
 * (the median of its last runs); inside a step the progress is git's where git reports one (the fetch) or the sync's
 * (its commits), else the time: `1 − e^(−t/usual)`, 63 % at the usual time, never still. A step reaches its end only
 * when it ends (`STEP_CEILING`), and at every change of step the rest of the bar is shared again among the steps left,
 * so the bar never jumps nor goes back.
 */

/** A step of an update, in order; `waiting` only while another operation holds the repository */
export type UpdateStep = "fetch" | "waiting" | "check" | "apply" | "sync"

/** A step with a share of the bar: the wait for the lock has no usual time, the bar stands still meanwhile */
export type TimedStep = Exclude<UpdateStep, "waiting">

/** The order of the steps that have a share */
const ORDER: readonly TimedStep[] = ["fetch", "check", "apply", "sync"]

/** The last durations of each step (ms) in a repository, oldest first */
export type UpdateTimings = Partial<Record<TimedStep, number[]>>

/** How many durations of a step an estimate keeps: one slow fetch shifts the median of five by one place */
export const TIMING_SAMPLES = 5

/** How far a step gets before it ends: the last part of its share waits for the end */
export const STEP_CEILING = 0.95

/** Before a step ran in the repository (ms) */
const DEFAULT_DURATIONS: Readonly<Record<TimedStep, number>> = { fetch: 3000, check: 1500, apply: 800, sync: 2000 }

/** The usual duration of a step in the repository (ms): the median of its last runs, a default before the first */
export function usualDuration(timings: UpdateTimings | undefined, step: TimedStep): number {
  const samples = timings?.[step]
  if (samples === undefined || samples.length === 0) {
    return DEFAULT_DURATIONS[step]
  }
  const sorted = samples.toSorted((a, b) => a - b)
  const middle = Math.floor(sorted.length / 2)
  return sorted.length % 2 === 1 ? sorted[middle]! : (sorted[middle - 1]! + sorted[middle]!) / 2
}

/** The timings with one more duration of the step, the oldest beyond `TIMING_SAMPLES` dropped */
export function addTiming(timings: UpdateTimings | undefined, step: TimedStep, duration: number): UpdateTimings {
  return { ...timings, [step]: [...(timings?.[step] ?? []), Math.round(duration)].slice(-TIMING_SAMPLES) }
}

/** A step that ended, with what it did, e.g. "5.79 MiB" of a fetch */
export interface FinishedStep {
  readonly step: UpdateStep
  readonly duration: number
  readonly detail: string | null
}

/** What an update or a fetch of the baseline (the Fetch of a merge toast) is at; immutable, `advanceRun` makes the next */
export interface UpdateRun {
  /** An update, or only a fetch of the baseline */
  readonly kind: "update" | "fetch"
  readonly step: UpdateStep
  /** When the step started (ms) */
  readonly stepStartedAt: number
  /** The steps with a share, in order, those done included: all of them from the start of the run */
  readonly plan: readonly TimedStep[]
  /** The progress when the step started, 0 to 1: the rest of the bar is shared among the steps left */
  readonly base: number
  readonly done: readonly FinishedStep[]
  /** The usual duration of each step, as the run started (ms) */
  readonly usual: Readonly<Record<TimedStep, number>>
  /** How far the fetch is; null before git reports a stage and after the fetch */
  readonly fetch: TransferProgress | null
}

/** A run that starts with its first step now */
export function startRun(kind: UpdateRun["kind"], plan: readonly TimedStep[], timings: UpdateTimings | undefined, now: number): UpdateRun {
  return {
    kind,
    step: plan[0] ?? "fetch",
    stepStartedAt: now,
    plan,
    base: 0,
    done: [],
    usual: { fetch: usualDuration(timings, "fetch"), check: usualDuration(timings, "check"), apply: usualDuration(timings, "apply"), sync: usualDuration(timings, "sync") },
    fetch: null,
  }
}

/** The steps of the plan after `step`: those still to run */
export function stepsAfter(plan: readonly TimedStep[], step: UpdateStep): TimedStep[] {
  // the wait for the lock sits between the fetch and the check
  const position = step === "waiting" ? ORDER.indexOf("fetch") : ORDER.indexOf(step)
  return plan.filter(it => ORDER.indexOf(it) > position)
}

/** How far the step is, 0 to `STEP_CEILING`: the larger of git's (or the sync's) progress and the time's */
export function stepFraction(run: UpdateRun, now: number, real: number | null): number {
  if (run.step === "waiting") {
    return 0
  }
  const byTime = 1 - Math.exp(-Math.max(now - run.stepStartedAt, 0) / Math.max(run.usual[run.step], 1))
  return Math.min(Math.max(byTime, real ?? 0), STEP_CEILING)
}

/** The progress of the step itself where it reports one: the fetch its percent, the sync its commits (`syncFraction`) */
function realFraction(run: UpdateRun, syncFraction: number | null): number | null {
  switch (run.step) {
    case "fetch":
      return run.fetch === null ? null : run.fetch.percent / 100
    case "sync":
      return syncFraction
    default:
      return null
  }
}

/** The progress of the whole run, 0 to 1; never 1 before the run ends */
export function overallProgress(run: UpdateRun, now: number, syncFraction: number | null = null): number {
  if (run.step === "waiting") {
    return run.base
  }
  const current = run.usual[run.step]
  const rest = stepsAfter(run.plan, run.step).reduce((sum, step) => sum + run.usual[step], 0)
  return run.base + ((1 - run.base) * current * stepFraction(run, now, realFraction(run, syncFraction))) / (current + rest)
}

/**
 * The run at its next step, now: the step that ends is done with its detail, and the rest of the bar from where it
 * stands is shared among the steps of the plan left
 */
export function advanceRun(run: UpdateRun, step: UpdateStep, now: number, detail: string | null = null, syncFraction: number | null = null): UpdateRun {
  return {
    ...run,
    step,
    stepStartedAt: now,
    base: overallProgress(run, now, syncFraction),
    done: [...run.done, { step: run.step, duration: now - run.stepStartedAt, detail }],
    fetch: null,
  }
}

/** The run with its last step done: what a run learns from is in `done` */
export function finishRun(run: UpdateRun, now: number, detail: string | null = null): readonly FinishedStep[] {
  return [...run.done, { step: run.step, duration: now - run.stepStartedAt, detail }]
}

/**
 * The timings with what the steps of a run took; the wait for the lock is not a step of its own repository's usual time,
 * and the sync records its own time (`runSync`), also when no update asked for it
 */
export function learnTimings(timings: UpdateTimings | undefined, steps: readonly FinishedStep[]): UpdateTimings | undefined {
  let result = timings
  for (const { step, duration } of steps) {
    if (step !== "waiting" && step !== "sync") {
      result = addTiming(result, step, duration)
    }
  }
  return result
}

/** The percent the label shows: whole, and 100 only at the end */
export function progressPercent(progress: number): number {
  return Math.min(Math.floor(progress * 100), 99)
}

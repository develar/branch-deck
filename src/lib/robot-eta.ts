import type { PatronusRobot } from "@/lib/bindings"
import { formatPushMode } from "@/lib/patronus"

/** An estimate learns from the robots started this long before now: as long as the history of finished robots keeps them */
export const ROBOT_HISTORY_MS = 30 * 24 * 60 * 60_000
/** With this many finished robots into the same target branch, only they count: another target runs other checks */
export const SAME_TARGET_MIN_ROBOTS = 3
/** With this many finished robots of the same merge profile, only they count: the profile decides the checks */
export const SAME_PROFILE_MIN_ROBOTS = 3

const MINUTE = 60
const HOUR = 60 * MINUTE

/** A robot an estimate learns from: one the app listed, or one of the history of finished robots (`robot-history.ts`) */
export interface RobotSample {
  readonly id: string
  readonly status: string
  readonly pushMode: string
  readonly targetBranch: string
  /** Unix seconds */
  readonly startTime: number
  readonly finishTime: number | null
  /** The merge profile id; null: no profile; undefined: not known (a list doesn't name it) */
  readonly profileId?: string | null
}

/** A listed robot as a sample */
export function robotSample(robot: PatronusRobot): RobotSample {
  return {
    id: robot.id,
    status: robot.status,
    pushMode: robot.pushMode,
    targetBranch: robot.targetBranch,
    startTime: robot.startTime,
    finishTime: robot.finishTime,
    ...(robot.profile != null && { profileId: robot.profile.id }),
  }
}

/**
 * What an estimate is the usual time of: the robots of the same merge profile (`id` null: those without one), or, as
 * the rough fallback, of the same push mode
 */
export type EstimateBasis
  = | { readonly kind: "profile", readonly id: string | null, readonly name: string | null }
    | { readonly kind: "mode" }

/** How long a robot usually runs */
export interface RobotEstimate {
  readonly seconds: number
  readonly basis: EstimateBasis
}

/**
 * How long a robot usually runs, in seconds: the median duration of the successful robots of its merge profile and push
 * mode started in the last 30 days, when 3 or more of them are known (only those into its target branch when there are
 * 3 or more); else, roughly, of its push mode: the profile of the robot is not known yet (a list doesn't name it), or
 * too few robots of the profile finished. The samples are the robots the app already listed and the history of
 * finished ones: an estimate asks Patronus nothing. A failed robot stops early and says nothing about a full run. null
 * without one: the run is indeterminate.
 */
export function estimateRobotDuration(
  robot: Pick<PatronusRobot, "pushMode" | "targetBranch" | "profile">,
  samples: Iterable<RobotSample>,
  now: number,
): RobotEstimate | null {
  const since = (now - ROBOT_HISTORY_MS) / 1000
  // one sample per robot, the one that knows the profile
  const byId = new Map<string, RobotSample>()
  for (const it of samples) {
    if (it.status !== "success" || it.pushMode !== robot.pushMode || it.finishTime === null || it.startTime < since || it.finishTime <= it.startTime) {
      continue
    }
    const known = byId.get(it.id)
    if (known === undefined || (known.profileId === undefined && it.profileId !== undefined)) {
      byId.set(it.id, it)
    }
  }
  const profile = robot.profile
  if (profile != null) {
    const sameProfile = [...byId.values()].filter(it => it.profileId !== undefined && it.profileId === profile.id)
    if (sameProfile.length >= SAME_PROFILE_MIN_ROBOTS) {
      return { seconds: medianDuration(sameProfile, robot.targetBranch)!, basis: { kind: "profile", id: profile.id, name: profile.name } }
    }
  }
  const seconds = medianDuration([...byId.values()], robot.targetBranch)
  return seconds === null ? null : { seconds, basis: { kind: "mode" } }
}

/** The median duration of `samples`, of those into `targetBranch` when there are enough */
function medianDuration(samples: readonly RobotSample[], targetBranch: string): number | null {
  const sameTarget = samples.filter(it => it.targetBranch === targetBranch)
  return median((sameTarget.length >= SAME_TARGET_MIN_ROBOTS ? sameTarget : samples).map(it => it.finishTime! - it.startTime))
}

function median(values: number[]): number | null {
  if (values.length === 0) {
    return null
  }
  values.sort((a, b) => a - b)
  const middle = Math.floor(values.length / 2)
  return values.length % 2 === 1 ? values[middle]! : Math.round((values[middle - 1]! + values[middle]!) / 2)
}

/** Where a running robot is, in seconds */
export interface RobotTiming {
  /** Since the start */
  readonly elapsed: number
  /** The usual duration (`estimateRobotDuration`); null without history */
  readonly expected: number | null
  /** What the usual duration is of; null without history */
  readonly basis: EstimateBasis | null
  /** Until the usual duration; null without history and once past it */
  readonly remaining: number | null
  /** `elapsed / expected` from 0 to 1; null without history and once past it: the progress is indeterminate then */
  readonly progress: number | null
}

/** The timing of a running robot at `now` (milliseconds) */
export function getRobotTiming(robot: Pick<PatronusRobot, "startTime">, estimate: RobotEstimate | null, now: number): RobotTiming {
  const elapsed = Math.max(0, Math.floor(now / 1000) - robot.startTime)
  const expected = estimate?.seconds ?? null
  const basis = estimate?.basis ?? null
  if (expected === null || elapsed >= expected) {
    return { elapsed, expected, basis, remaining: null, progress: null }
  }
  return { elapsed, expected, basis, remaining: expected - elapsed, progress: elapsed / expected }
}

/** The usual duration is known and the robot runs past it */
export function isRobotOverdue(timing: RobotTiming): boolean {
  return timing.expected !== null && timing.remaining === null
}

/**
 * A duration in the slot of the row: "<1m", "18m", "1h 05m", "2h", and from 10 hours "14h" (the minutes don't matter
 * then). Tabular figures keep its width while it counts.
 */
export function formatShortDuration(seconds: number): string {
  if (seconds < MINUTE) {
    return "<1m"
  }
  const hours = Math.floor(seconds / HOUR)
  const minutes = Math.floor((seconds % HOUR) / MINUTE)
  if (hours === 0) {
    return `${minutes}m`
  }
  return hours >= 10 || minutes === 0 ? `${hours}h` : `${hours}h ${String(minutes).padStart(2, "0")}m`
}

/** A duration in a sentence: "less than a minute", "18 min", "1 h 5 min", "14 h" */
export function formatDuration(seconds: number): string {
  if (seconds < MINUTE) {
    return "less than a minute"
  }
  const hours = Math.floor(seconds / HOUR)
  const minutes = Math.floor((seconds % HOUR) / MINUTE)
  if (hours === 0) {
    return `${minutes} min`
  }
  return hours >= 10 || minutes === 0 ? `${hours} h` : `${hours} h ${minutes} min`
}

/** A duration for the screen reader: "18 minutes", "1 hour 5 minutes", "14 hours" */
function formatSpokenDuration(seconds: number): string {
  if (seconds < MINUTE) {
    return "less than a minute"
  }
  const hours = Math.floor(seconds / HOUR)
  const minutes = Math.floor((seconds % HOUR) / MINUTE)
  const hourText = hours === 1 ? "1 hour" : `${hours} hours`
  const minuteText = minutes === 1 ? "1 minute" : `${minutes} minutes`
  if (hours === 0) {
    return minuteText
  }
  return hours >= 10 || minutes === 0 ? hourText : `${hourText} ${minuteText}`
}

/** The remaining time rounded up: a robot one second short of its usual time has "~1m left", not "<1m left" */
function roundUpToMinute(seconds: number): number {
  return Math.ceil(seconds / MINUTE) * MINUTE
}

/** The slot text of a running robot: how long it runs, then how long it usually takes ("18m", "~20m left") */
export interface RobotProgressText {
  readonly elapsed: string
  /** null without history */
  readonly eta: string | null
}

/**
 * The slot text of a running robot: "18m · ~20m left"; past its usual time the usual time instead, "55m · usually ~40m"
 * (the hours only from an hour, "14h · usually ~1h": the minutes no longer matter, and the slot has a fixed width, too
 * narrow for "longer than usual", which the details and the screen reader say); without history "18m".
 */
export function formatRobotProgress(timing: RobotTiming): RobotProgressText {
  if (timing.remaining !== null) {
    return { elapsed: formatShortDuration(timing.elapsed), eta: `~${formatShortDuration(roundUpToMinute(timing.remaining))} left` }
  }
  if (timing.expected !== null) {
    const elapsed = timing.elapsed >= HOUR ? `${Math.floor(timing.elapsed / HOUR)}h` : formatShortDuration(timing.elapsed)
    return { elapsed, eta: `usually ~${formatShortDuration(timing.expected)}` }
  }
  return { elapsed: formatShortDuration(timing.elapsed), eta: null }
}

/** What the screen reader says after the state: "18 minutes, about 20 minutes left", "14 hours, longer than usual" */
export function formatSpokenRobotProgress(timing: RobotTiming): string {
  const elapsed = formatSpokenDuration(timing.elapsed)
  if (timing.remaining !== null) {
    return `${elapsed}, about ${formatSpokenDuration(roundUpToMinute(timing.remaining))} left`
  }
  return isRobotOverdue(timing) ? `${elapsed}, longer than usual` : elapsed
}

/**
 * What the usual time is of, after "usually ~40 min": "for Large Scale Change" (the robots of its profile), "without a
 * merge profile", or, roughly, "for Safe Push" (of its push mode)
 */
function formatEstimateBasis(robot: Pick<PatronusRobot, "pushMode">, basis: EstimateBasis): string {
  if (basis.kind === "mode") {
    return `for ${formatPushMode(robot.pushMode)}`
  }
  if (basis.name !== null) {
    return `for ${basis.name}`
  }
  return basis.id === null ? "without a merge profile" : "for its merge profile"
}

/**
 * The timing after the start time in the details of a running robot, with what the estimate is of: "usually ~1 h 50
 * min for Large Scale Change · about 22 min left", the rough "usually ~40 min for Safe Push · 55 min, longer than
 * usual", and without history "running 18 min · no Safe Push finished in the last 30 days to estimate from"
 */
export function formatRobotTimingDetails(robot: Pick<PatronusRobot, "pushMode">, timing: RobotTiming): string {
  if (timing.expected === null || timing.basis === null) {
    return `running ${formatDuration(timing.elapsed)} · no ${formatPushMode(robot.pushMode)} finished in the last 30 days to estimate from`
  }
  const usually = `usually ~${formatDuration(timing.expected)} ${formatEstimateBasis(robot, timing.basis)}`
  return timing.remaining === null
    ? `${usually} · ${formatDuration(timing.elapsed)}, longer than usual`
    : `${usually} · about ${formatDuration(roundUpToMinute(timing.remaining))} left`
}

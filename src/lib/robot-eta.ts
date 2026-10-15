import type { PatronusRobot } from "@/lib/bindings"
import { formatPushMode } from "@/lib/patronus"

/** An estimate learns from the robots started this long before now, as long as the first list of Patronus reaches */
export const ROBOT_HISTORY_MS = 7 * 24 * 60 * 60_000
/** With this many finished robots into the same target branch, only they count: another target runs other checks */
export const SAME_TARGET_MIN_ROBOTS = 3

const MINUTE = 60
const HOUR = 60 * MINUTE

/**
 * How long a robot usually runs, in seconds: the median duration of the successful robots of its push mode started in
 * the last 7 days (only those into its target branch when there are 3 or more), from the robots the app already listed,
 * so an estimate asks Patronus nothing. A failed robot stops early and says nothing about a full run. null without one:
 * the run is indeterminate.
 */
export function expectedRobotDuration(robot: Pick<PatronusRobot, "pushMode" | "targetBranch">, robots: Iterable<PatronusRobot>, now: number): number | null {
  const since = (now - ROBOT_HISTORY_MS) / 1000
  const all: number[] = []
  const sameTarget: number[] = []
  for (const it of robots) {
    if (it.status !== "success" || it.pushMode !== robot.pushMode || it.finishTime === null || it.startTime < since || it.finishTime <= it.startTime) {
      continue
    }
    const duration = it.finishTime - it.startTime
    all.push(duration)
    if (it.targetBranch === robot.targetBranch) {
      sameTarget.push(duration)
    }
  }
  return median(sameTarget.length >= SAME_TARGET_MIN_ROBOTS ? sameTarget : all)
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
  /** The usual duration (`expectedRobotDuration`); null without history */
  readonly expected: number | null
  /** Until the usual duration; null without history and once past it */
  readonly remaining: number | null
  /** `elapsed / expected` from 0 to 1; null without history and once past it: the progress is indeterminate then */
  readonly progress: number | null
}

/** The timing of a running robot at `now` (milliseconds) */
export function getRobotTiming(robot: Pick<PatronusRobot, "startTime">, expected: number | null, now: number): RobotTiming {
  const elapsed = Math.max(0, Math.floor(now / 1000) - robot.startTime)
  if (expected === null || elapsed >= expected) {
    return { elapsed, expected, remaining: null, progress: null }
  }
  return { elapsed, expected, remaining: expected - elapsed, progress: elapsed / expected }
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
 * The timing after the start time in the details of a running robot: "usually ~40 min · about 22 min left", "usually
 * ~40 min · 14 h, longer than usual", and without history "running 18 min · no Safe Push finished in the last 7 days to
 * estimate from"
 */
export function formatRobotTimingDetails(robot: Pick<PatronusRobot, "pushMode">, timing: RobotTiming): string {
  if (timing.expected === null) {
    return `running ${formatDuration(timing.elapsed)} · no ${formatPushMode(robot.pushMode)} finished in the last 7 days to estimate from`
  }
  const usually = `usually ~${formatDuration(timing.expected)}`
  return timing.remaining === null
    ? `${usually} · ${formatDuration(timing.elapsed)}, longer than usual`
    : `${usually} · about ${formatDuration(roundUpToMinute(timing.remaining))} left`
}

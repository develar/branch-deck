import type { PageHealth } from "../helpers/page-health"
import type { MachineSample } from "./machine-sampler"

/** An action of a test (a click, a press) with when it started (ms since the epoch) and the worker that ran it */
export interface TimedAction {
  test: string
  action: string
  ms: number
  start: number
  worker: number
}

/**
 * What stood beside a stalled action, and what it says of it (`verdict`): its wall time and worker; the load average
 * and what the machine swapped while it lasted; the slow actions of the other workers in the same time; the long tasks
 * of its own pages that overlapped it and, for a failed test, whether its pages answered at the end
 */
export interface StallContext {
  from: number
  to: number
  worker: number
  load: [number, number] | null
  swapins: number | null
  decompressions: number | null
  beside: TimedAction[]
  longTasks: [end: number, ms: number][]
  answered: boolean | null
  verdict: string
}

/** The machine is the cause of a stall that this many slow actions of other workers stood beside */
const BESIDE_THE_MACHINE = 2

/**
 * The context of `stall` among `actions` (of every worker), the `samples` of the machine and the `health` of the pages of
 * its test; an action of another worker is slow from `slowMs`
 */
export function stallContext(
  stall: TimedAction,
  actions: readonly TimedAction[],
  samples: readonly MachineSample[],
  slowMs: number,
  sampleMs: number,
  health: PageHealth | undefined,
): StallContext {
  const from = stall.start
  const to = stall.start + stall.ms
  // the samples of the stall, and the one before and after it: a stall shorter than the interval still has its load
  const within = samples.filter(sample => sample.at >= from - sampleMs && sample.at <= to + sampleMs)
  const loads = within.map(sample => sample.load)
  const delta = (counter: "swapins" | "decompressions") => {
    const counted = within.filter(sample => sample[counter] !== undefined)
    return counted.length < 2 ? null : counted[counted.length - 1]![counter]! - counted[0]![counter]!
  }
  const beside = actions.filter(action => action.worker !== stall.worker && action.ms >= slowMs && action.start < to && action.start + action.ms > from).sort((a, b) => b.ms - a.ms)
  const longTasks = (health?.longTasks ?? []).filter(([end, ms]) => end - ms < to && end > from)
  const context = {
    from,
    to,
    worker: stall.worker,
    load: loads.length === 0 ? null : ([Math.min(...loads), Math.max(...loads)] as [number, number]),
    swapins: delta("swapins"),
    decompressions: delta("decompressions"),
    beside,
    longTasks,
    answered: health?.answered ?? null,
  }
  return { ...context, verdict: verdictOf(stall, context) }
}

/**
 * The page or the machine, in one line: a page whose main thread did not answer, or a long task of its own over half of
 * the stall, is the page; slow actions of other workers beside it or a swap are the machine; with neither, the action
 * waited for something of its own (an overlay over its target, a page that did not take it)
 */
export function verdictOf(stall: TimedAction, context: Omit<StallContext, "verdict">): string {
  const seconds = (ms: number) => `${(ms / 1000).toFixed(1)} s`
  if (context.answered === false) {
    return "the page: it did not answer before the end of its test, its main thread stuck in a task"
  }
  const longest = context.longTasks.reduce((max, [, ms]) => Math.max(max, ms), 0)
  if (longest >= stall.ms / 2) {
    return `the page: a long task of ${seconds(longest)} stood in it`
  }
  if (context.beside.length >= BESIDE_THE_MACHINE || (context.swapins ?? 0) > 0) {
    const swap = (context.swapins ?? 0) > 0 ? `, the machine swapped in ${context.swapins} pages` : ""
    return `the machine: ${context.beside.length} slow actions of other workers stood beside it${swap}`
  }
  return "neither: no long task of its page and no stall of the machine beside it; an overlay over its target, or a page that did not take it"
}

/** The lines of a stalled action in the summary of a run: the action, then its verdict and what it rests on */
export function describeStall(stall: TimedAction, context: StallContext, seconds: (ms: number) => string): string {
  const clock = (ms: number) => new Date(ms).toTimeString().slice(0, 8)
  const parts = [`worker ${context.worker}`, `${clock(context.from)}–${clock(context.to)}`]
  if (context.load !== null) {
    parts.push(`load ${context.load[0].toFixed(0)}–${context.load[1].toFixed(0)}`)
  }
  if (context.swapins !== null) {
    parts.push(`swap-ins +${context.swapins}`)
  }
  if (context.decompressions !== null) {
    parts.push(`decompressions +${context.decompressions}`)
  }
  parts.push(context.longTasks.length === 0 ? "no long task of its pages" : `long tasks ${context.longTasks.map(([, ms]) => seconds(ms)).join(", ")}`)
  if (context.answered !== null) {
    parts.push(context.answered ? "its pages answered" : "a page did not answer")
  }
  parts.push(
    context.beside.length === 0
      ? "no slow action of another worker"
      : `beside it ${context.beside
          .slice(0, 3)
          .map(action => `${seconds(action.ms)} ${action.action.slice(0, 40)} (worker ${action.worker})`)
          .join(", ")}${context.beside.length > 3 ? ` and ${context.beside.length - 3} more` : ""}`,
  )
  return `  ${seconds(stall.ms)} ${stall.action.slice(0, 100)} in ${stall.test}\n      → ${context.verdict}\n        ${parts.join(", ")}`
}

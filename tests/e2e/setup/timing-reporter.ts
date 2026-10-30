import { writeFileSync } from "fs"
import type { FullResult, Reporter, TestCase, TestResult, TestStep } from "@playwright/test/reporter"

/** How many of the slowest tests and stalls the summary prints; the JSON keeps four times as many */
const SLOWEST = 5

/**
 * An action is a stall when it took this long and this many times the median action of the same run: a click under
 * a toast that waits for it to time out, a hover over an element that moves. The ratio keeps a loaded machine, where
 * every action is slower, from flagging all of them
 */
const STALL_MS = 2_000
const STALL_RATIO = 20

/** The steps of the actions of a test by the start of their title: their duration is the wait of the action itself */
const ACTIONS = ["Click", "Double click", "Hover", "Fill", "Press", "Type", "Check", "Uncheck", "Select option", "Drag and drop"]

function isAction(step: TestStep): boolean {
  return step.category === "pw:api" && ACTIONS.some(name => step.title === name || step.title.startsWith(`${name} `))
}

interface Stall {
  test: string
  action: string
  ms: number
}

/**
 * The steps of a test in `tests/results/steps.json`, each `[offset from the start of the test, duration, depth,
 * "category: title", "file:line"]` (ms): where the time of a slow test went, step by step, which its total does not say
 */
interface Profile {
  title: string
  status: string
  start: number
  ms: number
  steps: [number, number, number, string, string][]
}

/**
 * The timing of a run, printed at its end and written to `tests/results/timing.json`: the start (global setup, the
 * server, the workers) and the tests, the slowest tests, and the actions that stalled. A test that gets slow, or a
 * click that starts to wait for an overlay, shows in every run instead of in a sum nobody reads. The steps of every
 * test go to `tests/results/steps.json` (`Profile`)
 */
export default class TimingReporter implements Reporter {
  private readonly started = Date.now()
  private firstTest = Infinity
  private lastEnd = 0
  private readonly tests: { title: string; ms: number }[] = []
  private readonly actions: { test: string; action: string; ms: number }[] = []
  private readonly profiles: Profile[] = []

  onTestEnd(test: TestCase, result: TestResult) {
    const title = `${test.location.file.replace(/^.*tests\/e2e\//, "")} › ${test.titlePath().slice(3).join(" › ")}`
    const start = result.startTime.getTime()
    this.firstTest = Math.min(this.firstTest, start)
    this.lastEnd = Math.max(this.lastEnd, start + result.duration)
    this.tests.push({ title, ms: result.duration })
    const steps: Profile["steps"] = []
    const walk = (list: TestStep[], depth: number) => {
      for (const step of list) {
        if (isAction(step)) {
          this.actions.push({ test: title, action: step.title, ms: step.duration })
        }
        const where = step.location === undefined ? "" : `${step.location.file.replace(/^.*tests\/e2e\//, "")}:${step.location.line}`
        steps.push([step.startTime.getTime() - start, step.duration, depth, `${step.category}: ${step.title.slice(0, 160)}`, where])
        walk(step.steps, depth + 1)
      }
    }
    walk(result.steps, 0)
    this.profiles.push({ title: `${title} (line ${test.location.line})`, status: result.status, start: start - this.started, ms: result.duration, steps })
  }

  onEnd(result: FullResult) {
    if (this.tests.length === 0) {
      return
    }
    const durations = this.actions.map(it => it.ms).sort((a, b) => a - b)
    const median = durations[Math.floor(durations.length / 2)] ?? 0
    const stalls: Stall[] = this.actions.filter(it => it.ms >= STALL_MS && it.ms >= median * STALL_RATIO).sort((a, b) => b.ms - a.ms)
    const slowest = [...this.tests].sort((a, b) => b.ms - a.ms).slice(0, SLOWEST * 4)
    const testTime = this.tests.reduce((sum, it) => sum + it.ms, 0)
    const timing = {
      status: result.status,
      startMs: this.firstTest - this.started,
      testsMs: this.lastEnd - this.firstTest,
      testTimeMs: testTime,
      tests: this.tests.length,
      medianActionMs: median,
      slowest,
      stalls: stalls.slice(0, SLOWEST * 4),
    }
    try {
      writeFileSync("tests/results/timing.json", JSON.stringify(timing, null, 2))
      writeFileSync("tests/results/steps.json", JSON.stringify(this.profiles))
    } catch {
      // no results directory: a run that listed nothing
    }
    const s = (ms: number) => `${(ms / 1000).toFixed(1)} s`
    const lines = [
      `Timing: start ${s(timing.startMs)}, tests ${s(timing.testsMs)}; ${this.tests.length} tests, ${s(testTime)} of test time`,
      `Slowest: ${slowest
        .slice(0, SLOWEST)
        .map(it => `${s(it.ms)} ${it.title}`)
        .join("\n         ")}`,
    ]
    if (stalls.length > 0) {
      lines.push(
        `Stalled actions (≥ ${s(STALL_MS)} and ${STALL_RATIO}× the median action, ${median} ms; an overlay over the target, e.g. a toast to close):`,
        ...stalls.slice(0, SLOWEST).map(it => `  ${s(it.ms)} ${it.action.slice(0, 100)} in ${it.test}`),
      )
    }
    console.log(`\n${lines.join("\n")}`)
  }
}

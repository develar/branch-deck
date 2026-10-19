// @spec spec/repository/update-project.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { TransferProgress } from "@/lib/bindings"
import type { UpdateRun } from "@/lib/update-progress"
import { STEP_CEILING, addTiming, advanceRun, finishRun, learnTimings, overallProgress, startRun, usualDuration } from "@/lib/update-progress"

const FULL = ["fetch", "check", "apply", "sync"] as const

/** A run whose steps usually take: fetch 3 s, check 1.5 s, apply 0.8 s, sync 2 s (the defaults) */
function run(): UpdateRun {
  return startRun("update", FULL, undefined, 0)
}

function fetched(percent: number): TransferProgress {
  return { percent, transferred: null, speed: null, line: `Receiving objects: ${percent}%` }
}

/** The progress sampled every 100 ms from `from` to `to` */
function samples(at: (now: number) => number, from: number, to: number): number[] {
  const result: number[] = []
  for (let now = from; now <= to; now += 100) {
    result.push(at(now))
  }
  return result
}

function isNonDecreasing(values: readonly number[]): boolean {
  return values.every((value, index) => index === 0 || value >= values[index - 1]! - 1e-9)
}

describe("the progress of an update", () => {
  it("shares the bar by the usual time of each step in the repository: the median of the last five", () => {
    expect(usualDuration(undefined, "fetch")).toBe(3000)
    let timings = addTiming(undefined, "fetch", 1000)
    for (const duration of [9000, 1200, 1100, 60_000, 1300]) {
      timings = addTiming(timings, "fetch", duration)
    }
    // the first one dropped out; one week-long fetch shifts the median by one place, not the estimate by a minute
    expect(timings.fetch).toEqual([9000, 1200, 1100, 60_000, 1300])
    expect(usualDuration(timings, "fetch")).toBe(1300)
  })

  it("moves with the time in a step that reports nothing, never standing still, and keeps the end of a step for its end", () => {
    const checking = advanceRun(run(), "check", 3000)
    const usual = samples(now => overallProgress(checking, now), 3000, 3000 + 1500)
    const late = samples(now => overallProgress(checking, now), 6000, 9000)
    expect(isNonDecreasing([...usual, ...late])).toBe(true)
    // past its usual time it still moves, slower, and never reaches the share of the step
    expect(late.at(-1)!).toBeGreaterThan(late[0]!)
    const share = (1 - checking.base) * (1500 / (1500 + 800 + 2000))
    expect(late.at(-1)! - checking.base).toBeLessThanOrEqual(share * STEP_CEILING + 1e-9)
  })

  it("takes git's percent in the fetch when it is ahead of the time", () => {
    const started = run()
    expect(overallProgress({ ...started, fetch: fetched(80) }, 100)).toBeCloseTo((0.8 * 3000) / 7300)
    // never the end of the fetch before it ends
    expect(overallProgress({ ...started, fetch: fetched(100) }, 100)).toBeCloseTo((STEP_CEILING * 3000) / 7300)
  })

  it("goes on from where it stands at each change of step: no jump after a slow fetch, no stall after a fast one", () => {
    for (const fetchEnd of [500, 9000]) {
      const fetching = run()
      const before = overallProgress(fetching, fetchEnd)
      const checking = advanceRun(fetching, "check", fetchEnd)
      expect(overallProgress(checking, fetchEnd)).toBeCloseTo(before)
      expect(overallProgress(checking, fetchEnd + 200)).toBeGreaterThan(before)
    }
  })

  it("stands still while it waits for the lock", () => {
    const waiting = advanceRun(run(), "waiting", 2000)
    const atWait = overallProgress(waiting, 2000)
    expect(overallProgress(waiting, 8000)).toBe(atWait)
  })

  it("takes the commits of its own sync, capped until the sync ends", () => {
    const syncing = advanceRun(advanceRun(advanceRun(run(), "check", 3000), "apply", 4500), "sync", 5300)
    const start = overallProgress(syncing, 5300, null)
    expect(overallProgress(syncing, 5300, 0.5)).toBeCloseTo(start + (1 - syncing.base) * 0.5)
    expect(overallProgress(syncing, 5300, 1)).toBeCloseTo(syncing.base + (1 - syncing.base) * STEP_CEILING)
    expect(overallProgress(syncing, 5300, 1)).toBeLessThan(1)
  })

  it("learns what the steps took, without the wait for the lock and the sync, which learns from every sync", () => {
    const fetching = run()
    const waiting = advanceRun(fetching, "waiting", 2000)
    const checking = advanceRun(waiting, "check", 5000)
    const syncing = advanceRun(advanceRun(checking, "apply", 6000), "sync", 6500)
    expect(learnTimings(undefined, finishRun(syncing, 9000))).toEqual({ fetch: [2000], check: [1000], apply: [500] })
  })
})

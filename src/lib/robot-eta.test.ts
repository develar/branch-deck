import { describe, expect, it } from "vitest"
import type { PatronusRobot, PatronusRobotProfile } from "@/lib/bindings"
import type { RobotEstimate, RobotSample, RobotTiming } from "@/lib/robot-eta"
import { estimateRobotDuration, formatDuration, formatRobotProgress, formatRobotTimingDetails, formatShortDuration, formatSpokenRobotProgress, getRobotTiming, isRobotOverdue, robotSample } from "@/lib/robot-eta"

/** 2024-01-02 10:00Z, the clock of the E2E tests */
const NOW = 1704189600_000
const NOW_SECONDS = NOW / 1000

function finished(id: string, minutes: number, patch: Partial<PatronusRobot> = {}): PatronusRobot {
  const startTime = NOW_SECONDS - 24 * 3600
  return {
    id,
    name: id,
    status: "success",
    pushMode: "rebaseAutosquash",
    branchName: id,
    sourceRevision: "tip",
    targetBranch: "master",
    startTime,
    finishTime: startTime + minutes * 60,
    cancellationReason: null,
    url: `https://patronus/robot/${id}`,
    ...patch,
  }
}

const LSC: PatronusRobotProfile = { id: "lsc", name: "Large Scale Change" }

function samples(...robots: PatronusRobot[]): RobotSample[] {
  return robots.map(robotSample)
}

const running = { pushMode: "rebaseAutosquash", targetBranch: "master", profile: null } as const

/** An estimate of the push mode, as the fallback has it */
function rough(minutes: number): RobotEstimate {
  return { seconds: minutes * 60, basis: { kind: "mode" } }
}

/** The slot text as it reads */
function progressText(timing: RobotTiming): string {
  const { elapsed, eta } = formatRobotProgress(timing)
  return eta === null ? elapsed : `${elapsed} · ${eta}`
}

describe("estimateRobotDuration", () => {
  it("takes the median of the successful robots of the push mode started in the last 30 days", () => {
    const robots = samples(
      finished("a", 30),
      finished("b", 40),
      finished("c", 90),
      // a failure stops early, a dry run runs other checks, an old robot ran another month
      finished("failed", 5, { status: "failure" }),
      finished("dry", 10, { pushMode: "dryRun" }),
      finished("old", 200, { startTime: NOW_SECONDS - 31 * 24 * 3600, finishTime: NOW_SECONDS - 31 * 24 * 3600 + 200 * 60 }),
      finished("running", 0, { status: "running", finishTime: null }),
    )
    expect(estimateRobotDuration(running, robots, NOW)).toEqual(rough(40))
    // an even count: the mean of the middle two
    expect(estimateRobotDuration(running, samples(finished("a", 30), finished("b", 40)), NOW)).toEqual(rough(35))
    expect(estimateRobotDuration({ ...running, pushMode: "dryRun" }, robots, NOW)).toEqual(rough(10))
  })

  it("is indeterminate without a finished robot to learn from", () => {
    expect(estimateRobotDuration(running, [], NOW)).toBeNull()
    expect(estimateRobotDuration(running, samples(finished("failed", 5, { status: "failure" })), NOW)).toBeNull()
  })

  it("counts only the robots into the same target once there are enough of them", () => {
    const releases = [finished("r1", 120, { targetBranch: "release" }), finished("r2", 130, { targetBranch: "release" })]
    const masters = [finished("m1", 30), finished("m2", 40), finished("m3", 50)]
    expect(estimateRobotDuration(running, samples(...releases, ...masters), NOW)).toEqual(rough(40))
    // two robots into the release branch are too few: every target counts
    expect(estimateRobotDuration({ ...running, targetBranch: "release" }, samples(...releases, ...masters), NOW)).toEqual(rough(50))
  })

  it("takes the robots of the same merge profile once three of them finished, and those without one for a robot without one", () => {
    const robots = samples(
      finished("d1", 30, { profile: { id: null, name: null } }),
      finished("d2", 40, { profile: { id: null, name: null } }),
      finished("d3", 45, { profile: { id: null, name: null } }),
      finished("l1", 100, { profile: LSC }),
      finished("l2", 110, { profile: LSC }),
      finished("l3", 120, { profile: LSC }),
      // a listed robot whose profile no answer named yet
      finished("u1", 60),
    )
    expect(estimateRobotDuration({ ...running, profile: LSC }, robots, NOW)).toEqual({ seconds: 110 * 60, basis: { kind: "profile", id: "lsc", name: "Large Scale Change" } })
    expect(estimateRobotDuration({ ...running, profile: { id: null, name: null } }, robots, NOW)).toEqual({ seconds: 40 * 60, basis: { kind: "profile", id: null, name: null } })
  })

  it("falls back to the push mode, roughly, while the profile is not known or too few of its robots finished", () => {
    const robots = samples(finished("l1", 100, { profile: LSC }), finished("l2", 110, { profile: LSC }), finished("m1", 30), finished("m2", 40))
    // not known: a list doesn't name it
    expect(estimateRobotDuration(running, robots, NOW)).toEqual(rough(70))
    // two robots of the profile are too few
    expect(estimateRobotDuration({ ...running, profile: LSC }, robots, NOW)).toEqual(rough(70))
  })

  it("takes the sample that knows the profile, of a robot both listed and in the history", () => {
    const listed = samples(finished("l1", 100), finished("l2", 110), finished("l3", 120))
    const history = listed.map(it => ({ ...it, profileId: "lsc" }))
    expect(estimateRobotDuration({ ...running, profile: LSC }, [...listed, ...history], NOW)?.basis).toEqual({ kind: "profile", id: "lsc", name: "Large Scale Change" })
  })
})

describe("getRobotTiming", () => {
  it("counts the time left until the usual duration, indeterminate past it and without history", () => {
    const robot = { startTime: NOW_SECONDS - 18 * 60 }
    expect(getRobotTiming(robot, rough(40), NOW)).toEqual({ elapsed: 18 * 60, expected: 40 * 60, basis: { kind: "mode" }, remaining: 22 * 60, progress: 18 / 40 })
    const overdue = getRobotTiming(robot, rough(10), NOW)
    expect(overdue).toEqual({ elapsed: 18 * 60, expected: 10 * 60, basis: { kind: "mode" }, remaining: null, progress: null })
    expect(isRobotOverdue(overdue)).toBe(true)
    const indeterminate = getRobotTiming(robot, null, NOW)
    expect(indeterminate.progress).toBeNull()
    expect(isRobotOverdue(indeterminate)).toBe(false)
    // a start time ahead of the local clock
    expect(getRobotTiming({ startTime: NOW_SECONDS + 5 }, null, NOW).elapsed).toBe(0)
  })
})

describe("formatRobotProgress", () => {
  it("says how long it runs and how long it usually takes", () => {
    const start = { startTime: NOW_SECONDS - 18 * 60 }
    expect(progressText(getRobotTiming(start, rough(40), NOW))).toBe("18m · ~22m left")
    expect(formatSpokenRobotProgress(getRobotTiming(start, rough(40), NOW))).toBe("18 minutes, about 22 minutes left")
    // the remaining time rounds up
    expect(progressText(getRobotTiming(start, { seconds: 18 * 60 + 1, basis: { kind: "mode" } }, NOW))).toBe("18m · ~1m left")
    expect(progressText(getRobotTiming(start, null, NOW))).toBe("18m")
    expect(formatSpokenRobotProgress(getRobotTiming(start, null, NOW))).toBe("18 minutes")
  })

  it("names the usual time once a robot runs longer, in hours once it runs an hour", () => {
    expect(progressText(getRobotTiming({ startTime: NOW_SECONDS - 45 * 60 }, rough(40), NOW))).toBe("45m · usually ~40m")
    const fourteenHours = getRobotTiming({ startTime: NOW_SECONDS - 14 * 3600 }, rough(60), NOW)
    expect(progressText(fourteenHours)).toBe("14h · usually ~1h")
    expect(formatSpokenRobotProgress(fourteenHours)).toBe("14 hours, longer than usual")
    expect(progressText(getRobotTiming({ startTime: NOW_SECONDS - 65 * 60 }, rough(60), NOW))).toBe("1h · usually ~1h")
  })

  it("explains the estimate and what it is of in the details", () => {
    const start = { startTime: NOW_SECONDS - 18 * 60 }
    const lsc: RobotEstimate = { seconds: 110 * 60, basis: { kind: "profile", id: "lsc", name: "Large Scale Change" } }
    expect(formatRobotTimingDetails(running, getRobotTiming(start, lsc, NOW))).toBe("usually ~1 h 50 min for Large Scale Change · about 1 h 32 min left")
    expect(formatRobotTimingDetails(running, getRobotTiming(start, { seconds: 40 * 60, basis: { kind: "profile", id: null, name: null } }, NOW))).toBe("usually ~40 min without a merge profile · about 22 min left")
    // the rough one names the push mode
    expect(formatRobotTimingDetails(running, getRobotTiming(start, rough(40), NOW))).toBe("usually ~40 min for Safe Push · about 22 min left")
    expect(formatRobotTimingDetails(running, getRobotTiming(start, rough(10), NOW))).toBe("usually ~10 min for Safe Push · 18 min, longer than usual")
    expect(formatRobotTimingDetails(running, getRobotTiming(start, null, NOW))).toBe("running 18 min · no Safe Push finished in the last 30 days to estimate from")
  })
})

describe("formatShortDuration", () => {
  it("is compact, with the minutes of the first ten hours", () => {
    expect(formatShortDuration(30)).toBe("<1m")
    expect(formatShortDuration(18 * 60 + 59)).toBe("18m")
    expect(formatShortDuration(65 * 60)).toBe("1h 05m")
    expect(formatShortDuration(120 * 60)).toBe("2h")
    expect(formatShortDuration(14 * 3600 + 5 * 60)).toBe("14h")
    expect(formatDuration(65 * 60)).toBe("1 h 5 min")
    expect(formatDuration(120 * 60)).toBe("2 h")
    expect(formatDuration(30)).toBe("less than a minute")
  })
})

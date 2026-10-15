import { describe, expect, it } from "vitest"
import type { PatronusRobot } from "@/lib/bindings"
import type { RobotTiming } from "@/lib/robot-eta"
import { expectedRobotDuration, formatDuration, formatRobotProgress, formatRobotTimingDetails, formatShortDuration, formatSpokenRobotProgress, getRobotTiming, isRobotOverdue } from "@/lib/robot-eta"

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

const running = { pushMode: "rebaseAutosquash", targetBranch: "master" } as const

/** The slot text as it reads */
function progressText(timing: RobotTiming): string {
  const { elapsed, eta } = formatRobotProgress(timing)
  return eta === null ? elapsed : `${elapsed} · ${eta}`
}

describe("expectedRobotDuration", () => {
  it("takes the median of the successful robots of the push mode started in the last 7 days", () => {
    const robots = [
      finished("a", 30),
      finished("b", 40),
      finished("c", 90),
      // a failure stops early, a dry run runs other checks, an old robot ran another week
      finished("failed", 5, { status: "failure" }),
      finished("dry", 10, { pushMode: "dryRun" }),
      finished("old", 200, { startTime: NOW_SECONDS - 8 * 24 * 3600, finishTime: NOW_SECONDS - 8 * 24 * 3600 + 200 * 60 }),
      finished("running", 0, { status: "running", finishTime: null }),
    ]
    expect(expectedRobotDuration(running, robots, NOW)).toBe(40 * 60)
    // an even count: the mean of the middle two
    expect(expectedRobotDuration(running, [finished("a", 30), finished("b", 40)], NOW)).toBe(35 * 60)
    expect(expectedRobotDuration({ ...running, pushMode: "dryRun" }, robots, NOW)).toBe(10 * 60)
  })

  it("is indeterminate without a finished robot to learn from", () => {
    expect(expectedRobotDuration(running, [], NOW)).toBeNull()
    expect(expectedRobotDuration(running, [finished("failed", 5, { status: "failure" })], NOW)).toBeNull()
  })

  it("counts only the robots into the same target once there are enough of them", () => {
    const releases = [finished("r1", 120, { targetBranch: "release" }), finished("r2", 130, { targetBranch: "release" })]
    const masters = [finished("m1", 30), finished("m2", 40), finished("m3", 50)]
    expect(expectedRobotDuration(running, [...releases, ...masters], NOW)).toBe(40 * 60)
    // two robots into the release branch are too few: every target counts
    expect(expectedRobotDuration({ ...running, targetBranch: "release" }, [...releases, ...masters], NOW)).toBe(50 * 60)
  })
})

describe("getRobotTiming", () => {
  it("counts the time left until the usual duration, indeterminate past it and without history", () => {
    const robot = { startTime: NOW_SECONDS - 18 * 60 }
    expect(getRobotTiming(robot, 40 * 60, NOW)).toEqual({ elapsed: 18 * 60, expected: 40 * 60, remaining: 22 * 60, progress: 18 / 40 })
    const overdue = getRobotTiming(robot, 10 * 60, NOW)
    expect(overdue).toEqual({ elapsed: 18 * 60, expected: 10 * 60, remaining: null, progress: null })
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
    expect(progressText(getRobotTiming(start, 40 * 60, NOW))).toBe("18m · ~22m left")
    expect(formatSpokenRobotProgress(getRobotTiming(start, 40 * 60, NOW))).toBe("18 minutes, about 22 minutes left")
    // the remaining time rounds up
    expect(progressText(getRobotTiming(start, 18 * 60 + 1, NOW))).toBe("18m · ~1m left")
    expect(progressText(getRobotTiming(start, null, NOW))).toBe("18m")
    expect(formatSpokenRobotProgress(getRobotTiming(start, null, NOW))).toBe("18 minutes")
  })

  it("names the usual time once a robot runs longer, in hours once it runs an hour", () => {
    expect(progressText(getRobotTiming({ startTime: NOW_SECONDS - 45 * 60 }, 40 * 60, NOW))).toBe("45m · usually ~40m")
    const fourteenHours = getRobotTiming({ startTime: NOW_SECONDS - 14 * 3600 }, 60 * 60, NOW)
    expect(progressText(fourteenHours)).toBe("14h · usually ~1h")
    expect(formatSpokenRobotProgress(fourteenHours)).toBe("14 hours, longer than usual")
    expect(progressText(getRobotTiming({ startTime: NOW_SECONDS - 65 * 60 }, 60 * 60, NOW))).toBe("1h · usually ~1h")
  })

  it("explains the estimate in the details", () => {
    const start = { startTime: NOW_SECONDS - 18 * 60 }
    expect(formatRobotTimingDetails(running, getRobotTiming(start, 40 * 60, NOW))).toBe("usually ~40 min · about 22 min left")
    expect(formatRobotTimingDetails(running, getRobotTiming(start, 10 * 60, NOW))).toBe("usually ~10 min · 18 min, longer than usual")
    expect(formatRobotTimingDetails(running, getRobotTiming(start, null, NOW))).toBe("running 18 min · no Safe Push finished in the last 7 days to estimate from")
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

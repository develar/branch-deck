import { describe, expect, it } from "vitest"
import type { PatronusRobot } from "@/lib/bindings"
import { findBranchRobot, formatPatronusAvailabilityHint, getPatronusChipState, needsPatronusAttention } from "@/lib/patronus"

function robot(patch: Partial<PatronusRobot>): PatronusRobot {
  return {
    id: "r1",
    name: "IJPL-1 fix the cache",
    status: "running",
    pushMode: "rebaseAutosquash",
    branchName: "IJPL-1",
    sourceRevision: "tip",
    targetBranch: "master",
    startTime: 100,
    finishTime: null,
    cancellationReason: null,
    url: "https://patronus/robot/r1",
    ...patch,
  }
}

function byId(...robots: PatronusRobot[]): Record<string, PatronusRobot> {
  return Object.fromEntries(robots.map(it => [it.id, it]))
}

describe("findBranchRobot", () => {
  it("matches a robot started on the branch, and a Safe Push of a temporary ref by the local or the pushed tip", () => {
    const onBranch = robot({ id: "branch" })
    const cliPush = robot({ id: "cli", branchName: null, sourceRevision: "pushed" })
    const otherBranch = robot({ id: "other", branchName: "IJPL-2", sourceRevision: "tip" })
    expect(findBranchRobot(byId(cliPush, otherBranch), "IJPL-1", "local", "pushed")).toBe(cliPush)
    expect(findBranchRobot(byId(cliPush, otherBranch), "IJPL-1", "pushed", null)).toBe(cliPush)
    // a robot of another virtual branch never belongs to this one, even on the same commit
    expect(findBranchRobot(byId(otherBranch), "IJPL-1", "tip", null)).toBeUndefined()
    expect(findBranchRobot(byId(onBranch, otherBranch), "IJPL-1", "anything", null)).toBe(onBranch)
    expect(findBranchRobot(undefined, "IJPL-1", "tip", null)).toBeUndefined()
  })

  it("prefers a running robot, then the newest", () => {
    const failed = robot({ id: "failed", status: "failure", startTime: 300 })
    const running = robot({ id: "running", status: "canceling", startTime: 200 })
    const older = robot({ id: "older", status: "success", startTime: 100 })
    expect(findBranchRobot(byId(failed, running, older), "IJPL-1", "tip", null)).toBe(running)
    expect(findBranchRobot(byId(failed, older), "IJPL-1", "tip", null)).toBe(failed)
  })
})

describe("getPatronusChipState", () => {
  it("names the state in its hue; a successful dry run passed, a successful push merged", () => {
    expect(getPatronusChipState(robot({ status: "running" }), "tip")).toEqual({ kind: "running", color: "neutral", label: "Running", outdated: false })
    expect(getPatronusChipState(robot({ status: "canceling" }), "tip").label).toBe("Canceling")
    expect(getPatronusChipState(robot({ status: "failure" }), "tip")).toMatchObject({ kind: "failed", color: "error", label: "Failed" })
    expect(getPatronusChipState(robot({ status: "success" }), "tip")).toMatchObject({ kind: "merged", color: "info", label: "Merged" })
    expect(getPatronusChipState(robot({ status: "success", pushMode: "dryRun" }), "tip")).toMatchObject({ kind: "passed", color: "success", label: "Dry run passed" })
    expect(getPatronusChipState(robot({ status: "canceled" }), "tip")).toMatchObject({ kind: "canceled", color: "neutral" })
  })

  it("marks a robot that tests another commit than the local tip, except a merged one", () => {
    expect(getPatronusChipState(robot({ status: "failure" }), "newer").outdated).toBe(true)
    expect(getPatronusChipState(robot({ status: "running" }), undefined).outdated).toBe(false)
    expect(getPatronusChipState(robot({ status: "success" }), "newer").outdated).toBe(false)
  })
})

describe("needsPatronusAttention", () => {
  it("is set by a failed robot of the commit the branch is at", () => {
    expect(needsPatronusAttention(robot({ status: "failure" }), "tip")).toBe(true)
    expect(needsPatronusAttention(robot({ status: "failure" }), "fixed")).toBe(false)
    expect(needsPatronusAttention(robot({ status: "running" }), "tip")).toBe(false)
  })
})

describe("formatPatronusAvailabilityHint", () => {
  it("hints at what to do, and says nothing when there is nothing to do", () => {
    expect(formatPatronusAvailabilityHint({ status: "notLoggedIn" })).toBe("Run ./safePush.cmd once to log in to Patronus")
    expect(formatPatronusAvailabilityHint({ status: "error", message: "offline" })).toBe("Patronus: offline")
    expect(formatPatronusAvailabilityHint({ status: "available" })).toBeNull()
    expect(formatPatronusAvailabilityHint({ status: "notPatronus" })).toBeNull()
    expect(formatPatronusAvailabilityHint(null)).toBeNull()
  })
})

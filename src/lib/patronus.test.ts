import { describe, expect, it } from "vitest"
import type { PatronusRobot } from "@/lib/bindings"
import { allowsPushMode, canCancelRobot, canRestartRobot, findBranchRobot, formatPatronusAvailabilityHint, formatReviewerGroup, getPatronusChipState, isPushIntoRobotRisky, isStartBlocked, needsPatronusAttention } from "@/lib/patronus"

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

  it("prefers a running robot over a canceling one over a finished one, then the newest", () => {
    const failed = robot({ id: "failed", status: "failure", startTime: 300 })
    const canceling = robot({ id: "canceling", status: "canceling", startTime: 200 })
    const restarted = robot({ id: "restarted", status: "running", startTime: 200 })
    const older = robot({ id: "older", status: "success", startTime: 100 })
    expect(findBranchRobot(byId(failed, canceling, older), "IJPL-1", "tip", null)).toBe(canceling)
    expect(findBranchRobot(byId(canceling, restarted), "IJPL-1", "tip", null)).toBe(restarted)
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

  it("names a robot that failed for missing approvals \"Needs review\"", () => {
    expect(getPatronusChipState(robot({ status: "failure" }), "tip", true)).toMatchObject({ kind: "needsReview", color: "warning", label: "Needs review" })
    // only a failure
    expect(getPatronusChipState(robot({ status: "running" }), "tip", true).kind).toBe("running")
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

describe("actions", () => {
  it("restarts a failed, canceled or canceling robot of the branch, a passed dry run only with a newer pushed commit", () => {
    expect(canRestartRobot(robot({ status: "failure" }), "IJPL-1", null)).toBe(true)
    expect(canRestartRobot(robot({ status: "canceled" }), "IJPL-1", null)).toBe(true)
    expect(canRestartRobot(robot({ status: "canceling" }), "IJPL-1", null)).toBe(true)
    expect(canRestartRobot(robot({ status: "running" }), "IJPL-1", null)).toBe(false)
    expect(canRestartRobot(robot({ status: "success" }), "IJPL-1", "newer")).toBe(false)
    expect(canRestartRobot(robot({ status: "success", pushMode: "dryRun" }), "IJPL-1", "tip")).toBe(false)
    expect(canRestartRobot(robot({ status: "success", pushMode: "dryRun" }), "IJPL-1", "newer")).toBe(true)
    // a Safe Push of a temporary ref restarts its old commit: not offered
    expect(canRestartRobot(robot({ status: "failure", branchName: null }), "IJPL-1", null)).toBe(false)
    // a restart would check the same merge request without approvals
    expect(canRestartRobot(robot({ status: "failure" }), "IJPL-1", null, true)).toBe(false)
  })

  it("cancels a running robot only", () => {
    expect(canCancelRobot(robot({ status: "running" }))).toBe(true)
    expect(canCancelRobot(robot({ status: "canceling" }))).toBe(false)
  })

  it("guards a push into the branch of a running Safe Push, not of a dry run", () => {
    expect(isPushIntoRobotRisky(robot({ status: "running" }), "IJPL-1")).toBe(true)
    expect(isPushIntoRobotRisky(robot({ status: "running", pushMode: "dryRun" }), "IJPL-1")).toBe(false)
    expect(isPushIntoRobotRisky(robot({ status: "failure" }), "IJPL-1")).toBe(false)
    expect(isPushIntoRobotRisky(robot({ status: "running", branchName: null }), "IJPL-1")).toBe(false)
  })

  it("blocks a start while a robot runs on the commit the branch is at", () => {
    expect(isStartBlocked(robot({ status: "running" }), "tip")).toBe(true)
    expect(isStartBlocked(robot({ status: "running" }), "fixed")).toBe(false)
    expect(isStartBlocked(robot({ status: "failure" }), "tip")).toBe(false)
    expect(isStartBlocked(undefined, "tip")).toBe(false)
  })

  it("offers a profile for the modes it allows; none listed allows any", () => {
    expect(allowsPushMode({ id: "a", name: "A", description: null, allowedPushModes: [] }, "dryRun")).toBe(true)
    expect(allowsPushMode({ id: "a", name: "A", description: null, allowedPushModes: ["dryRun"] }, "rebaseAutosquash")).toBe(false)
  })
})

describe("formatReviewerGroup", () => {
  it("names the group and the available reviewers Patronus suggests", () => {
    expect(formatReviewerGroup({
      name: "Air Plugin Release Team",
      reviewers: [
        { fullName: "Alice", username: "alice", available: true, suggested: true },
        { fullName: "Bob", username: "bob", available: false, suggested: true },
        { fullName: "Carol", username: "carol", available: true, suggested: false },
      ],
    })).toBe("Air Plugin Release Team (suggested: Alice)")
    expect(formatReviewerGroup({ name: "Bazel QA", reviewers: [] })).toBe("Bazel QA")
  })
})

import { describe, expect, it } from "vite-plus/test"
import type { RobotCommitMap, RobotCommitState } from "@/lib/bindings"
import { branchCoverageOf, robotCoverageNeed, robotRailMarks } from "@/lib/robot-coverage"
import { testRobot } from "@/lib/test-robot"
import { PUSH, testRemoteStatus } from "@/stores/test-branch"

function states(entries: Record<string, RobotCommitState>): ReadonlyMap<string, RobotCommitState> {
  return new Map(Object.entries(entries))
}

describe("robotCoverageNeed", () => {
  it("covers every commit while the robot took the branch as it is, or the same commits on an older baseline", () => {
    const robot = testRobot("r", "auth")
    expect(robotCoverageNeed(robot, "auth-tip", null)).toBe("all")
    // the sync rebuilt the branch on a newer baseline: new hashes, the commits the remote has
    expect(robotCoverageNeed(robot, "rebuilt", testRemoteStatus({ remoteTip: "auth-tip" }))).toBe("all")
  })

  it("needs the commit map once the branch changed since the robot took it", () => {
    const robot = testRobot("r", "auth")
    expect(robotCoverageNeed(robot, "newer", testRemoteStatus({ remoteTip: "auth-tip", unpushedCommits: ["c"], commitsAhead: 1, action: PUSH }))).toBe("map")
    expect(robotCoverageNeed(robot, "newer", testRemoteStatus({ remoteTip: "newer" }))).toBe("map")
  })

  it("tells nothing without the robot's commit, the tip, or the remote status of a changed branch", () => {
    expect(robotCoverageNeed(testRobot("r", "auth", { sourceRevision: null }), "auth-tip", null)).toBeNull()
    expect(robotCoverageNeed(testRobot("r", "auth"), undefined, null)).toBeNull()
    expect(robotCoverageNeed(testRobot("r", "auth"), "newer", null)).toBeNull()
  })
})

describe("branchCoverageOf", () => {
  it("takes the commits of the branch by their local hash, and none while the robot's commit is not here", () => {
    const map: RobotCommitMap = {
      commits: [
        { localId: "c3", subject: "(auth) c3", authorTime: 3, branch: "auth", state: "added" },
        { localId: "c2", subject: "(auth) c2", authorTime: 2, branch: "auth", state: "tested" },
        { localId: "p1", subject: "(base) p1", authorTime: 1, branch: "base", state: "rebased" },
        { localId: null, subject: "gone", authorTime: 0, branch: null, state: "removed" },
      ],
      sourceMissing: false,
      virtualBehindHead: false,
    }
    expect([...branchCoverageOf(map, "auth")]).toEqual([
      ["c3", "added"],
      ["c2", "tested"],
    ])
    expect([...branchCoverageOf(map, "base")]).toEqual([["p1", "rebased"]])
    expect(branchCoverageOf({ ...map, sourceMissing: true }, "auth").size).toBe(0)
  })
})

describe("robotRailMarks", () => {
  it("runs the line of the robot over the commits it covers and marks the ones added or changed since", () => {
    // newest first: two commits after the robot started, then the one it took and an older one
    const marks = robotRailMarks(["c4", "c3", "c2", "c1"], states({ c4: "added", c3: "changed", c2: "tested", c1: "rebased" }), true)
    expect(marks.get("c4")).toEqual({ dot: "uncovered", change: "added", up: false, down: false })
    expect(marks.get("c3")).toEqual({ dot: "uncovered", change: "changed", up: false, down: false })
    expect(marks.get("c2")).toEqual({ dot: "tip", change: null, up: false, down: true })
    expect(marks.get("c1")).toEqual({ dot: null, change: null, up: false, down: false })
  })

  it("draws the line from the top of the rail down to the last dot while the robot covers every commit", () => {
    const marks = robotRailMarks(["c2", "c1"], "all", true)
    expect(marks.get("c2")).toEqual({ dot: null, change: null, up: true, down: true })
    expect(marks.get("c1")).toEqual({ dot: null, change: null, up: false, down: false })
  })

  it("finds the newest covered commit below the added ones also in a list that is oldest first", () => {
    const marks = robotRailMarks(["c1", "c2", "c3"], states({ c1: "tested", c2: "tested", c3: "added" }), false)
    expect(marks.get("c1")).toEqual({ dot: null, change: null, up: true, down: true })
    expect(marks.get("c2")).toEqual({ dot: "tip", change: null, up: false, down: false })
    expect(marks.get("c3")).toEqual({ dot: "uncovered", change: "added", up: false, down: false })
  })

  it("keeps the plain dot for a commit whose state is not known", () => {
    const marks = robotRailMarks(["c2", "c1"], states({ c1: "unknown" }), true)
    expect(marks.get("c2")).toEqual({ dot: null, change: null, up: false, down: false })
    expect(marks.get("c1")).toEqual({ dot: null, change: null, up: false, down: false })
  })
})

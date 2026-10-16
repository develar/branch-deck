import { openUrl } from "@tauri-apps/plugin-opener"
import { beforeEach, describe, expect, it, vi } from "vitest"
import type { PatronusRobot } from "@/lib/bindings"
import { getRobotActions } from "@/lib/patronus-actions"
import type { Branch } from "@/stores/branch-sync"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

const { calls, landing } = vi.hoisted(() => ({
  /** What reached Patronus and the push, in order */
  calls: [] as string[],
  /** The robot that lands an ancestor of the branch, the robot of a branch built on it that lands it */
  landing: { ancestor: null as unknown, span: null as unknown },
}))

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn(async () => {}) }))
vi.mock("@/stores/activity", () => ({ recordRobot: () => {} }))
vi.mock("@/lib/git/push", () => ({
  pushBranch: async (name: string, options: { restartRobot?: PatronusRobot, announce?: boolean }) => {
    calls.push(`push ${name}, restart ${options.restartRobot?.id}, announce ${options.announce}`)
  },
}))
vi.mock("@/stores/patronus", () => ({
  isRobotNeedingReview: () => false,
  getLandingAncestor: () => landing.ancestor,
  getLandingSpan: () => landing.span,
  restartPatronusRobot: async (robot: PatronusRobot) => {
    calls.push(`restart ${robot.id}`)
    return robot
  },
  cancelPatronusRobot: async (robot: PatronusRobot) => {
    calls.push(`cancel ${robot.id}`)
    return robot
  },
}))

function robot(patch: Partial<PatronusRobot>): PatronusRobot {
  return {
    id: "r1",
    name: "IJPL-1 fix the cache",
    status: "failure",
    pushMode: "rebaseAutosquash",
    branchName: "IJPL-1",
    sourceRevision: "tip",
    targetBranch: "master",
    startTime: 100,
    finishTime: 200,
    cancellationReason: null,
    url: "https://patronus/robot/r1",
    ...patch,
  }
}

/** IJPL-1 at the commit `local` */
function branch(overrides: Partial<Branch>): Branch {
  return testBranch("IJPL-1", null, { commits: [{ hash: "local" } as Branch["commits"][number]], ...overrides })
}

/** The remote has the commit the branch is at, pushed since the robot took `tip` */
const pushedSince = testRemoteStatus({ remoteTip: "local" })
/** The remote is at the commit the robot took, the branch has a commit more */
const unpushed = testRemoteStatus({ remoteTip: "tip", unpushedCommits: ["fix"], commitsAhead: 1, myCommitsAhead: 1 })

function labels(subject: PatronusRobot, of: Branch | null, needsReview?: boolean): string[] {
  return getRobotActions(subject, of, needsReview).map(it => it.disabled === true ? `${it.label} (disabled: ${it.description})` : it.label)
}

beforeEach(() => {
  calls.length = 0
  landing.ancestor = null
  landing.span = null
  vi.mocked(openUrl).mockClear()
})

describe("getRobotActions", () => {
  it("restarts a failed robot, pushing first while the branch has work the remote lacks, and not for a blocked branch", async () => {
    const failed = robot({})
    expect(labels(failed, branch({ remoteStatus: pushedSince }))).toEqual(["Restart Safe Push", "Open in Patronus"])
    expect(labels(failed, branch({ remoteStatus: unpushed }))).toEqual(["Push and Restart Safe Push", "Open in Patronus"])
    expect(labels(robot({ pushMode: "dryRun" }), branch({ remoteStatus: unpushed }))).toEqual(["Push and Restart Dry Run", "Open in Patronus"])
    const blocked = branch({ remoteStatus: unpushed, status: "Blocked", stack: { ...testBranch("IJPL-1", "auth").stack!, blockedBy: { parent: "auth", reason: "Conflict" } } })
    expect(labels(failed, blocked)[0]).toMatch(/^Push and Restart Safe Push \(disabled: .+\)$/)

    const [restart] = getRobotActions(failed, branch({ remoteStatus: pushedSince }))
    restart!.onSelect()
    const [pushAndRestart] = getRobotActions(failed, branch({ remoteStatus: unpushed }))
    pushAndRestart!.onSelect()
    // the restart takes the commit just pushed, and its toast is the one of the action
    await vi.waitFor(() => expect(calls).toEqual(["restart r1", "push IJPL-1, restart r1, announce false"]))
  })

  it("waits for a robot that lands an ancestor of the branch, as a start does; a dry run lands nothing and needn't", () => {
    landing.ancestor = { branch: "auth", robot: robot({ id: "auth-run", status: "running", finishTime: null, branchName: "auth" }) }
    expect(labels(robot({}), branch({ remoteStatus: pushedSince }))[0]).toBe("Restart Safe Push (disabled: A Safe Push of auth is running: wait until it merges)")
    expect(labels(robot({ pushMode: "dryRun" }), branch({ remoteStatus: pushedSince }))[0]).toBe("Restart Dry Run")
  })

  it("cancels a running robot, and opens it in Patronus and the pull request it was started from", async () => {
    const running = robot({ status: "running", finishTime: null, pullRequest: { number: 12, url: "https://github.com/acme/monorepo/pull/12" } })
    const actions = getRobotActions(running, branch({ remoteStatus: pushedSince }))
    expect(actions.map(it => [it.id, it.label])).toEqual([
      ["cancel-robot", "Cancel Safe Push"],
      ["open-robot", "Open in Patronus"],
      ["open-robot-pull-request", "Open Pull Request #12"],
    ])
    for (const action of actions) {
      action.onSelect()
    }
    await vi.waitFor(() => expect(calls).toEqual(["cancel r1"]))
    expect(vi.mocked(openUrl).mock.calls.map(([url]) => url)).toEqual(["https://patronus/robot/r1", "https://github.com/acme/monorepo/pull/12"])
  })

  it("restarts the robot of a branch without a row on its remote branch, never a robot of a temporary ref or one that waits for approvals in a merge request", () => {
    expect(labels(robot({}), null)).toEqual(["Restart Safe Push", "Open in Patronus"])
    expect(labels(robot({ branchName: null }), null)).toEqual(["Open in Patronus"])
    expect(labels(robot({ branchName: null }), branch({ remoteStatus: pushedSince }))).toEqual(["Open in Patronus"])
    // a restart would check the same merge request, which has no approvals
    expect(labels(robot({}), branch({ remoteStatus: pushedSince }), true)).toEqual(["Open in Patronus"])
    // a merged robot is done
    expect(labels(robot({ status: "success" }), branch({ remoteStatus: pushedSince }))).toEqual(["Open in Patronus"])
  })
})

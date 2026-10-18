import { openUrl } from "@tauri-apps/plugin-opener"
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { PatronusRobot } from "@/lib/bindings"
import { getRobotActions } from "@/lib/patronus-actions"
import type { Branch, RemoteStatus, StackBranch } from "@/stores/branch-sync"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

const { calls, landing, stacks } = vi.hoisted(() => ({
  /** What reached Patronus and the push, in order */
  calls: [] as string[],
  /**
   * The robot that lands an ancestor of the branch, the robot of a branch built on it that lands it, who landed a branch
   * (`getLandedBy`) and why every branch of a stack landed (`getStackLandedReason`), by name
   */
  landing: { ancestor: null as unknown, span: null as unknown, by: {} as Record<string, unknown>, stack: {} as Record<string, string> },
  /** The stack branches the sync built, by root */
  stacks: new Map<string, unknown>(),
}))

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn(async () => {}) }))
vi.mock("@/stores/activity", () => ({ recordRobot: () => {} }))
vi.mock("@/lib/git/push", () => ({
  pushBranch: async (name: string, options: { restartRobot?: PatronusRobot; announce?: boolean }) => {
    calls.push(`push ${name}, restart ${options.restartRobot?.id}, announce ${options.announce}`)
  },
}))
vi.mock("@/lib/git/stack-push", () => ({
  pushWholeStack: async (root: string, options: { restartRobot?: PatronusRobot }) => {
    calls.push(`push the whole ${root} stack, restart ${options.restartRobot?.id}`)
  },
}))
vi.mock("@/stores/branch-sync", async importOriginal => ({ ...(await importOriginal()), getStackBranch: (root: string) => stacks.get(root) }))
vi.mock("@/stores/patronus", () => ({
  isRobotNeedingReview: () => false,
  isRobotFailing: () => false,
  getLandingAncestor: () => landing.ancestor,
  getLandingSpan: () => landing.span,
  getLandedBy: (name: string) => landing.by[name] ?? null,
  getStackLandedReason: (root: string) => landing.stack[root] ?? null,
  getStackLandingMember: () => null,
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

/** The prompts of a failed or failing robot, before "Open in Patronus" */
const PROMPTS = ["Copy Prompt for Agent", "Copy Prompt with CI Details"]

function labels(subject: PatronusRobot, of: Branch | null, needsReview?: boolean, failing?: boolean): string[] {
  return getRobotActions(subject, of, needsReview, failing).map(it => (it.disabled === true ? `${it.label} (disabled: ${it.description})` : it.label))
}

/** The stack branch of the whole air stack, built at `stack-local`, the remote at `stack-tip`, where its robot ran */
function airStack(remoteStatus: RemoteStatus): StackBranch {
  return {
    root: "air",
    branchName: "user/stack/air",
    members: ["air", "bazel", "side-fix"],
    commitCount: 3,
    state: "Built",
    tip: "stack-local",
    blockedBy: null,
    conflict: null,
    error: null,
    remoteStatus,
  }
}

beforeEach(() => {
  calls.length = 0
  landing.ancestor = null
  landing.span = null
  landing.by = {}
  landing.stack = {}
  stacks.clear()
  vi.mocked(openUrl).mockClear()
})

describe("getRobotActions", () => {
  it("restarts a failed robot, pushing first while the branch has work the remote lacks, and not for a blocked branch", async () => {
    const failed = robot({})
    expect(labels(failed, branch({ remoteStatus: pushedSince }))).toEqual(["Restart Safe Push", ...PROMPTS, "Open in Patronus"])
    expect(labels(failed, branch({ remoteStatus: unpushed }))).toEqual(["Restart Safe Push", ...PROMPTS, "Open in Patronus"])
    expect(labels(robot({ pushMode: "dryRun" }), branch({ remoteStatus: unpushed }))).toEqual(["Restart Dry Run", ...PROMPTS, "Open in Patronus"])
    const blocked = branch({ remoteStatus: unpushed, status: "Blocked", stack: { ...testBranch("IJPL-1", "auth").stack!, blockedBy: { parent: "auth", reason: "Conflict" } } })
    expect(labels(failed, blocked)[0]).toMatch(/^Restart Safe Push \(disabled: .+\)$/)

    const [restart] = getRobotActions(failed, branch({ remoteStatus: pushedSince }))
    restart!.onSelect()
    const [pushAndRestart] = getRobotActions(failed, branch({ remoteStatus: unpushed }))
    pushAndRestart!.onSelect()
    // the restart takes the commit just pushed, and its toast is the one of the action
    await vi.waitFor(() => expect(calls).toEqual(["restart r1", "push IJPL-1, restart r1, announce false"]))
  })

  it("pushes and restarts a running robot of a branch changed since: it cancels it first", async () => {
    const running = robot({ status: "running", finishTime: null })
    const actions = getRobotActions(running, branch({ remoteStatus: unpushed }))
    expect(actions.map(it => it.id)).toEqual(["push-and-restart-robot", "cancel-robot", "open-robot"])
    expect(actions[0]!.description).toMatch(/^Cancels the run on .+, pushes IJPL-1 and starts it again on the pushed commit$/)
    actions[0]!.onSelect()
    await vi.waitFor(() => expect(calls).toEqual(["push IJPL-1, restart r1, announce false"]))
  })

  it("waits for a robot that lands an ancestor of the branch, as a start does; a dry run lands nothing and needn't", () => {
    landing.ancestor = { branch: "auth", robot: robot({ id: "auth-run", status: "running", finishTime: null, branchName: "auth" }) }
    expect(labels(robot({}), branch({ remoteStatus: pushedSince }))[0]).toBe("Restart Safe Push (disabled: A Safe Push of auth is running: wait until it merges)")
    expect(labels(robot({ pushMode: "dryRun" }), branch({ remoteStatus: pushedSince }))[0]).toBe("Restart Dry Run")
  })

  it("refuses the restart of a branch a merged Safe Push landed whole, a Dry Run too, and restarts one with commits after the landed ones", () => {
    const merged = robot({ id: "oauth-run", branchName: "oauth", status: "success" })
    landing.by["IJPL-1"] = { branch: "oauth", robot: merged, wholeStack: false, own: false, after: [] }
    const reason = "Landed in master with oauth"
    expect(labels(robot({}), branch({ remoteStatus: pushedSince }))[0]).toBe(`Restart Safe Push (disabled: ${reason})`)
    expect(labels(robot({}), branch({ remoteStatus: unpushed }))[0]).toBe(`Restart Safe Push (disabled: ${reason})`)
    expect(labels(robot({ pushMode: "dryRun" }), branch({ remoteStatus: pushedSince }))[0]).toBe(`Restart Dry Run (disabled: ${reason})`)
    // a commit added after the landed ones is what is left: the restart lands it
    landing.by["IJPL-1"] = { branch: "oauth", robot: merged, wholeStack: false, own: false, after: ["local"] }
    expect(labels(robot({}), branch({ remoteStatus: unpushed }))[0]).toBe("Restart Safe Push")
  })

  it("restarts the robot of the whole stack on its stack branch, refused once merged Safe Pushes landed every branch of it, a Dry Run too", async () => {
    const whole = robot({ id: "whole", branchName: null, stackRoot: "air", sourceRevision: "stack-tip" })
    stacks.set("air", airStack(testRemoteStatus({ remoteTip: "stack-local" })))
    expect(labels(whole, null)).toEqual(["Restart Safe Push", ...PROMPTS, "Open in Patronus"])
    stacks.set("air", airStack(testRemoteStatus({ remoteTip: "stack-tip", unpushedCommits: ["s1"], commitsAhead: 1, myCommitsAhead: 1 })))
    const [pushAndRestart] = getRobotActions(whole, null)
    expect(pushAndRestart!.id).toBe("push-and-restart-robot")
    pushAndRestart!.onSelect()
    await vi.waitFor(() => expect(calls).toEqual(["push the whole air stack, restart whole"]))

    // the Safe Pushes of deploy and side-test landed all of it: nothing of it is left to restart, with or without a push
    const reason = "Landed in master with deploy and side-test"
    landing.stack.air = reason
    expect(labels(whole, null)[0]).toBe(`Restart Safe Push (disabled: ${reason})`)
    expect(labels(robot({ id: "dry", branchName: null, stackRoot: "air", pushMode: "dryRun", sourceRevision: "stack-tip" }), null)[0]).toBe(`Restart Dry Run (disabled: ${reason})`)
    stacks.set("air", airStack(testRemoteStatus({ remoteTip: "stack-local" })))
    expect(labels(whole, null)[0]).toBe(`Restart Safe Push (disabled: ${reason})`)
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

  it("offers the prompt for an agent for a failed robot and for a running one whose required check failed", () => {
    const running = robot({ status: "running", finishTime: null })
    expect(labels(running, branch({ remoteStatus: pushedSince }), false, true)).toEqual(["Cancel Safe Push", ...PROMPTS, "Open in Patronus"])
    expect(labels(running, branch({ remoteStatus: pushedSince }), false, false)).toEqual(["Cancel Safe Push", "Open in Patronus"])
    expect(labels(robot({ status: "canceled" }), null)).toEqual(["Restart Safe Push", "Open in Patronus"])
  })

  it("restarts the robot of a branch without a row on its remote branch, never a robot of a temporary ref or one that waits for approvals in a merge request", () => {
    expect(labels(robot({}), null)).toEqual(["Restart Safe Push", ...PROMPTS, "Open in Patronus"])
    expect(labels(robot({ branchName: null }), null)).toEqual([...PROMPTS, "Open in Patronus"])
    expect(labels(robot({ branchName: null }), branch({ remoteStatus: pushedSince }))).toEqual([...PROMPTS, "Open in Patronus"])
    // a restart has no pull request to count the approvals in; no code to fix either
    expect(labels(robot({}), branch({ remoteStatus: pushedSince }), true)).toEqual(["Open in Patronus"])
    // a merged robot is done
    expect(labels(robot({ status: "success" }), branch({ remoteStatus: pushedSince }))).toEqual(["Open in Patronus"])
  })
})

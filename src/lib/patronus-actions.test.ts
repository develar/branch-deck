import { openUrl } from "@tauri-apps/plugin-opener"
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { PatronusRobot, Reason, RobotRestart } from "@/lib/bindings"
import { FROM_MENU } from "@/lib/menu"
import { getRobotActions } from "@/lib/patronus-actions"
import type { Branch } from "@/stores/branch-sync"
import { getOperations } from "@/stores/operations"
import { PUSH, testBranch, testRemoteStatus } from "@/stores/test-branch"

const { calls, restarts } = vi.hoisted(() => ({
  /** What reached Patronus and the push, in order */
  calls: [] as string[],
  /** The restart each robot offers, as Rust decides it of the view (`PatronusView.restarts`), by robot id */
  restarts: new Map<string, RobotRestart>(),
}))

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn(async () => {}) }))
vi.mock("@/stores/activity", () => ({ recordRobot: () => {} }))
// the toast of a robot follows the robot in the stores: these tests are about the actions
vi.mock("@/lib/robot-toast", () => ({ showRobotEventToast: () => {} }))
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
vi.mock("@/stores/patronus", () => ({
  isRobotNeedingReview: () => false,
  isRobotFailing: () => false,
  getRobotRestart: (id: string) => restarts.get(id) ?? null,
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

/** IJPL-1 at the commit `local`, its remote at the commit the robot took, a commit behind */
const ROW: Branch = testBranch("IJPL-1", null, {
  commits: [{ hash: "local" } as Branch["commits"][number]],
  remoteStatus: testRemoteStatus({ remoteTip: "tip", unpushedCommits: ["fix"], commitsAhead: 1, myCommitsAhead: 1, action: PUSH }),
})

/** The prompts of a failed or failing robot, before "Open in Patronus" */
const PROMPTS = ["Copy Prompt for Agent", "Copy Prompt with CI Details"]

function labels(subject: PatronusRobot, of: Branch | null, needsReview?: boolean, failing?: boolean): string[] {
  return getRobotActions(subject, of, needsReview, failing).map(it => (it.disabled === true ? `${it.label} (disabled: ${it.description})` : it.label))
}

/** The robot `id` offers `action`, refused by `refusal` */
function offers(id: string, action: RobotRestart["action"], refusal: Reason | null = null): void {
  restarts.set(id, { action, refusal })
}

beforeEach(() => {
  calls.length = 0
  restarts.clear()
  vi.mocked(openUrl).mockClear()
})

// a restart is an operation of its robot: the next test starts with none running
afterEach(() => vi.waitFor(() => expect(getOperations()).toEqual([])))

describe("getRobotActions", () => {
  it("restarts a failed robot as Rust offers it, pushing first while the branch has work the remote lacks, disabled with the reason of a refusal", async () => {
    const failed = robot({})
    offers("r1", "restart")
    expect(labels(failed, ROW)).toEqual(["Restart Safe Push", ...PROMPTS, "Open in Patronus"])
    offers("r1", "pushAndRestart")
    expect(labels(failed, ROW)).toEqual(["Restart Safe Push", ...PROMPTS, "Open in Patronus"])
    offers("dry", "pushAndRestart")
    expect(labels(robot({ id: "dry", pushMode: "dryRun" }), ROW)).toEqual(["Restart Dry Run", ...PROMPTS, "Open in Patronus"])
    offers("r1", "restart", { code: "blocked", branch: "IJPL-1", by: "auth" })
    expect(labels(failed, ROW)[0]).toBe("Restart Safe Push (disabled: Blocked by auth: fix it first, then sync)")

    offers("r1", "restart")
    const [restart] = getRobotActions(failed, ROW)
    restart!.onSelect(FROM_MENU)
    // one restart of a robot at a time: the push and restart waits while the restart runs
    offers("r1", "pushAndRestart")
    expect(getRobotActions(failed, ROW)[0]).toMatchObject({ disabled: true, description: "Restarting the Safe Push…" })
    await vi.waitFor(() => expect(getOperations()).toEqual([]))
    const [pushAndRestart] = getRobotActions(failed, ROW)
    pushAndRestart!.onSelect(FROM_MENU)
    expect(getRobotActions(failed, ROW)[0]).toMatchObject({ running: true, description: "Pushing and restarting the Safe Push…" })
    // the restart takes the commit just pushed, and its toast is the one of the action
    await vi.waitFor(() => expect(calls).toEqual(["restart r1", "push IJPL-1, restart r1, announce false"]))
  })

  it("pushes and restarts a running robot of a branch changed since: it cancels it first", async () => {
    const running = robot({ status: "running", finishTime: null })
    offers("r1", "pushAndRestart")
    const actions = getRobotActions(running, ROW)
    expect(actions.map(it => it.id)).toEqual(["push-and-restart-robot", "cancel-robot", "open-robot"])
    expect(actions[0]!.description).toMatch(/^Cancels the run on .+, pushes IJPL-1 and starts it again on the pushed commit$/)
    actions[0]!.onSelect(FROM_MENU)
    await vi.waitFor(() => expect(calls).toEqual(["push IJPL-1, restart r1, announce false"]))
  })

  it("says why a restart waits: a robot that lands an ancestor, a merged Safe Push that landed the branch whole", () => {
    offers("r1", "restart", { code: "ancestor", landing: { branch: "auth", robotId: "auth-run", wholeStack: false }, mode: "rebaseAutosquash" })
    expect(labels(robot({}), ROW)[0]).toBe("Restart Safe Push (disabled: A Safe Push of auth is running: wait until it merges)")
    offers("r1", "pushAndRestart", { code: "landed", branch: "IJPL-1", by: { branch: "oauth", robotId: "oauth-run", wholeStack: false }, own: false, target: "master" })
    expect(labels(robot({}), ROW)[0]).toBe("Restart Safe Push (disabled: Landed in master with oauth)")
  })

  it("restarts the robot of the whole stack on its stack branch, pushed first, refused once merged Safe Pushes landed every branch of it", async () => {
    const whole = robot({ id: "whole", branchName: null, stackRoot: "air", sourceRevision: "stack-tip" })
    offers("whole", "restart")
    expect(labels(whole, null)).toEqual(["Restart Safe Push", ...PROMPTS, "Open in Patronus"])
    offers("whole", "pushAndRestart")
    const [pushAndRestart] = getRobotActions(whole, null)
    expect(pushAndRestart!.id).toBe("push-and-restart-robot")
    pushAndRestart!.onSelect(FROM_MENU)
    await vi.waitFor(() => expect(calls).toEqual(["push the whole air stack, restart whole"]))
    await vi.waitFor(() => expect(getOperations()).toEqual([]))

    // the Safe Pushes of deploy and side-test landed all of it: nothing of it is left to restart
    const landedBy = ["deploy", "side-test"].map(branch => ({ branch, robotId: branch, wholeStack: false }))
    offers("whole", "restart", { code: "stackLanded", root: "air", by: landedBy, own: false, target: "master" })
    expect(labels(whole, null)[0]).toBe("Restart Safe Push (disabled: Landed in master with deploy and side-test)")
  })

  it("cancels a running robot, and opens it in Patronus and the pull request it was started from", async () => {
    const running = robot({ status: "running", finishTime: null, pullRequest: { number: 12, url: "https://github.com/acme/monorepo/pull/12" } })
    const actions = getRobotActions(running, ROW)
    expect(actions.map(it => [it.id, it.label])).toEqual([
      ["cancel-robot", "Cancel Safe Push"],
      ["open-robot", "Open in Patronus"],
      ["open-robot-pull-request", "Open Pull Request #12"],
    ])
    for (const action of actions) {
      action.onSelect(FROM_MENU)
    }
    await vi.waitFor(() => expect(calls).toEqual(["cancel r1"]))
    expect(vi.mocked(openUrl).mock.calls.map(([url]) => url)).toEqual(["https://patronus/robot/r1", "https://github.com/acme/monorepo/pull/12"])
  })

  it("offers the prompt for an agent for a failed robot and for a running one whose required check failed, none for missing approvals", () => {
    const running = robot({ status: "running", finishTime: null })
    expect(labels(running, ROW, false, true)).toEqual(["Cancel Safe Push", ...PROMPTS, "Open in Patronus"])
    expect(labels(running, ROW, false, false)).toEqual(["Cancel Safe Push", "Open in Patronus"])
    // a restart Rust doesn't offer (a robot of a temporary ref, one that waits for approvals in a merge request) is none
    expect(labels(robot({}), ROW, true)).toEqual(["Open in Patronus"])
    expect(labels(robot({ status: "success" }), ROW)).toEqual(["Open in Patronus"])
  })
})

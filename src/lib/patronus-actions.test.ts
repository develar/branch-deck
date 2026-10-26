import { openUrl } from "@tauri-apps/plugin-opener"
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { ActionState, PatronusRobot } from "@/lib/bindings"
import { FROM_MENU } from "@/lib/menu"
import { getRobotActionItems } from "@/lib/patronus-actions"
import { getOperations } from "@/stores/operations"
import { disabled, enabled } from "@/test-support/actions"

const { calls } = vi.hoisted(() => ({
  /** What reached Patronus and the push, in order */
  calls: [] as string[],
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

/** The prompts of a failed or failing robot, before "Open in Patronus" */
const PROMPTS = ["Copy Prompt for Agent", "Copy Prompt with CI Details"]

/** The labels of the actions of `subject` Rust offers (`offered`), a disabled one with its reason */
function labels(subject: PatronusRobot, offered: readonly ActionState[], stepReason: string | null = null): string[] {
  return getRobotActionItems(subject, getOperations(), stepReason, offered).map(it => (it.disabled === true ? `${it.label} (disabled: ${it.description})` : it.label))
}

beforeEach(() => {
  calls.length = 0
  vi.mocked(openUrl).mockClear()
})

// a restart is an operation of its robot: the next test starts with none running
afterEach(() => vi.waitFor(() => expect(getOperations()).toEqual([])))

describe("getRobotActionItems", () => {
  it("presents the restart Rust offers, named by the mode, disabled with the words of its reason, the step's for missing approvals", () => {
    const failed = robot({})
    const prompts = enabled("copy-robot-prompt", "copy-robot-full-prompt", "open-robot")
    expect(labels(failed, [...enabled("restart-robot"), ...prompts])).toEqual(["Restart Safe Push", ...PROMPTS, "Open in Patronus"])
    expect(labels(robot({ id: "dry", pushMode: "dryRun" }), [...enabled("push-and-restart-robot"), ...prompts])).toEqual(["Restart Dry Run", ...PROMPTS, "Open in Patronus"])
    const blocked = disabled("restart-robot", { code: "rule", reason: { code: "blocked", branch: "IJPL-1", by: "auth" } })
    expect(labels(failed, [blocked])).toEqual(["Restart Safe Push (disabled: Blocked by auth: fix it first, then sync)"])
    const approvals = disabled("restart-robot", { code: "step", blocker: { code: "step" } })
    expect(labels(failed, [approvals], "Waits for the approval of Ann")).toEqual(["Restart Safe Push (disabled: Waits for the approval of Ann)"])
    const problems = disabled("copy-robot-prompt", { code: "pending", fact: "robotProblems" })
    expect(labels(failed, [problems])).toEqual(["Copy Prompt for Agent (disabled: Waits for Patronus to tell why the Safe Push failed)"])
  })

  it("restarts one robot at a time: a push and restart waits while the restart runs, and pushes first", async () => {
    const failed = robot({})
    const [restart] = getRobotActionItems(failed, getOperations(), null, enabled("restart-robot"))
    restart!.onSelect(FROM_MENU)
    expect(getRobotActionItems(failed, getOperations(), null, enabled("push-and-restart-robot"))[0]).toMatchObject({ disabled: true, description: "Restarting the Safe Push…" })
    await vi.waitFor(() => expect(getOperations()).toEqual([]))
    const [pushAndRestart] = getRobotActionItems(failed, getOperations(), null, enabled("push-and-restart-robot"))
    pushAndRestart!.onSelect(FROM_MENU)
    expect(getRobotActionItems(failed, getOperations(), null, enabled("push-and-restart-robot"))[0]).toMatchObject({
      running: true,
      description: "Pushing and restarting the Safe Push…",
    })
    // the restart takes the commit just pushed, and its toast is the one of the action
    await vi.waitFor(() => expect(calls).toEqual(["restart r1", "push IJPL-1, restart r1, announce false"]))
  })

  it("pushes and restarts a running robot of a branch changed since: it cancels it first", async () => {
    const running = robot({ status: "running", finishTime: null })
    const actions = getRobotActionItems(running, getOperations(), null, enabled("push-and-restart-robot", "cancel-robot", "open-robot"))
    expect(actions.map(it => it.id)).toEqual(["push-and-restart-robot", "cancel-robot", "open-robot"])
    expect(actions[0]!.description).toMatch(/^Cancels the run on .+, pushes IJPL-1 and starts it again on the pushed commit$/)
    actions[0]!.onSelect(FROM_MENU)
    await vi.waitFor(() => expect(calls).toEqual(["push IJPL-1, restart r1, announce false"]))
  })

  it("restarts the robot of the whole stack on its stack branch, pushed first", async () => {
    const whole = robot({ id: "whole", branchName: null, stackRoot: "air", sourceRevision: "stack-tip" })
    const [pushAndRestart] = getRobotActionItems(whole, getOperations(), null, enabled("push-and-restart-robot"))
    expect(pushAndRestart).toMatchObject({ id: "push-and-restart-robot", description: "Pushes the whole air stack and starts it again on the pushed commit" })
    pushAndRestart!.onSelect(FROM_MENU)
    await vi.waitFor(() => expect(calls).toEqual(["push the whole air stack, restart whole"]))
  })

  it("cancels a running robot, and opens it in Patronus and the pull request it was started from", async () => {
    const running = robot({ status: "running", finishTime: null, pullRequest: { number: 12, url: "https://github.com/acme/monorepo/pull/12" } })
    const actions = getRobotActionItems(running, getOperations(), null, enabled("cancel-robot", "open-robot", "open-robot-pull-request"))
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
})

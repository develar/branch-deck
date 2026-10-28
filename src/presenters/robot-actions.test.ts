// @spec spec/branch/patronus.spec.md
import { openUrl } from "@tauri-apps/plugin-opener"
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { ActionState, PatronusRobot } from "@/lib/bindings"
import { FROM_MENU } from "@/lib/menu"
import { getRobotActionItems } from "@/presenters/actions"
import { branchSyncStore } from "@/stores/branch-sync"
import { disabled, enabled } from "@/test-support/actions"

const { calls } = vi.hoisted(() => ({
  /** What the items ran, in order */
  calls: [] as string[],
}))

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn(async () => {}) }))
// the items run the intents of the engine: what they run is the test
vi.mock("@/lib/robot-run", () => ({
  restartRobot: async (robot: PatronusRobot) => {
    calls.push(`restart ${robot.id}`)
    return robot
  },
  cancelRobot: async (robot: PatronusRobot) => {
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
  return getRobotActionItems(subject, { offered, stepReason }).map(it => (it.disabled === true ? `${it.label} (disabled: ${it.description})` : it.label))
}

/** The action `id` of a robot whose intent runs in the session as the operation `op` (`with_operations`) */
function runs(id: ActionState["id"]): ActionState {
  return { id, availability: { kind: "running", op: "op" } }
}

beforeEach(() => {
  calls.length = 0
  vi.mocked(openUrl).mockClear()
})

describe("getRobotActionItems", () => {
  it("presents the restart Rust offers, named by the mode, disabled with the words of its reason, the step's for missing approvals", () => {
    const failed = robot({})
    const prompts = enabled("copy-robot-prompt", "copy-robot-full-prompt", "open-robot")
    expect(labels(failed, [...enabled("restart-robot"), ...prompts])).toEqual(["Restart Safe Push", ...PROMPTS, "Open in Patronus"])
    expect(labels(robot({ id: "dry", pushMode: "dryRun" }), [...enabled("push-and-restart-robot"), ...prompts])).toEqual(["Restart Dry Run", ...PROMPTS, "Open in Patronus"])
    const blocked = disabled("restart-robot", { code: "rule", reason: { code: "blocked", branch: "IJPL-1", by: "auth" } })
    expect(labels(failed, [blocked])).toEqual(["Restart Safe Push (disabled: Blocked by auth: fix it first, then sync)"])
    const parked = disabled("restart-robot", { code: "rule", reason: { code: "parked", branch: "IJPL-1", with: null, note: "" } })
    // a parked branch stays local: its restart would land it
    expect(labels(failed, [parked])).toEqual(["Restart Safe Push (disabled: Parked: Unpark it to push)"])
    const approvals = disabled("restart-robot", { code: "step", blocker: { code: "step" } })
    expect(labels(failed, [approvals], "Waits for the approval of Ann")).toEqual(["Restart Safe Push (disabled: Waits for the approval of Ann)"])
    const problems = disabled("copy-robot-prompt", { code: "pending", fact: "robotProblems" })
    expect(labels(failed, [problems])).toEqual(["Copy Prompt for Agent (disabled: Waits for Patronus to tell why the Safe Push failed)"])
  })

  it("runs the restart of the engine from both restarts, and shows one running in the session running on every surface", async () => {
    const failed = robot({})
    const [restart] = getRobotActionItems(failed, { offered: enabled("restart-robot") })
    restart!.onSelect(FROM_MENU)
    const [pushAndRestart] = getRobotActionItems(failed, { offered: enabled("push-and-restart-robot") })
    pushAndRestart!.onSelect(FROM_MENU)
    await vi.waitFor(() => expect(calls).toEqual(["restart r1", "restart r1"]))
    // Rust lays the restart that runs over the actions of its robot: running, and the cancel waits for it
    branchSyncStore.setState({
      operations: {
        op: {
          id: "op",
          intent: { kind: "restartRobot", robotId: "r1" },
          resources: [{ kind: "robot", id: "r1" }],
          step: { kind: "starting" },
          origin: { kind: "thisProcess" },
          moves: [],
        },
      },
    })
    const busy: ActionState = { id: "cancel-robot", availability: { kind: "disabled", reason: { code: "busy", op: "op" } } }
    expect(getRobotActionItems(failed, { offered: [runs("push-and-restart-robot"), busy] })).toMatchObject([
      { running: true, disabled: true, description: "Restarting the Safe Push…" },
      { disabled: true, description: "Restarting robot r1…" },
    ])
    branchSyncStore.setState({ operations: {} })
  })

  it("names what a restart of a robot of a branch changed since does: it cancels a running one, then pushes", () => {
    const running = robot({ status: "running", finishTime: null })
    const actions = getRobotActionItems(running, { offered: enabled("push-and-restart-robot", "cancel-robot", "open-robot") })
    expect(actions.map(it => it.id)).toEqual(["push-and-restart-robot", "cancel-robot", "open-robot"])
    expect(actions[0]!.description).toMatch(/^Cancels the run on .+, pushes IJPL-1 and starts it again on the pushed commit$/)
    const whole = robot({ id: "whole", branchName: null, stackRoot: "air", sourceRevision: "stack-tip" })
    const [pushAndRestart] = getRobotActionItems(whole, { offered: enabled("push-and-restart-robot") })
    expect(pushAndRestart).toMatchObject({ id: "push-and-restart-robot", description: "Pushes the whole air stack and starts it again on the pushed commit" })
  })

  it("cancels a running robot through the engine, and opens it in Patronus and the pull request it was started from", async () => {
    const runningRobot = robot({ status: "running", finishTime: null, pullRequest: { number: 12, url: "https://github.com/acme/monorepo/pull/12" } })
    const actions = getRobotActionItems(runningRobot, { offered: enabled("cancel-robot", "open-robot", "open-robot-pull-request") })
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
    expect(getRobotActionItems(runningRobot, { offered: [runs("cancel-robot")] })[0]).toMatchObject({
      running: true,
      description: "Canceling the Safe Push…",
    })
  })
})

// @spec spec/branch/patronus.spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { ApiError, EffectRecord, Intent, Outcome, PatronusRobot, Plan, Reason } from "@/lib/bindings"
import { cancelRobot, restartRobot } from "@/lib/robot-run"
import type { IntentRun } from "@/lib/run-intent"
import type { RemoteStatus } from "@/stores/branch-sync"
import { resetToasts, toasts } from "@/test-support/toast-mock"

const { engine } = vi.hoisted(() => ({
  engine: {
    /** The intents that reached the engine, with the repository */
    asked: [] as string[],
    /** How the next run goes */
    run: null as IntentRun | null,
    /** The robot toasts and the Activity of the robots, in order */
    said: [] as string[],
    /** The toasts of the remote that moved: the branch and whether its push restarts the Safe Push of the branch */
    moved: [] as string[],
    /** The remote status of the branch of the robot after the run */
    remoteStatus: undefined as RemoteStatus | undefined,
  },
}))

vi.mock("@/components/ui/toast", async () => (await import("@/test-support/toast-mock")).toastModule)
vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))
vi.mock("@/lib/run-intent", async importOriginal => ({
  ...(await importOriginal<typeof import("@/lib/run-intent")>()),
  runIntent: async (repositoryPath: string, intent: Intent) => {
    engine.asked.push(`${intent.kind} ${"robotId" in intent ? intent.robotId : ""} in ${repositoryPath}`)
    return engine.run
  },
}))
vi.mock("@/lib/robot-toast", () => ({
  showRobotToast: (title: string, robot: PatronusRobot) => engine.said.push(`toast ${title}: ${robot.id}`),
}))
vi.mock("@/stores/activity", () => ({
  recordRobot: (title: string, _description: string, robot: PatronusRobot) => engine.said.push(`activity ${title}: ${robot.id}`),
  recordActivity: () => {},
}))
vi.mock("@/lib/push-run", () => ({
  showRemoteMovedToast: async (_repositoryPath: string, branch: string, _status: RemoteStatus, restartRunning: boolean) => {
    engine.moved.push(`${branch}, restarts ${restartRunning}`)
  },
}))
vi.mock("@/stores/branch-sync", () => ({
  getBranch: () => (engine.remoteStatus === undefined ? undefined : { remoteStatus: engine.remoteStatus }),
}))

const FAILED: PatronusRobot = {
  id: "r1",
  name: "IJPL-1 fix the cache",
  status: "failure",
  pushMode: "rebaseAutosquash",
  branchName: "auth",
  sourceRevision: "older",
  targetBranch: "master",
  startTime: 100,
  finishTime: 200,
  cancellationReason: null,
  url: "https://patronus/robot/r1",
}

const PLAN = { intent: { kind: "restartRobot", robotId: "r1" }, effects: [], blockers: [] } as unknown as Plan

/** A run whose effects went as `effects` say, answering `robot` */
function ran(effects: EffectRecord[], robot: PatronusRobot | null, alreadyFinished = false): IntentRun {
  const outcome = { op: "op", effects, newsKey: null, versionAfter: null, result: { kind: "robot", robot, canceled: null, alreadyFinished }, stats: {} }
  return { kind: "ran", plan: PLAN, outcome: outcome as unknown as Outcome }
}

function error(code: ApiError["code"], message: string): ApiError {
  return { code, message, details: null } as ApiError
}

beforeEach(() => {
  resetToasts()
  engine.asked.length = 0
  engine.said.length = 0
  engine.moved.length = 0
  engine.remoteStatus = undefined
})

describe("restartRobot", () => {
  it("restarts through the engine and says it in the toast of the robot and Activity", async () => {
    engine.run = ran([{ effect: { kind: "restartRobot", robotId: "r1" }, status: { kind: "done" } }], { ...FAILED, id: "r2", status: "running" })
    expect((await restartRobot(FAILED, "/repo"))?.id).toBe("r2")
    expect(engine.asked).toEqual(["restartRobot r1 in /repo"])
    expect(engine.said).toEqual(["toast Safe Push restarted: r2", "activity Safe Push restarted: r2"])
  })

  it("says what its plan refuses it by, and runs nothing", async () => {
    const parked: Reason = { code: "parked", branch: "auth", with: null, note: "" }
    engine.run = { kind: "refused", plan: PLAN, reason: parked }
    expect(await restartRobot(FAILED, "/repo")).toBeNull()
    expect(toasts.map(toast => [toast.type, toast.title, toast.description])).toEqual([["info", "Cannot restart the Safe Push", "Parked: Unpark it to push"]])
  })

  it("opens the toast of the remote that moved when its push found it moved: that push restarts the robot", async () => {
    const push = { effect: { kind: "push", branches: ["auth"], force: false }, status: { kind: "failed", error: error("lease_rejected", "moved") } } as EffectRecord
    const skipped = { effect: { kind: "restartRobot", robotId: "r1" }, status: { kind: "skipped", reason: null } } as EffectRecord
    engine.run = ran([push, skipped], null)
    engine.remoteStatus = { remoteTip: "theirs" } as RemoteStatus
    expect(await restartRobot(FAILED, "/repo")).toBeNull()
    expect(engine.moved).toEqual(["auth, restarts true"])
    expect(toasts).toEqual([])
  })

  it("says the change of the facts after its push in an info toast, and a failure of Patronus in an error toast", async () => {
    const changed = {
      effect: { kind: "restartRobot", robotId: "r1" },
      status: { kind: "skipped", reason: error("plan_changed", "auth changed after the push: no robot started") },
    } as EffectRecord
    engine.run = ran([changed], null)
    await restartRobot(FAILED, "/repo")
    engine.run = ran([{ effect: { kind: "restartRobot", robotId: "r1" }, status: { kind: "failed", error: error("patronus_error", "Patronus is down") } }], null)
    await restartRobot(FAILED, "/repo")
    expect(toasts.map(toast => [toast.type, toast.title])).toEqual([
      ["info", "Cannot restart the Safe Push"],
      ["error", "Cannot restart the Safe Push"],
    ])
  })
})

describe("cancelRobot", () => {
  const running = { ...FAILED, status: "running", finishTime: null } as PatronusRobot
  const cancel = { effect: { kind: "cancelRobot", robotId: "r1" }, status: { kind: "done" } } as EffectRecord

  it("cancels through the engine: the robot is canceling", async () => {
    engine.run = ran([cancel], { ...running, status: "canceling" })
    expect((await cancelRobot(running, "/repo"))?.status).toBe("canceling")
    expect(engine.asked).toEqual(["cancelRobot r1 in /repo"])
    expect(engine.said).toEqual(["toast Safe Push is canceling: r1", "activity Safe Push is canceling: r1"])
  })

  it("answers a robot that finished meanwhile as it ended: nothing was canceled", async () => {
    engine.run = ran([cancel], FAILED, true)
    expect((await cancelRobot(running, "/repo"))?.status).toBe("failure")
    expect(engine.said).toEqual(["toast Safe Push already finished: r1", "activity Safe Push already finished: r1"])
  })

  it("says what Patronus refused the cancel with", async () => {
    engine.run = ran([{ ...cancel, status: { kind: "failed", error: error("patronus_error", "no such robot") } }], null)
    expect(await cancelRobot(running, "/repo")).toBeNull()
    expect(toasts.map(toast => [toast.type, toast.title])).toEqual([["error", "Cannot cancel the Safe Push"]])
  })
})

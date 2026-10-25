import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { PatronusRobot, PlannedEffect } from "@/lib/bindings"
import type { SafePushStart } from "@/lib/safe-push-start"
import { startSafePush } from "@/lib/safe-push-start"
import { getSafePushStarting } from "@/stores/safe-push"

const { calls, selection, pushGate, errors, errorActions, target } = vi.hoisted(() => ({
  /** What reached the backend and Patronus, with the repository each step named */
  calls: [] as string[],
  /** The repository the main window shows */
  selection: { path: "/a" },
  /** The push of the start waits for it */
  pushGate: { current: Promise.resolve() },
  errors: [] as string[],
  /** The action of the last error toast */
  errorActions: [] as { children: string; onClick: () => void }[],
  /** What the target check answers */
  target: { current: { kind: "start", note: null } as { kind: "start"; note: null } | { kind: "refuse"; reason: string } },
}))

vi.mock("@/stores/repository", () => ({
  vcsRequestFactory: { createRequest: () => ({ repositoryPath: selection.path }) },
  getSelectedRepositoryPath: () => selection.path,
  useSelectedProjectPath: () => selection.path,
  getRepositorySwitchReason: (path: string) => (path === selection.path ? null : `${path} is no longer the open repository`),
}))
vi.mock("@/stores/branch-sync", () => ({ branchSyncStore: { getState: () => ({ baselineBranch: `origin/master of ${selection.path}` }) } }))
vi.mock("@/lib/landing-gate", () => ({
  checkLandingBeforeStart: async (branchName: string, _root: string | null, _allow: unknown, repository: { request: { repositoryPath: string } }) => {
    calls.push(`landing ${branchName} in ${repository.request.repositoryPath}`)
    return { kind: "start", note: null }
  },
  formatLandingFix: () => null,
}))
vi.mock("@/lib/target-conflict", () => ({
  checkTargetBeforeStart: async (branchName: string, _root: string | null, repository: { request: { repositoryPath: string }; baselineBranch: string }) => {
    calls.push(`target ${branchName} in ${repository.request.repositoryPath} onto ${repository.baselineBranch}`)
    return target.current
  },
}))
vi.mock("@/lib/git/push", () => ({
  pushForAction: async (branchName: string, options: { repositoryPath?: string }) => {
    calls.push(`push ${branchName} in ${options.repositoryPath}`)
    await pushGate.current
    return { pushed: true }
  },
  cancelHoldingRobot: async () => true,
}))
vi.mock("@/lib/git/stack-push", () => ({ pushWholeStack: async () => true }))
vi.mock("@/stores/patronus", () => ({
  startPatronusRobot: async (branchName: string, ...rest: unknown[]) => {
    const repository = rest[6] as { request: { repositoryPath: string } }
    calls.push(`start ${branchName} in ${repository.request.repositoryPath}`)
    return ROBOT
  },
  startPatronusFromPullRequest: async (number: number, _mode: unknown, subject: { branchName: string }, request: { repositoryPath: string }) => {
    calls.push(`comment on #${number} of ${subject.branchName} in ${request.repositoryPath}`)
    return { robot: ROBOT, commentUrl: "c", listed: Promise.resolve() }
  },
  getRobot: () => undefined,
}))
vi.mock("@/stores/pull-requests", () => ({
  retargetPullRequest: async (branchName: string, base: string) => {
    calls.push(`retarget ${branchName} to ${base}`)
  },
  getPullRequest: () => ({ url: "https://github.com/acme/monorepo/pull/7" }),
}))
vi.mock("@/stores/activity", () => ({
  recordRetarget: (title: string) => calls.push(`record ${title}`),
  recordRobot: (title: string, _description: string, _robot: PatronusRobot, repositoryPath: string) => calls.push(`record ${title} in ${repositoryPath}`),
}))
vi.mock("@/lib/errors", () => ({
  notifyError: (title: string, reason: unknown, action?: { children: string; onClick: () => void }) => {
    errors.push(`${title}: ${String(reason)}`)
    if (action !== undefined) {
      errorActions.push(action)
    }
  },
}))
vi.mock("@/components/ui/toast", async () => (await import("@/test-support/toast-mock")).toastModule)
vi.mock("@/stores/update-project", () => ({
  runUpdateProjectOf: async (repositoryPath: string) => {
    calls.push(`update project of ${repositoryPath}`)
  },
}))
vi.mock("@/hooks/useStackActions", () => ({ restackBranch: async () => true }))
vi.mock("@/lib/patronus-actions", () => ({ runRobotAction: async () => {}, getRobotActions: () => [] }))
// the toast of the robot follows it in the stores: this test is about the repository of the start
vi.mock("@/lib/robot-toast", () => ({ showRobotEventToast: () => {} }))

const ROBOT: PatronusRobot = {
  id: "r1",
  name: "Safe Push of auth",
  status: "running",
  pushMode: "rebaseAutosquash",
  branchName: "auth",
  sourceRevision: "tip",
  targetBranch: "master",
  startTime: 100,
  finishTime: null,
  cancellationReason: null,
  url: "u",
}

const API: PlannedEffect = { kind: "startRobot", route: { kind: "api", profile: null } }

const PUSH: PlannedEffect = { kind: "push", branches: ["auth"], force: false }

function start(needsPush: boolean, effects: readonly PlannedEffect[] = needsPush ? [PUSH, API] : [API]): SafePushStart {
  return { branchName: "auth", mode: "rebaseAutosquash", profile: null, mergeRequest: null, effects, wholeStack: null }
}

beforeEach(() => {
  calls.length = 0
  errors.length = 0
  errorActions.length = 0
  target.current = { kind: "start", note: null }
  selection.path = "/a"
  pushGate.current = Promise.resolve()
})

describe("a Safe Push start during a switch of the repository", () => {
  it("starts the robot in the repository it was asked in when the switch comes during its push", async () => {
    let release!: () => void
    pushGate.current = new Promise(resolve => {
      release = resolve
    })
    const started = startSafePush(start(true))
    await vi.waitFor(() => expect(calls).toContain("push auth in /a"))
    expect(getSafePushStarting("auth", "/a")?.step).toBe("pushing")
    // another repository opens: its auth is another branch, and shows no start
    selection.path = "/b"
    expect(getSafePushStarting("auth")).toBeNull()
    release()

    expect(await started).toBe(true)
    expect(calls).toEqual(["landing auth in /a", "target auth in /a onto origin/master of /a", "push auth in /a", "start auth in /a", "record Safe Push started in /a"])
    expect(getSafePushStarting("auth", "/a")).toBeNull()
    expect(errors).toEqual([])
  })

  it("pushes nothing of the other repository when the switch comes before its push, and says why", async () => {
    const started = startSafePush(start(true))
    // the switch during the checks
    selection.path = "/b"

    expect(await started).toBe(false)
    expect(calls).toEqual(["landing auth in /a", "target auth in /a onto origin/master of /a"])
    expect(errors).toEqual(["Cannot start the Safe Push: /a is no longer the open repository"])
  })
})

describe("a Safe Push start the target refuses", () => {
  it("updates the repository of the start from its toast, also after a switch of the repository", async () => {
    target.current = { kind: "refuse", reason: "auth conflicts with the newest master" }
    expect(await startSafePush(start(false))).toBe(false)
    expect(errors).toEqual(["Cannot start the Safe Push: auth conflicts with the newest master"])
    selection.path = "/b"
    calls.length = 0
    errorActions[0]!.onClick()
    expect(calls).toEqual(["update project of /a"])
  })
})

describe("a Safe Push start runs the effects Rust planned", () => {
  it("retargets the pull request out of a virtual branch first, then pushes, then comments on the pull request (D11)", async () => {
    const effects: PlannedEffect[] = [
      { kind: "retarget", branch: "auth", number: 7, from: "user/virtual/core", to: "master" },
      { kind: "push", branches: ["auth"], force: false },
      { kind: "startRobot", route: { kind: "comment", number: 7 } },
    ]
    expect(await startSafePush(start(true, effects))).toBe(true)
    expect(calls).toEqual([
      "landing auth in /a",
      "target auth in /a onto origin/master of /a",
      "retarget auth to master",
      "record Retargeted #7 to master",
      "push auth in /a",
      "comment on #7 of auth in /a",
      "record Safe Push started in /a",
    ])
    expect(errors).toEqual([])
  })
})

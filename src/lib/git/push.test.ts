import { beforeEach, describe, expect, it, vi } from "vitest"
import type { PatronusRobot } from "@/lib/bindings"
import { pushBranch } from "@/lib/git/push"
import type { PushGuardRequest } from "@/stores/push-guard"
import { answerPushGuard, pushGuardStore } from "@/stores/push-guard"

const { calls, robots } = vi.hoisted(() => ({
  /** What reached the backend and Patronus, in order */
  calls: [] as string[],
  robots: { running: undefined as PatronusRobot | undefined },
}))

vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage: (message: unknown) => void = () => {}
  },
}))

vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))
vi.mock("@/components/ui/toast", () => ({ toast: { add: () => "toast", close: () => {} } }))
vi.mock("@/lib/git/stacked-push-toast", () => ({ showStackedPushToast: () => {} }))

vi.mock("@/lib/bindings", () => ({
  commands: {
    pushBranch: async (params: { branchName: string }) => {
      calls.push(`push ${params.branchName}`)
      return {
        status: "ok",
        data: {
          type: "pushed",
          remoteStatus: {
            branchName: params.branchName,
            remoteExists: true,
            unpushedCommits: [],
            commitsBehind: 0,
            droppedCommits: [],
            remoteTip: "tip",
            myUnpushedCount: 0,
            fastForward: true,
            lastPushTime: 1,
          },
        },
      }
    },
  },
}))

vi.mock("@/stores/branch-sync", () => ({
  branchSyncStore: { getState: () => ({ baselineBranch: "origin/master" }) },
  getBranch: (name: string) => ({ name, myEmail: "me@example.com", stack: null, remoteStatus: null, commits: [{ hash: "tip" }] }),
  isBranchPushing: () => false,
  setBranchPushing: () => {},
  setBranchPushProgress: () => {},
  setBranchRemoteStatus: () => {},
  toRemoteStatus: (status: { remoteTip: string }) => ({ remoteTip: status.remoteTip }),
}))

vi.mock("@/stores/patronus", () => ({
  getBranchRobot: () => robots.running,
  fetchPatronusRobotStatus: async () => "running",
  cancelPatronusRobot: async (robot: PatronusRobot) => {
    calls.push(`cancel ${robot.id}`)
    return { ...robot, status: "canceling" }
  },
  restartPatronusRobot: async (robot: PatronusRobot) => {
    calls.push(`restart ${robot.id}`)
    return { ...robot, id: "restarted" }
  },
}))

vi.mock("@/stores/pull-requests", () => ({ refreshPullRequestsAfterPush: async () => {} }))
vi.mock("@/stores/repository", () => ({
  vcsRequestFactory: { createRequest: () => ({ repositoryPath: "/repo", branchPrefix: "user" }) },
}))

/** A Safe Push of `auth` running on an older commit than the branch is at */
const RUNNING: PatronusRobot = {
  id: "old",
  name: "Safe Push of auth",
  status: "running",
  pushMode: "rebaseAutosquash",
  branchName: "auth",
  sourceRevision: "older",
  targetBranch: "master",
  startTime: 100,
  finishTime: null,
  cancellationReason: null,
  url: "u",
}

/** The question the push asks once it read the robot's status */
async function question(): Promise<PushGuardRequest> {
  await vi.waitFor(() => expect(pushGuardStore.getState().request).not.toBeNull())
  return pushGuardStore.getState().request!
}

beforeEach(() => {
  calls.length = 0
  robots.running = RUNNING
})

describe("pushBranch into a branch whose Safe Push runs", () => {
  it("cancels the running Safe Push for a push of a new one, and never restarts it", async () => {
    const push = pushBranch("auth", { startsRobot: true })
    expect(await question()).toMatchObject({ branchName: "auth", robot: RUNNING, startsRobot: true })
    answerPushGuard("cancel-push")
    expect(await push).toBe(true)
    // the caller's start is the one robot of the branch
    await vi.waitFor(() => expect(calls).toEqual(["cancel old", "push auth"]))
  })

  it("restarts the cancelled Safe Push on the pushed commit for any other push", async () => {
    const push = pushBranch("auth")
    expect(await question()).toMatchObject({ startsRobot: false })
    answerPushGuard("cancel-push-restart")
    expect(await push).toBe(true)
    await vi.waitFor(() => expect(calls).toEqual(["cancel old", "push auth", "restart old"]))
  })

  it("asks nothing for a branch without a running Safe Push", async () => {
    robots.running = undefined
    expect(await pushBranch("auth", { startsRobot: true })).toBe(true)
    expect(calls).toEqual(["push auth"])
    expect(pushGuardStore.getState().request).toBeNull()
  })
})

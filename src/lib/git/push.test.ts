import { beforeEach, describe, expect, it, vi } from "vitest"
import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import { pushBranch } from "@/lib/git/push"
import type { PushGuardRequest } from "@/stores/push-guard"
import { answerPushGuard, pushGuardStore } from "@/stores/push-guard"

const { calls, robots, toasts, stores } = vi.hoisted(() => ({
  /** What reached the backend, Patronus and the browser, in order */
  calls: [] as string[],
  robots: { running: undefined as PatronusRobot | undefined },
  toasts: [] as { title?: string, description?: unknown, actionProps?: { children: string, onClick: () => void } }[],
  stores: {
    /** The remote status of each branch after its push, as the store keeps it */
    remoteStatuses: {} as Record<string, unknown>,
    /** The pull requests GitHub lists, by branch */
    pullRequests: {} as Record<string, PullRequest>,
  },
}))

vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage: (message: unknown) => void = () => {}
  },
}))

vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))
vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: async (url: string) => {
    calls.push(`open ${url}`)
  },
}))
vi.mock("@/components/ui/toast", () => ({
  toast: {
    add: (options: (typeof toasts)[number]) => {
      toasts.push(options)
      return "toast"
    },
    close: () => {},
  },
}))
vi.mock("@/lib/git/stacked-push-toast", () => ({ showStackedPushToast: () => {}, getPullRequestBase: () => "master" }))

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
  getBranch: (name: string) => ({ name, myEmail: "me@example.com", stack: null, remoteStatus: stores.remoteStatuses[name] ?? null, commits: [{ hash: "tip" }] }),
  isBranchPushing: () => false,
  setBranchPushing: () => {},
  setBranchPushProgress: () => {},
  setBranchRemoteStatus: (name: string, status: unknown) => {
    stores.remoteStatuses[name] = status
  },
  toRemoteStatus: (status: { remoteExists: boolean, remoteTip: string }) => ({ exists: status.remoteExists, remoteTip: status.remoteTip }),
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

vi.mock("@/stores/pull-requests", () => ({
  refreshPullRequestsAfterPush: async () => {},
  getPullRequest: (name: string) => stores.pullRequests[name],
  getGitHubRepository: () => ({ owner: "JetBrains", name: "ultimate" }),
}))
vi.mock("@/stores/activity", async () => (await import("@/stores/test-store-mocks")).activityMock)
vi.mock("@/stores/repository", () => ({
  vcsRequestFactory: { createRequest: () => ({ repositoryPath: "/repo", branchPrefix: "user" }) },
  getFullBranchName: (name: string) => `user/virtual/${name}`,
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
  toasts.length = 0
  robots.running = RUNNING
  stores.remoteStatuses = {}
  stores.pullRequests = {}
})

describe("the toast of a push", () => {
  beforeEach(() => {
    robots.running = undefined
  })

  it("offers to create the pull request of a branch without one, into the baseline", async () => {
    expect(await pushBranch("auth")).toBe(true)
    const toast = toasts.at(-1)!
    expect(toast.title).toBe("Pushed auth")
    expect(toast.actionProps?.children).toBe("Create Pull Request")
    toast.actionProps!.onClick()
    await vi.waitFor(() => expect(calls).toEqual(["push auth", "open https://github.com/JetBrains/ultimate/compare/master...user/virtual/auth?expand=1"]))
  })

  it("opens the pull request the branch has", async () => {
    stores.pullRequests.auth = { url: "https://github.com/JetBrains/ultimate/pull/7" } as PullRequest
    expect(await pushBranch("auth")).toBe(true)
    const toast = toasts.at(-1)!
    expect(toast.actionProps?.children).toBe("Open Pull Request")
    toast.actionProps!.onClick()
    await vi.waitFor(() => expect(calls).toEqual(["push auth", "open https://github.com/JetBrains/ultimate/pull/7"]))
  })
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

  it("restarts the canceled Safe Push on the pushed commit for any other push", async () => {
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

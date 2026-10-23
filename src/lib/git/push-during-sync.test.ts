// The push of a branch pressed during a sync, with the real branch table: what the sync brings reaches the push
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { GroupedBranchInfo, RemoteStatusUpdate, SyncEvent } from "@/lib/bindings"
import { pushBranch } from "@/lib/git/push"
import { branchSyncStore, getBranch, isBranchPushing, syncBranches, whenSynced } from "@/stores/branch-sync"

const { remote, sync, pushes, toasts } = vi.hoisted(() => ({
  /** The toasts, "title: description" */
  toasts: [] as string[],
  /** Where the remote branch of `a` is */
  remote: { tip: "old-tip" },
  /** The syncs that wait until a test finishes them, with the channel of their events */
  sync: { held: [] as { channel: { onmessage: (event: SyncEvent) => void }; finish: () => void }[] },
  /** The `expectedRemote` of each push that reached the backend */
  pushes: [] as (string | null)[],
}))

vi.mock("@tauri-apps/api/core", () => import("@/test-support/channel-mock"))
vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))
vi.mock("@/components/ui/toast", () => ({
  toast: {
    add: (options: { title?: string; description?: unknown }) => {
      toasts.push(`${options.title}: ${String(options.description)}`)
      return "toast"
    },
    close: () => {},
  },
}))

vi.mock("@/lib/bindings", () => ({
  commands: {
    syncBranches: (_request: unknown, channel: { onmessage: (event: SyncEvent) => void }) =>
      new Promise(resolve => sync.held.push({ channel, finish: () => resolve({ status: "ok", data: null }) })),
    getSyncInputsFingerprint: async () => ({ status: "ok", data: "f1" }),
    // a lease on the remote tip: a remote that moved since is refused and fetched
    pushBranch: async (params: { branchName: string; expectedRemote: string | null }) => {
      pushes.push(params.expectedRemote)
      const moved = params.expectedRemote !== remote.tip
      if (!moved) {
        remote.tip = "a-tip"
      }
      return {
        status: "ok",
        data: { type: moved ? "remoteMoved" : "pushed", remoteStatus: statusOf(remote.tip, moved ? ["a-tip"] : []) },
      }
    },
  },
}))

vi.mock("@/stores/repository", () => ({
  getLoadingPromise: () => null,
  getSelectedProject: () => ({ path: "/repo" }),
  vcsRequestFactory: { createRequest: () => ({ repositoryPath: "/repo", branchPrefix: "user" }) },
  getSelectedRepositoryPath: () => "/repo",
  isSelectedRepository: (path: string) => path === "/repo",
  getRepositorySwitchReason: () => null,
  getFullBranchName: (name: string) => `user/virtual/${name}`,
}))
vi.mock("@/stores/settings/repositorySettings", () => ({ updateProject: () => {} }))
vi.mock("@/stores/archived-branches", () => ({
  updateFromArchivedNames: () => {},
  updateFromIntegrationInfo: () => {},
  updateFromIntegrationInfos: () => {},
  settleArchivedPlaceholders: () => {},
  resetArchivedBranches: () => {},
}))
vi.mock("@/stores/patronus", () => ({
  getPushHold: () => null,
  getPatronusGitHubRepository: () => null,
  getStackPushHold: () => null,
  getStackLandedReason: () => null,
  getStackBranchHoldReason: () => null,
  fetchPatronusRobotStatus: async () => "running",
  whenRepositoryViewCurrent: async () => {},
  cancelPatronusRobot: async () => null,
  restartPatronusRobot: async () => null,
}))
vi.mock("@/stores/pull-requests", () => ({
  refreshPullRequests: async () => {},
  refreshPullRequestsAfterPush: async () => {},
  refreshPullRequestsAfterPushes: async () => {},
  getPullRequest: () => undefined,
  getStackPullRequest: () => undefined,
  getGitHubRepository: () => null,
}))
vi.mock("@/stores/activity", async () => (await import("@/stores/test-store-mocks")).activityMock)

const groupA: GroupedBranchInfo = {
  name: "a",
  commits: [{ originalHash: "a-tip", strippedSubject: "subject", message: "subject", author: "a", authorTime: 1, committerTime: 1 }],
  latestCommitTime: 1,
  summary: "subject",
  allCommitsHaveIssueReferences: false,
  myEmail: null,
  origin: "Prefix",
  issueKey: null,
}

const branchesGrouped: SyncEvent = { type: "branchesGrouped", data: { branches: [groupA], baselineBranch: "origin/master" } }

/** The remote branch of `a` at `remoteTip`, an ancestor of the branch: its one commit is ahead */
function remoteAt(remoteTip: string): SyncEvent {
  return { type: "remoteStatusUpdate", data: statusOf(remoteTip, ["a-tip"]) }
}

/** The remote branch of `a` at `remoteTip`, which lacks `unpushedCommits` of it */
function statusOf(remoteTip: string, unpushedCommits: string[]): RemoteStatusUpdate {
  return {
    branchName: "a",
    remoteExists: true,
    unpushedCommits,
    commitsBehind: 0,
    commitsAmended: 0,
    commitsRestacked: 0,
    commitsLanded: 0,
    landedCommits: [],
    commitsReworded: 0,
    ancestorCommitsAhead: 0,
    myUnpushedCount: unpushedCommits.length,
    fastForward: true,
    lastPushTime: 0,
    droppedCommits: [],
    remoteTip,
  }
}

/** The next sync, once its request reached the backend (the inputs are read first) */
async function nextSync(): Promise<(typeof sync.held)[number]> {
  await vi.waitFor(() => expect(sync.held).toHaveLength(1))
  return sync.held.shift()!
}

/** A sync whose events are `events`, finished at once */
async function syncWith(...events: SyncEvent[]): Promise<void> {
  const done = syncBranches()
  const call = await nextSync()
  for (const event of events) {
    call.channel.onmessage(event)
  }
  call.finish()
  await done
}

describe("a push pressed during a sync", () => {
  beforeEach(async () => {
    branchSyncStore.setState(branchSyncStore.getInitialState(), true)
    remote.tip = "old-tip"
    pushes.length = 0
    toasts.length = 0
    sync.held.length = 0
    await syncWith(branchesGrouped, remoteAt("old-tip"))
  })

  it("pushes once, over the remote tip that the fetch of the sync it waited for brought", async () => {
    expect(getBranch("a")!.remoteStatus!.remoteTip).toBe("old-tip")
    const syncing = syncBranches()
    const call = await nextSync()

    const push = pushBranch("a")
    await vi.waitFor(() => expect(isBranchPushing("a", "/repo")).toBe(true))
    expect(pushes).toEqual([])
    // the fetch of the sync moved the remote branch: another machine pushed an ancestor of the branch
    remote.tip = "new-tip"
    call.channel.onmessage(branchesGrouped)
    call.channel.onmessage(remoteAt("new-tip"))
    call.finish()
    await syncing

    expect(await push, toasts.join("\n")).toBe(true)
    // one push, leased on the tip the sync brought: never a refused push over the old one, then a second
    expect(pushes).toEqual(["new-tip"])
    expect(remote.tip).toBe("a-tip")
    await whenSynced()
    expect(isBranchPushing("a", "/repo")).toBe(false)
    expect(getBranch("a")!.remoteStatus!.remoteTip).toBe("a-tip")
  })
})

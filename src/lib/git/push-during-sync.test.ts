// The push of a branch pressed during a sync, with the real store of the session's view: what the sync brings reaches the push
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { RemoteStatusUpdate } from "@/lib/bindings"
import { pushBranch } from "@/lib/git/push"
import { closeSession, openSession, requestSessionSync, whenSessionReady } from "@/model/store"
import { branchSyncStore, getBranch, isBranchPushing, whenSynced } from "@/stores/branch-sync"
import { fakeRemoteStatus, fakeSessions, grouping } from "@/test-support/fake-sessions"

const { remote, pushes, toasts } = vi.hoisted(() => ({
  /** The toasts, "title: description" */
  toasts: [] as string[],
  /** Where the remote branch of `a` is */
  remote: { tip: "old-tip" },
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
    update: () => {},
    close: () => {},
  },
}))

vi.mock("@/lib/bindings", async () => {
  const { fakeSessions, sessionCommands } = await import("@/test-support/fake-sessions")
  return {
    commands: {
      ...sessionCommands(),
      // a lease on the remote tip: a remote that moved since is refused and fetched; the session gets the outcome as a
      // patch, whose version the answer names
      pushBranch: async (params: { branchName: string; expectedRemote: string | null }) => {
        pushes.push(params.expectedRemote)
        const moved = params.expectedRemote !== remote.tip
        if (!moved) {
          remote.tip = "a-tip"
        }
        const remoteStatus = statusOf(remote.tip, moved ? ["a-tip"] : [])
        const { next } = fakeSessions.send("/repo", { branches: { a: { remote: { status: remoteStatus } } } })
        return { status: "ok", data: { outcome: moved ? { type: "remoteMoved", remoteStatus } : { type: "pushed", remoteStatus, pushedOver: null }, version: next } }
      },
    },
  }
})

vi.mock("@/stores/repository", async () => ({
  repositoryStore: (await import("@/test-support/quiet-store")).quietStore,
  getSelectedProject: () => ({ path: "/repo" }),
  vcsRequestFactory: { createRequest: () => ({ repositoryPath: "/repo" }) },
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
  getBranchLanding: () => ({ hold: null, start: null, dryRun: null, replaces: null, push: null }),
  getStackLanding: () => ({ hold: null, start: null, dryRun: null, replaces: null }),
  whenRepositoryViewCurrent: async () => {},
}))
vi.mock("@/stores/pull-requests", async () => ({
  pullRequestsStore: (await import("@/test-support/quiet-store")).quietStore,
  getPullRequest: () => undefined,
  getStackPullRequest: () => undefined,
  getGitHubRepository: () => null,
}))
vi.mock("@/stores/activity", async () => (await import("@/stores/test-store-mocks")).activityMock)

/** The remote branch of `a` at `remoteTip`, which lacks `unpushedCommits` of it */
function statusOf(remoteTip: string, unpushedCommits: string[]): RemoteStatusUpdate {
  return { ...fakeRemoteStatus("a", unpushedCommits.length, remoteTip), unpushedCommits }
}

/** The branch `a`, its remote branch at `remoteTip`, an ancestor of it: its one commit is ahead */
function remoteAt(remoteTip: string) {
  return { branches: { a: { remote: { status: statusOf(remoteTip, ["a-tip"]) } } } }
}

describe("a push pressed during a sync", () => {
  beforeEach(async () => {
    fakeSessions.reset()
    closeSession()
    remote.tip = "old-tip"
    pushes.length = 0
    toasts.length = 0
    openSession("/repo")
    await whenSessionReady()
    fakeSessions.send("/repo", grouping("a"))
    fakeSessions.send("/repo", remoteAt("old-tip"))
    await vi.waitFor(() => expect(branchSyncStore.getState().branchOrder).toEqual(["a"]))
  })

  it("pushes once, over the remote tip that the fetch of the sync it waited for brought", async () => {
    expect(getBranch("a")!.remoteStatus!.remoteTip).toBe("old-tip")
    const syncing = requestSessionSync("manual")
    await vi.waitFor(() => expect(fakeSessions.syncs).toHaveLength(1))

    const push = pushBranch("a")
    await vi.waitFor(() => expect(isBranchPushing("a", "/repo")).toBe(true))
    expect(pushes).toEqual([])
    // the fetch of the sync moved the remote branch: another machine pushed an ancestor of the branch
    remote.tip = "new-tip"
    fakeSessions.send("/repo", { sync: { kind: "running", step: "building" } })
    fakeSessions.send("/repo", { ...remoteAt("new-tip"), sync: { kind: "idle" } })
    fakeSessions.syncs[0]!.answer()
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

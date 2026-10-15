import { createElement, Fragment } from "react"
import type { ReactNode } from "react"
import { renderToStaticMarkup } from "react-dom/server"
import { beforeEach, describe, expect, it, vi } from "vitest"
import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import { pushBranch, pushStack } from "@/lib/git/push"
import type { Branch, RemoteStatus } from "@/stores/branch-sync"
import type { PushGuardRequest } from "@/stores/push-guard"
import { answerPushGuard, pushGuardStore } from "@/stores/push-guard"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

const { calls, records, robots, toasts, stores } = vi.hoisted(() => ({
  /** What reached the backend, Patronus, the clipboard and the browser, in order */
  calls: [] as string[],
  /** What Activity recorded */
  records: [] as string[],
  robots: { running: undefined as PatronusRobot | undefined },
  toasts: [] as { type?: string, title?: string, description?: unknown, actionProps?: { children: string, onClick: () => void } }[],
  stores: {
    /** The branch table, by name */
    branches: {} as Record<string, Branch>,
    /** The pull requests GitHub lists, by branch */
    pullRequests: {} as Record<string, PullRequest>,
    /** The `expectedRemote` of each push */
    expectedRemotes: [] as (string | null)[],
    /** Runs while the push guard waits for the answer, e.g. a sync */
    whileAsking: () => {},
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
vi.mock("@/lib/clipboard", () => ({
  copyText: async (text: string) => {
    calls.push(`copy ${text}`)
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

vi.mock("@/lib/bindings", () => ({
  commands: {
    pushBranch: async (params: { branchName: string, expectedRemote: string | null }) => {
      calls.push(`push ${params.branchName}`)
      stores.expectedRemotes.push(params.expectedRemote)
      return {
        status: "ok",
        data: {
          type: "pushed",
          remoteStatus: { branchName: params.branchName, remoteExists: true, remoteTip: "tip" },
        },
      }
    },
  },
}))

vi.mock("@/stores/branch-sync", () => ({
  branchSyncStore: { getState: () => ({ baselineBranch: "origin/master" }) },
  getBranch: (name: string) => stores.branches[name],
  getStackDescendants: () => [],
  isBranchPushing: () => false,
  setBranchPushing: () => {},
  setBranchPushProgress: () => {},
  setBranchRemoteStatus: (name: string, status: RemoteStatus) => {
    stores.branches[name] = { ...stores.branches[name]!, remoteStatus: status }
  },
  toRemoteStatus: (status: { remoteExists: boolean, remoteTip: string }): RemoteStatus => ({
    exists: status.remoteExists,
    unpushedCommits: [],
    commitsAhead: 0,
    commitsBehind: 0,
    myCommitsAhead: 0,
    fastForward: true,
    lastPushTime: 1,
    remoteTip: status.remoteTip,
    droppedCommits: [],
  }),
}))

vi.mock("@/stores/patronus", () => ({
  getBranchRobot: () => robots.running,
  getPatronusGitHubRepository: () => null,
  fetchPatronusRobotStatus: async () => "running",
  cancelPatronusRobot: async (robot: PatronusRobot) => {
    calls.push(`cancel ${robot.id}`)
    return { ...robot, status: "canceling" }
  },
  restartPatronusRobot: async (robot: PatronusRobot) => {
    calls.push(`restart ${robot.id}`)
    return { ...robot, id: "restarted", url: "https://patronus/restarted" }
  },
}))

vi.mock("@/stores/pull-requests", () => ({
  refreshPullRequestsAfterPush: async () => {},
  getPullRequest: (name: string) => stores.pullRequests[name],
  getGitHubRepository: () => ({ owner: "JetBrains", name: "ultimate" }),
}))
vi.mock("@/stores/activity", async () => ({
  ...(await import("@/stores/test-store-mocks")).activityMock,
  recordPush: (_repositoryPath: string, branchName: string) => {
    records.push(`push ${branchName}`)
  },
  recordRobot: (title: string) => {
    records.push(title)
  },
}))
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

const COMPARE = "https://github.com/JetBrains/ultimate/compare"

/** A branch of one commit the remote lacks; `remote: false` when it isn't on the remote yet */
function addBranch(name: string, parent: string | null, remote = true): void {
  const remoteStatus = testRemoteStatus({ exists: remote, unpushedCommits: ["c"], commitsAhead: 1, myCommitsAhead: 1, remoteTip: remote ? "old-tip" : null })
  stores.branches[name] = testBranch(name, parent, { remoteStatus, commits: [{ hash: `${name}-tip` }] as unknown as Branch["commits"], myEmail: "me@example.com" })
}

/** The text of a toast description, which names a branch in medium weight */
function text(description: unknown): string {
  return renderToStaticMarkup(createElement(Fragment, null, description as ReactNode)).replace(/<[^>]+>/g, "")
}

/** The question the push asks once it read the robot's status */
async function question(): Promise<PushGuardRequest> {
  await vi.waitFor(() => expect(pushGuardStore.getState().request).not.toBeNull())
  return pushGuardStore.getState().request!
}

beforeEach(() => {
  calls.length = 0
  records.length = 0
  toasts.length = 0
  robots.running = RUNNING
  stores.branches = {}
  stores.pullRequests = {}
  stores.expectedRemotes = []
  // a root with stacks on: a stack without a parent
  addBranch("auth", null, false)
})

describe("the toast of a push", () => {
  beforeEach(() => {
    robots.running = undefined
  })

  it("offers to create the pull request of a branch without one, into the baseline", async () => {
    expect(await pushBranch("auth")).toBe(true)
    expect(toasts).toHaveLength(1)
    const toast = toasts[0]!
    expect(toast.title).toBe("Pushed auth")
    // a root names no base: the baseline is the base of every branch that isn't stacked
    expect(text(toast.description)).toBe("1 commit · created the remote branch")
    expect(toast.actionProps?.children).toBe("Create Pull Request")
    toast.actionProps!.onClick()
    await vi.waitFor(() => expect(calls).toEqual(["push auth", `open ${COMPARE}/master...user/virtual/auth?expand=1`]))
  })

  it("opens the pull request the branch has", async () => {
    stores.pullRequests.auth = { url: "https://github.com/JetBrains/ultimate/pull/7", state: "open" } as PullRequest
    expect(await pushBranch("auth")).toBe(true)
    const toast = toasts.at(-1)!
    expect(toast.actionProps?.children).toBe("Open Pull Request")
    toast.actionProps!.onClick()
    await vi.waitFor(() => expect(calls).toEqual(["push auth", "open https://github.com/JetBrains/ultimate/pull/7"]))
  })

  it("offers a new pull request when the branch has only a closed one", async () => {
    stores.pullRequests.auth = { url: "https://github.com/JetBrains/ultimate/pull/7", state: "closed" } as PullRequest
    expect(await pushBranch("auth")).toBe(true)
    expect(toasts.at(-1)!.actionProps?.children).toBe("Create Pull Request")
  })

  it("names the parent of a stacked branch and creates its pull request into the parent's branch", async () => {
    addBranch("auth", null)
    addBranch("login", "auth", false)
    expect(await pushBranch("login")).toBe(true)
    const toast = toasts.at(-1)!
    expect(text(toast.description)).toBe("1 commit · created the remote branch · pull request base: auth")
    expect(toast.actionProps?.children).toBe("Create Pull Request")
    toast.actionProps!.onClick()
    await vi.waitFor(() => expect(calls).toContain(`open ${COMPARE}/user/virtual/auth...user/virtual/login?expand=1`))
  })

  it("creates the pull request of a stacked branch into the baseline while its parent isn't on the remote", async () => {
    addBranch("login", "auth", false)
    expect(await pushBranch("login")).toBe(true)
    const toast = toasts.at(-1)!
    expect(text(toast.description)).toBe("1 commit · created the remote branch · pull request base: master")
    toast.actionProps!.onClick()
    await vi.waitFor(() => expect(calls).toContain(`open ${COMPARE}/master...user/virtual/login?expand=1`))
  })

  it("copies the command that retargets a pull request with another base", async () => {
    addBranch("auth", null)
    addBranch("login", "auth")
    stores.pullRequests.login = { number: 8, state: "open", baseRefName: "master" } as PullRequest
    expect(await pushBranch("login")).toBe(true)
    const toast = toasts.at(-1)!
    expect(text(toast.description)).toBe("1 commit · the pull request targets another base: retarget it to auth")
    expect(toast.actionProps?.children).toBe("Copy gh pr edit")
    toast.actionProps!.onClick()
    await vi.waitFor(() => expect(calls).toContain("copy gh pr edit 8 --base user/virtual/auth"))
  })

  it("says nothing for a push that is a step of another action, and Activity records it", async () => {
    expect(await pushBranch("auth", { announce: false })).toBe(true)
    expect(toasts).toEqual([])
    expect(records).toEqual(["push auth"])
  })
})

describe("pushBranch into a branch whose Safe Push runs", () => {
  it("cancels the running Safe Push for a push of a new one, and never restarts it", async () => {
    const push = pushBranch("auth", { startsRobot: true, announce: false })
    expect(await question()).toMatchObject({ branchName: "auth", robot: RUNNING, startsRobot: true })
    answerPushGuard("cancel-push")
    expect(await push).toBe(true)
    // the caller's start is the one robot of the branch, and its toast the one toast
    await vi.waitFor(() => expect(calls).toEqual(["cancel old", "push auth"]))
    expect(toasts).toEqual([])
  })

  it("restarts the canceled Safe Push on the pushed commit for any other push, in the one toast of the push", async () => {
    const push = pushBranch("auth")
    expect(await question()).toMatchObject({ startsRobot: false })
    answerPushGuard("cancel-push-restart")
    expect(await push).toBe(true)
    expect(calls).toEqual(["cancel old", "push auth", "restart old"])
    expect(toasts).toHaveLength(1)
    const toast = toasts[0]!
    expect(toast.title).toBe("Pushed auth")
    expect(text(toast.description)).toBe("1 commit · created the remote branch · Safe Push restarted")
    expect(toast.actionProps?.children).toBe("Open in Patronus")
    toast.actionProps!.onClick()
    await vi.waitFor(() => expect(calls).toContain("open https://patronus/restarted"))
    expect(records).toEqual(["push auth", "Safe Push restarted"])
  })

  it("says only the restart for Push and Restart, recorded once", async () => {
    robots.running = undefined
    expect(await pushBranch("auth", { restartRobot: { ...RUNNING, status: "failure" }, announce: false })).toBe(true)
    expect(toasts.map(toast => toast.title)).toEqual(["Safe Push restarted"])
    expect(records).toEqual(["push auth", "Safe Push restarted"])
  })

  it("pushes the branch as a sync left it while the question waited", async () => {
    const push = pushBranch("auth")
    await question()
    stores.branches.auth = { ...stores.branches.auth!, remoteStatus: testRemoteStatus({ remoteTip: "synced-tip", commitsAhead: 1 }) }
    answerPushGuard("push")
    expect(await push).toBe(true)
    expect(stores.expectedRemotes).toEqual(["synced-tip"])
  })

  it("asks nothing for a branch without a running Safe Push", async () => {
    robots.running = undefined
    expect(await pushBranch("auth", { startsRobot: true })).toBe(true)
    expect(calls).toEqual(["push auth"])
    expect(pushGuardStore.getState().request).toBeNull()
  })
})

describe("the toast of a stack push", () => {
  beforeEach(() => {
    robots.running = undefined
    addBranch("login", "auth", false)
    addBranch("oauth", "login", false)
  })

  it("is one toast for the branches, with the pull request of the branch the stack was pushed from", async () => {
    await pushStack("oauth")
    expect(calls).toEqual(["push auth", "push login", "push oauth"])
    expect(records).toEqual(["push auth", "push login", "push oauth"])
    expect(toasts).toHaveLength(1)
    const toast = toasts[0]!
    expect(toast.title).toBe("Pushed auth, login and oauth")
    expect(text(toast.description)).toBe("3 commits · created 3 remote branches")
    expect(toast.actionProps?.children).toBe("Create Pull Request")
    toast.actionProps!.onClick()
    await vi.waitFor(() => expect(calls).toContain(`open ${COMPARE}/user/virtual/login...user/virtual/oauth?expand=1`))
  })
})

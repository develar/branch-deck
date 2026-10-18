import { createElement, Fragment } from "react"
import type { ReactNode } from "react"
import { renderToStaticMarkup } from "react-dom/server"
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import { formatNewPullRequestBlocker, getNewPullRequestUrl } from "@/lib/git/new-pull-request"
import { pushBranch, pushForAction } from "@/lib/git/push"
import { pushStack, pushUpTo } from "@/lib/git/stack-push"
import type { Branch, RemoteStatus } from "@/stores/branch-sync"
import { createPullRequestDialogStore } from "@/stores/create-pull-request-dialog"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

const { calls, records, robots, toasts, stores } = vi.hoisted(() => ({
  /** What reached the backend, Patronus, the clipboard and the browser, in order */
  calls: [] as string[],
  /** What Activity recorded */
  records: [] as string[],
  robots: {
    /** The running Safe Push of `auth`, which holds its push */
    running: undefined as PatronusRobot | undefined,
    /** What Patronus answers when the push reads the status fresh */
    status: "running" as string,
    /** The branches a running Safe Push of a branch built on them lands: branch -> its robot */
    spans: {} as Record<string, PatronusRobot>,
  },
  toasts: [] as { type?: string; title?: string; description?: unknown; actionProps?: { children: string; onClick: () => void } }[],
  stores: {
    /** The branch table, by name */
    branches: {} as Record<string, Branch>,
    /** What `gh pr edit` answers: null = it retargets */
    retargetError: null as string | null,
    /** The stack pushes in flight: the branch a push started from -> the branches it pushes */
    stackPushes: {} as Record<string, readonly string[]>,
    /**
     * What the next stack pushes find instead of pushing, in turn: the branch whose remote branch moved (the push refuses
     * all of its branches), and its commits the branch lacks
     */
    movedInStack: [] as { branch: string; commitsBehind: number }[],
    /** The pull requests GitHub lists, by branch */
    pullRequests: {} as Record<string, PullRequest>,
    /** The `expectedRemote` of each push */
    expectedRemotes: [] as (string | null)[],
    /** What the next pushes find on the remote instead of pushing (`remoteMoved`), in turn: its commits behind, its tip */
    movedRemotes: [] as { commitsBehind: number; remoteTip: string }[],
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
    pushBranch: async (params: { branchName: string; expectedRemote: string | null }) => {
      calls.push(`push ${params.branchName}`)
      stores.expectedRemotes.push(params.expectedRemote)
      const moved = stores.movedRemotes.shift()
      return {
        status: "ok",
        data: {
          type: moved === undefined ? "pushed" : "remoteMoved",
          remoteStatus: { branchName: params.branchName, remoteExists: true, remoteTip: moved?.remoteTip ?? "tip", commitsBehind: moved?.commitsBehind ?? 0 },
        },
      }
    },
    /** One push of the branches, "push auth login@<tip>" */
    pushBranches: async (params: { branches: { branchName: string; expectedRemote: string | null; tip?: string | null }[] }) => {
      calls.push(`push ${params.branches.map(branch => (branch.tip == null ? branch.branchName : `${branch.branchName}@${branch.tip}`)).join(" ")}`)
      stores.expectedRemotes.push(...params.branches.map(branch => branch.expectedRemote))
      const moved = stores.movedInStack.shift()
      // refused, the fetched remote of the moved branch is at their commit, the others where the rows showed them
      const remoteStatuses = params.branches.map(branch => {
        if (moved === undefined) {
          return { branchName: branch.branchName, remoteExists: true, remoteTip: "tip" }
        }
        return branch.branchName === moved.branch
          ? { branchName: branch.branchName, remoteExists: true, remoteTip: "theirs", commitsBehind: moved.commitsBehind }
          : { branchName: branch.branchName, remoteExists: branch.expectedRemote !== null, remoteTip: branch.expectedRemote }
      })
      return { status: "ok", data: moved === undefined ? { type: "pushed", remoteStatuses } : { type: "remoteMoved", branchName: moved.branch, remoteStatuses } }
    },
  },
}))

vi.mock("@/stores/branch-sync", () => ({
  branchSyncStore: { getState: () => ({ baselineBranch: "origin/master" }) },
  getBranch: (name: string) => stores.branches[name],
  getStackDescendants: () => [],
  isBranchPushing: () => false,
  whenSynced: async () => {},
  whenBranchPushed: async () => {},
  isStackPushing: (name: string) => Object.hasOwn(stores.stackPushes, name),
  setBranchPushing: () => {},
  setStackPush: (startedFrom: string, names: readonly string[] | null) => {
    const { [startedFrom]: _, ...others } = stores.stackPushes
    stores.stackPushes = names === null ? others : { ...others, [startedFrom]: names }
  },
  setBranchPushProgress: () => {},
  setBranchesPushProgress: () => {},
  setBranchRemoteStatus: (name: string, status: RemoteStatus) => {
    stores.branches[name] = { ...stores.branches[name]!, remoteStatus: status }
  },
  toRemoteStatus: (status: { remoteExists: boolean; remoteTip: string; commitsBehind?: number }): RemoteStatus => ({
    exists: status.remoteExists,
    unpushedCommits: [],
    // a remote that moved still lacks the branch's commit
    commitsAhead: status.remoteTip === "tip" ? 0 : 1,
    commitsBehind: status.commitsBehind ?? 0,
    commitsAmended: 0,
    commitsRestacked: 0,
    myCommitsAhead: 0,
    fastForward: true,
    lastPushTime: 1,
    remoteTip: status.remoteTip,
    droppedCommits: [],
  }),
}))

vi.mock("@/stores/patronus", () => ({
  getPushHold: (name: string) => {
    if (name === "auth" && robots.running !== undefined && robots.running.status === "running") {
      return { kind: "own", branch: "auth", robot: robots.running }
    }
    const span = robots.spans[name]
    return span === undefined || span.status !== "running" ? null : { kind: "span", branch: span.branchName, robot: span }
  },
  getPatronusGitHubRepository: () => null,
  fetchPatronusRobotStatus: async () => robots.status,
  cancelPatronusRobot: async (robot: PatronusRobot) => {
    calls.push(`cancel ${robot.id}`)
    // the store has it canceling: it holds nothing
    for (const held of [robots.running, ...Object.values(robots.spans)]) {
      if (held?.id === robot.id) {
        held.status = "canceling"
      }
    }
    return { ...robot, status: "canceling" }
  },
  restartPatronusRobot: async (robot: PatronusRobot) => {
    calls.push(`restart ${robot.id}`)
    return { ...robot, id: "restarted", url: "https://patronus/restarted" }
  },
}))

vi.mock("@/stores/pull-requests", () => ({
  refreshPullRequestsAfterPush: async () => {},
  refreshPullRequestsAfterPushes: async () => {},
  getPullRequest: (name: string) => stores.pullRequests[name],
  getGitHubRepository: () => ({ owner: "JetBrains", name: "ultimate" }),
  retargetPullRequest: async (name: string, base: string) => {
    calls.push(`retarget ${name} ${base}`)
    if (stores.retargetError !== null) {
      throw new Error(stores.retargetError)
    }
    stores.pullRequests[name] = { ...stores.pullRequests[name]!, baseRefName: base }
  },
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

beforeEach(() => {
  calls.length = 0
  records.length = 0
  toasts.length = 0
  robots.running = { ...RUNNING }
  robots.status = "running"
  robots.spans = {}
  stores.branches = {}
  stores.pullRequests = {}
  stores.expectedRemotes = []
  stores.movedRemotes = []
  stores.retargetError = null
  stores.movedInStack = []
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
    expect(toast.actionProps?.children).toBe("Create Pull Request…")
    toast.actionProps!.onClick()
    expect(createPullRequestDialogStore.getState().branchName).toBe("auth")
    // "Continue on GitHub" of the dialog opens the form into the same base
    expect(getNewPullRequestUrl(stores.branches.auth!)).toBe(`${COMPARE}/master...user/virtual/auth?expand=1`)
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
    expect(toasts.at(-1)!.actionProps?.children).toBe("Create Pull Request…")
  })

  it("names the parent of a stacked branch and creates its pull request into the parent's branch", async () => {
    addBranch("auth", null)
    addBranch("login", "auth", false)
    expect(await pushBranch("login")).toBe(true)
    const toast = toasts.at(-1)!
    expect(text(toast.description)).toBe("1 commit · created the remote branch · pull request base: auth")
    expect(toast.actionProps?.children).toBe("Create Pull Request…")
    toast.actionProps!.onClick()
    expect(createPullRequestDialogStore.getState().branchName).toBe("login")
    // "Continue on GitHub" of the dialog opens the form into the same base
    expect(getNewPullRequestUrl(stores.branches.login!)).toBe(`${COMPARE}/user/virtual/auth...user/virtual/login?expand=1`)
  })

  it("creates the pull request of a stacked branch into the baseline while its parent isn't on the remote", async () => {
    addBranch("login", "auth", false)
    expect(await pushBranch("login")).toBe(true)
    const toast = toasts.at(-1)!
    expect(text(toast.description)).toBe("1 commit · created the remote branch · pull request base: master")
    toast.actionProps!.onClick()
    expect(createPullRequestDialogStore.getState().branchName).toBe("login")
    // "Continue on GitHub" of the dialog opens the form into the same base
    expect(getNewPullRequestUrl(stores.branches.login!)).toBe(`${COMPARE}/master...user/virtual/login?expand=1`)
  })

  it("retargets a pull request with another base before the push", async () => {
    addBranch("auth", null)
    addBranch("login", "auth")
    stores.pullRequests.login = { number: 8, state: "open", baseRefName: "master" } as PullRequest
    expect(await pushBranch("login")).toBe(true)
    expect(calls.slice(0, 2)).toEqual(["retarget login user/virtual/auth", "push login"])
    const toast = toasts.at(-1)!
    expect(text(toast.description)).toBe("1 commit · retargeted pull request #8 to auth")
    expect(toast.actionProps?.children).toBe("Open Pull Request")
  })

  it("pushes and copies the command that retargets a pull request when the retarget fails", async () => {
    addBranch("auth", null)
    addBranch("login", "auth")
    stores.pullRequests.login = { number: 8, state: "open", baseRefName: "master" } as PullRequest
    stores.retargetError = "HTTP 403"
    expect(await pushBranch("login")).toBe(true)
    expect(toasts.some(toast => toast.title === "Cannot retarget pull request #8")).toBe(true)
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
  it("refuses the push: it won't land, and the Safe Push would fail at the end", async () => {
    expect(await pushBranch("auth")).toBe(false)
    expect(calls).toEqual([])
    expect(toasts.map(toast => toast.title)).toEqual(["Cannot push auth"])
    expect(text(toasts[0]!.description)).toContain("A Safe Push of auth is running: wait until it merges, or restart it on the current branch")
  })

  it("pushes when the Safe Push finished meanwhile: the status is read fresh", async () => {
    robots.status = "success"
    expect(await pushBranch("auth")).toBe(true)
    expect(calls).toEqual(["push auth"])
  })

  it("cancels the running Safe Push for the start that replaces it, and never restarts it", async () => {
    expect(await pushBranch("auth", { replacesRobot: robots.running, announce: false })).toBe(true)
    // the caller's start is the one robot of the branch, and its toast the one toast
    expect(calls).toEqual(["cancel old", "push auth"])
    expect(toasts).toEqual([])
  })

  it("cancels, pushes and restarts the running Safe Push for its restart, in the one toast of the restart", async () => {
    expect(await pushBranch("auth", { restartRobot: robots.running, announce: false })).toBe(true)
    expect(calls).toEqual(["cancel old", "push auth", "restart old"])
    expect(toasts.map(toast => toast.title)).toEqual(["Safe Push restarted"])
    expect(records).toEqual(["push auth", "Safe Push restarted"])
  })

  it("refuses a restart of another robot: only the robot the push restarts lets it through", async () => {
    expect(await pushBranch("auth", { restartRobot: { ...RUNNING, id: "other", status: "failure" } })).toBe(false)
    expect(calls).toEqual([])
  })

  it("says only the restart for the restart of a finished robot that pushes first, recorded once", async () => {
    robots.running = undefined
    expect(await pushBranch("auth", { restartRobot: { ...RUNNING, status: "failure" }, announce: false })).toBe(true)
    expect(toasts.map(toast => toast.title)).toEqual(["Safe Push restarted"])
    expect(records).toEqual(["push auth", "Safe Push restarted"])
  })

  it("refuses a branch a Safe Push of a branch built on it lands", async () => {
    robots.running = undefined
    addBranch("login", "auth")
    robots.spans.auth = { ...RUNNING, id: "login-run", branchName: "login" }
    expect(await pushBranch("auth")).toBe(false)
    expect(text(toasts[0]!.description)).toContain("A Safe Push of login is running: it lands auth with it")
  })
})

describe("a stack push next to a running Safe Push", () => {
  beforeEach(() => {
    robots.running = undefined
    addBranch("login", "auth", false)
    addBranch("oauth", "login", false)
  })

  it("starts above the branches a running Safe Push holds", async () => {
    robots.spans.auth = { ...RUNNING, id: "login-run", branchName: "login" }
    await pushStack("oauth")
    // the robot of login holds login itself only in its own row; auth it lands
    expect(calls).toEqual(["push login oauth"])
  })

  it("cancels the robot the start replaces at the first branch it holds, and pushes its ancestors with it", async () => {
    const run = { ...RUNNING, id: "oauth-run", branchName: "oauth" }
    robots.spans.auth = run
    robots.spans.login = run
    await pushStack("oauth", { replacesRobot: run, announce: false })
    // canceled once, before the one push of the branches it held
    expect(calls).toEqual(["cancel oauth-run", "push auth login oauth"])
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
    expect(calls).toEqual(["push auth login oauth"])
    expect(records).toEqual(["push auth", "push login", "push oauth"])
    expect(toasts).toHaveLength(1)
    const toast = toasts[0]!
    expect(toast.title).toBe("Pushed auth, login and oauth")
    expect(text(toast.description)).toBe("3 commits · created 3 remote branches")
    expect(toast.actionProps?.children).toBe("Create Pull Request…")
    toast.actionProps!.onClick()
    expect(createPullRequestDialogStore.getState().branchName).toBe("oauth")
    // "Continue on GitHub" of the dialog opens the form into the same base
    expect(getNewPullRequestUrl(stores.branches.oauth!)).toBe(`${COMPARE}/user/virtual/login...user/virtual/oauth?expand=1`)
  })

  it("pushes nothing more from a row whose stack push runs", async () => {
    const first = pushStack("oauth")
    expect(stores.stackPushes).toEqual({ oauth: [] })
    expect(await pushStack("oauth")).toBe(false)
    expect(await first).toBe(true)
    expect(calls).toEqual(["push auth login oauth"])
    expect(stores.stackPushes).toEqual({})
  })

  it("says nothing for a stack push that is a step of a Safe Push start", async () => {
    expect(await pushStack("oauth", { announce: false })).toBe(true)
    expect(calls).toEqual(["push auth login oauth"])
    expect(toasts).toHaveLength(0)
  })

  it("pushes all or none: a remote branch that moved refuses the push, and its toast says what that remote has", async () => {
    stores.movedInStack = [{ branch: "login", commitsBehind: 1 }]
    expect(await pushStack("oauth")).toBe(false)
    expect(calls).toEqual(["push auth login oauth"])
    // leased as the rows showed them: none on the remote
    expect(stores.expectedRemotes).toEqual([null, null, null])
    // the row of login shows its fetched remote
    expect(stores.branches.login!.remoteStatus?.remoteTip).toBe("theirs")
    expect(records).toEqual(["push login"])
    expect(toasts).toHaveLength(1)
    const toast = toasts[0]!
    expect(toast.title).toBe("login changed on the remote")
    expect(toast.description).toBe("1 commit you don't have. Nothing was pushed.")
    // the action pushes login over what its remote has now
    expect(toast.actionProps?.children).toBe("Force Push")
  })

  it("pushes the stack again over the fetched heads when the remote that moved has no commits the branch lacks", async () => {
    stores.movedInStack = [{ branch: "login", commitsBehind: 0 }]
    expect(await pushStack("oauth")).toBe(true)
    expect(calls).toEqual(["push auth login oauth", "push auth login oauth"])
    // the second push leases login on the head the refused one fetched
    expect(stores.expectedRemotes.slice(3)).toEqual([null, "theirs", null])
    expect(toasts.map(toast => toast.title)).toEqual(["Pushed auth, login and oauth"])
  })

  it("pushes up to a commit with the branches below it whole, in one push", async () => {
    expect(
      await pushUpTo([
        { name: "auth", sent: 1, total: 1 },
        { name: "login", sent: 1, total: 1 },
        { name: "oauth", tip: "c", sent: 1, total: 1 },
      ]),
    ).toBe(true)
    expect(calls).toEqual(["push auth login oauth@c"])
    expect(toasts.map(toast => toast.title)).toEqual(["Pushed auth, login and oauth"])
  })
})

describe("a push over a remote that moved", () => {
  beforeEach(() => {
    robots.running = undefined
  })

  it("pushes again over the new remote tip when the remote has no commits the branch lacks: nothing is lost", async () => {
    stores.movedRemotes = [{ commitsBehind: 0, remoteTip: "theirs" }]
    expect(await pushBranch("auth")).toBe(true)
    expect(calls).toEqual(["push auth", "push auth"])
    expect(stores.expectedRemotes).toEqual([null, "theirs"])
    expect(toasts.map(toast => toast.title)).toEqual(["Pushed auth"])
  })

  it("tells a remote with commits the branch lacks, and pushes nothing over it", async () => {
    stores.movedRemotes = [{ commitsBehind: 2, remoteTip: "theirs" }]
    expect(await pushBranch("auth")).toBe(false)
    expect(calls).toEqual(["push auth"])
    expect(toasts.map(toast => toast.title)).toEqual(["auth changed on the remote"])
  })
})

describe("pushForAction", () => {
  beforeEach(() => {
    robots.running = undefined
  })

  it("pushes the branch for the action without a toast, Activity records it", async () => {
    expect(await pushForAction("auth")).toEqual({ pushed: true })
    expect(calls).toEqual(["push auth"])
    expect(toasts).toEqual([])
    expect(records).toEqual(["push auth"])
  })

  it("pushes nothing for a branch the remote has", async () => {
    stores.branches.auth = { ...stores.branches.auth!, remoteStatus: testRemoteStatus({ exists: true, remoteTip: "tip" }) }
    expect(await pushForAction("auth")).toEqual({ pushed: true })
    expect(calls).toEqual([])
  })

  it("pushes the ancestors with work for the remote first, root first", async () => {
    addBranch("login", "auth", false)
    expect(await pushForAction("login")).toEqual({ pushed: true })
    expect(calls).toEqual(["push auth login"])
    expect(toasts).toEqual([])
  })

  it("refuses a push that drops remote commits of the branch or of an ancestor: the row asks first", async () => {
    addBranch("login", "auth")
    stores.branches.auth = {
      ...stores.branches.auth!,
      remoteStatus: testRemoteStatus({ exists: true, commitsAhead: 1, commitsBehind: 2, fastForward: false, remoteTip: "theirs" }),
    }
    expect(await pushForAction("login")).toEqual({
      pushed: false,
      reason: "The remote branch of auth has 2 commits that auth lacks: push it from its row, which shows them",
    })
    expect(calls).toEqual([])
  })

  it("returns the reason of a hold instead of a toast", async () => {
    robots.running = { ...RUNNING }
    const result = await pushForAction("auth")
    expect(result.pushed).toBe(false)
    expect(result.pushed ? null : result.reason).toContain("A Safe Push of auth is running")
    expect(toasts).toEqual([])
    expect(calls).toEqual([])
  })

  it("says a branch gone from the list in place", async () => {
    expect(await pushForAction("gone")).toEqual({ pushed: false, reason: "gone is no longer in the branch list" })
  })
})

describe("formatNewPullRequestBlocker", () => {
  it("says why GitHub's form and Create can't make a new pull request: a live one; a branch not on the remote can, it is pushed first", () => {
    expect(formatNewPullRequestBlocker("auth", null)).toBeNull()
    expect(formatNewPullRequestBlocker("auth", { number: 8, state: "open" } as PullRequest)).toBe("auth already has pull request #8")
    expect(formatNewPullRequestBlocker("auth", { number: 8, state: "merged" } as PullRequest)).toBe("Pull request #8 of auth is merged")
  })
})

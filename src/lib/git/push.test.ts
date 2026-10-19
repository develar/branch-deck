import { createElement, Fragment } from "react"
import type { ReactNode } from "react"
import { renderToStaticMarkup } from "react-dom/server"
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import { formatNewPullRequestBlocker, getNewPullRequestUrl, getRetargetCommand } from "@/lib/git/new-pull-request"
import { pushBranch, pushForAction } from "@/lib/git/push"
import { pushStack, pushUpTo, pushWholeStack, pushWithChildren } from "@/lib/git/stack-push"
import type { Branch, RemoteStatus, StackBranch } from "@/stores/branch-sync"
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
    /** The branches a merged Safe Push of a branch built on them landed: branch -> its robot */
    landed: {} as Record<string, PatronusRobot>,
    /** The running Safe Push of the whole stack of `auth`, which holds the push of its stack branch */
    stack: undefined as PatronusRobot | undefined,
    /** The whole stacks every branch of which merged Safe Pushes landed: root -> why nothing is left */
    stackLanded: {} as Record<string, string>,
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
    /** The branches stacked on a branch, parents first */
    descendants: {} as Record<string, string[]>,
    /** The stack branches of the whole stacks, by root */
    stackBranches: {} as Record<string, StackBranch>,
    /** The pull requests of the whole stacks, by root */
    stackPullRequests: {} as Record<string, PullRequest>,
    /** Resolves when the running sync ends */
    synced: Promise.resolve(),
    /** The pull requests GitHub lists, by branch */
    pullRequests: {} as Record<string, PullRequest>,
    /** The `expectedRemote` of each push */
    expectedRemotes: [] as (string | null)[],
    /** What the next pushes find on the remote instead of pushing (`remoteMoved`), in turn: its commits behind, its tip */
    movedRemotes: [] as { commitsBehind: number; remoteTip: string }[],
    /** The stack pushes wait for it, once they are asked: what a second request sees while a push runs */
    pushGate: Promise.resolve(),
    /** Why the next stack push fails, as the backend says it; null: it pushes */
    pushError: null as string | null,
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
    /** One push of the branches, "push auth login@<tip> stack:auth" (the stack branch of the whole stack of auth) */
    pushBranches: async (params: { branches: { branchName: string; expectedRemote: string | null; tip?: string | null; wholeStack?: boolean }[] }) => {
      const nameOf = (branch: { branchName: string; wholeStack?: boolean }) => (branch.wholeStack === true ? `stack:${branch.branchName}` : branch.branchName)
      calls.push(`push ${params.branches.map(branch => (branch.tip == null ? nameOf(branch) : `${branch.branchName}@${branch.tip}`)).join(" ")}`)
      stores.expectedRemotes.push(...params.branches.map(branch => branch.expectedRemote))
      await stores.pushGate
      const error = stores.pushError
      if (error !== null) {
        stores.pushError = null
        return { status: "error", error }
      }
      const moved = stores.movedInStack.shift()
      // refused, the fetched remote of the moved branch is at their commit, the others where the rows showed them
      const remoteStatuses = params.branches.map(branch => {
        if (moved === undefined) {
          return { branchName: branch.branchName, remoteExists: true, remoteTip: "tip" }
        }
        return nameOf(branch) === moved.branch
          ? { branchName: branch.branchName, remoteExists: true, remoteTip: "theirs", commitsBehind: moved.commitsBehind }
          : { branchName: branch.branchName, remoteExists: branch.expectedRemote !== null, remoteTip: branch.expectedRemote }
      })
      const wholeStack = moved?.branch.startsWith("stack:") === true
      return {
        status: "ok",
        data:
          moved === undefined
            ? { type: "pushed", remoteStatuses }
            : { type: "remoteMoved", branchName: wholeStack ? moved.branch.slice("stack:".length) : moved.branch, wholeStack, remoteStatuses },
      }
    },
  },
}))

vi.mock("@/stores/branch-sync", () => ({
  branchSyncStore: { getState: () => ({ baselineBranch: "origin/master" }) },
  getBranch: (name: string) => stores.branches[name],
  getStackBranch: (root: string) => stores.stackBranches[root],
  setStackBranchRemoteStatus: (root: string, status: RemoteStatus) => {
    stores.stackBranches[root] = { ...stores.stackBranches[root]!, remoteStatus: status }
  },
  getStackDescendants: (name: string) => stores.descendants[name] ?? [],
  isBranchPushing: () => false,
  whenSynced: () => stores.synced,
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
    commitsLanded: 0,
    commitsReworded: 0,
    myCommitsAhead: 0,
    fastForward: true,
    lastPushTime: 1,
    remoteTip: status.remoteTip,
    droppedCommits: [],
  }),
}))

vi.mock("@/stores/patronus", () => ({
  getPushHold: (name: string) => {
    const landed = robots.landed[name]
    if (landed !== undefined) {
      return { kind: "landed", branch: landed.branchName, robot: landed }
    }
    if (name === "auth" && robots.running !== undefined && robots.running.status === "running") {
      return { kind: "own", branch: "auth", robot: robots.running }
    }
    const span = robots.spans[name]
    return span === undefined || span.status !== "running" ? null : { kind: "span", branch: span.branchName, robot: span }
  },
  getPatronusGitHubRepository: () => null,
  getStackPushHold: (root: string) =>
    root === "auth" && robots.stack !== undefined && robots.stack.status === "running" ? { kind: "own", branch: "auth", robot: robots.stack, wholeStack: true } : null,
  getStackLandedReason: (root: string) => robots.stackLanded[root] ?? null,
  getStackBranchHoldReason: (root: string) => {
    if (robots.stackLanded[root] !== undefined) {
      return `${robots.stackLanded[root]}: nothing of it is left to push`
    }
    return root === "auth" && robots.stack !== undefined && robots.stack.status === "running" ? `A Safe Push of the whole ${root} stack is running` : null
  },
  fetchPatronusRobotStatus: async () => robots.status,
  cancelPatronusRobot: async (robot: PatronusRobot) => {
    calls.push(`cancel ${robot.id}`)
    // the store has it canceling: it holds nothing
    for (const held of [robots.running, robots.stack, ...Object.values(robots.spans)]) {
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
  refreshPullRequests: async () => {},
  refreshPullRequestsAfterPush: async () => {},
  refreshPullRequestsAfterPushes: async () => {},
  getPullRequest: (name: string) => stores.pullRequests[name],
  getStackPullRequest: (root: string) => stores.stackPullRequests[root],
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
  recordRetarget: (title: string, description: string) => {
    records.push(`${title}: ${description}`)
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
  robots.landed = {}
  robots.stack = undefined
  robots.stackLanded = {}
  stores.branches = {}
  stores.pullRequests = {}
  stores.expectedRemotes = []
  stores.movedRemotes = []
  stores.retargetError = null
  stores.movedInStack = []
  stores.descendants = {}
  stores.stackBranches = {}
  stores.stackPullRequests = {}
  stores.synced = Promise.resolve()
  stores.pushGate = Promise.resolve()
  stores.pushError = null
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

  it("creates the pull request of a stacked branch into the baseline, its parent on the remote or not", async () => {
    addBranch("auth", null)
    addBranch("login", "auth", false)
    expect(await pushBranch("login")).toBe(true)
    const toast = toasts.at(-1)!
    // every pull request goes into the baseline: the toast names no base
    expect(text(toast.description)).toBe("1 commit · created the remote branch")
    expect(toast.actionProps?.children).toBe("Create Pull Request…")
    toast.actionProps!.onClick()
    expect(createPullRequestDialogStore.getState().branchName).toBe("login")
    // "Continue on GitHub" of the dialog opens the form into the same base, with the commits of auth
    expect(getNewPullRequestUrl(stores.branches.login!)).toBe(`${COMPARE}/master...user/virtual/login?expand=1`)

    addBranch("oauth", "login", false)
    stores.branches.login = { ...stores.branches.login!, remoteStatus: testRemoteStatus({ exists: false }) }
    expect(getNewPullRequestUrl(stores.branches.oauth!)).toBe(`${COMPARE}/master...user/virtual/oauth?expand=1`)
  })

  it("retargets a pull request into a virtual branch to the baseline before the push, and Activity records it", async () => {
    addBranch("auth", null)
    addBranch("login", "auth")
    stores.pullRequests.login = { number: 8, state: "open", baseRefName: "user/virtual/auth", url: "https://github.com/JetBrains/ultimate/pull/8" } as PullRequest
    expect(await pushBranch("login")).toBe(true)
    expect(calls.slice(0, 2)).toEqual(["retarget login master", "push login"])
    const toast = toasts.at(-1)!
    expect(text(toast.description)).toBe("1 commit · retargeted pull request #8 to master")
    expect(toast.actionProps?.children).toBe("Open Pull Request")
    expect(records).toEqual(["Retargeted #8 to master: login · it went into auth", "push login"])
  })

  it("leaves a pull request into a release branch: that base may be meant", async () => {
    addBranch("login", "auth")
    stores.pullRequests.login = { number: 8, state: "open", baseRefName: "release-252", url: "https://github.com/JetBrains/ultimate/pull/8" } as PullRequest
    expect(await pushBranch("login")).toBe(true)
    expect(calls).toEqual(["push login"])
    const toast = toasts.at(-1)!
    expect(text(toast.description)).toBe("1 commit")
    expect(toast.actionProps?.children).toBe("Open Pull Request")
    expect(getRetargetCommand(stores.branches.login!)).toBeNull()
  })

  it("pushes and copies the command that retargets a pull request when the retarget fails", async () => {
    addBranch("auth", null)
    addBranch("login", "auth")
    stores.pullRequests.login = { number: 8, state: "open", baseRefName: "user/virtual/auth" } as PullRequest
    stores.retargetError = "HTTP 403"
    expect(await pushBranch("login")).toBe(true)
    expect(toasts.some(toast => toast.title === "Cannot retarget pull request #8")).toBe(true)
    const toast = toasts.at(-1)!
    expect(text(toast.description)).toBe("1 commit · the pull request goes into auth: retarget it to master")
    expect(toast.actionProps?.children).toBe("Copy gh pr edit")
    toast.actionProps!.onClick()
    await vi.waitFor(() => expect(calls).toContain("copy gh pr edit 8 --base master"))
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

  it("refuses a branch a merged Safe Push of a branch built on it landed, without reading its status", async () => {
    robots.running = undefined
    addBranch("login", "auth")
    // a status read would let the push through: the robot finished
    robots.status = "success"
    robots.landed.auth = { ...RUNNING, id: "login-run", branchName: "login", status: "success", finishTime: 200 }
    expect(await pushBranch("auth")).toBe(false)
    expect(calls).toEqual([])
    expect(text(toasts[0]!.description)).toContain("Landed in master with login: nothing of it is left to push")
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
    // "Continue on GitHub" of the dialog opens the form into the baseline, with the commits of auth and login
    expect(getNewPullRequestUrl(stores.branches.oauth!)).toBe(`${COMPARE}/master...user/virtual/oauth?expand=1`)
  })

  it("names each pull request into a virtual branch the pushes retargeted to the baseline", async () => {
    stores.pullRequests.auth = { number: 802, state: "open", baseRefName: "user/virtual/core", url: "https://github.com/JetBrains/ultimate/pull/802" } as PullRequest
    stores.pullRequests.login = { number: 801, state: "open", baseRefName: "user/virtual/auth", url: "https://github.com/JetBrains/ultimate/pull/801" } as PullRequest
    await pushStack("oauth")
    // each branch's retarget first, then the one push of the stack
    expect(calls).toEqual(["retarget auth master", "retarget login master", "push auth login oauth"])
    expect(text(toasts[0]!.description)).toBe(
      "3 commits · created 3 remote branches · retargeted pull request #802 of auth to master · retargeted pull request #801 of login to master",
    )
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

/** auth with login and session built on it, a stack that forks; its stack branch built, `remote` its remote status */
function forkingStack(remote: RemoteStatus | null): void {
  robots.running = undefined
  addBranch("login", "auth", false)
  addBranch("session", "auth", false)
  stores.descendants.auth = ["login", "session"]
  stores.stackBranches.auth = {
    root: "auth",
    branchName: "user/stack/auth",
    members: ["auth", "login", "session"],
    commitCount: 3,
    state: "Built",
    tip: "stack-tip",
    blockedBy: null,
    conflict: null,
    error: null,
    remoteStatus: remote,
  }
}

/** Holds the stack pushes asked from now on until the returned function lets them go */
function holdPushes(): () => void {
  let release!: () => void
  stores.pushGate = new Promise(resolve => (release = resolve))
  return release
}

/** The stack branch of auth has two commits its remote lacks */
const STACK_AHEAD = { commitsAhead: 2, unpushedCommits: ["a", "b"], remoteTip: "stack-old" }

/** A failed Safe Push of the whole stack of auth, which "Restart Safe Push" pushes first and restarts */
const STACK_ROBOT: PatronusRobot = { ...RUNNING, id: "whole", name: "Safe Push of the whole auth stack", branchName: null, stackRoot: "auth" }

describe("pushWholeStack", () => {
  it("joins a push of the stack branch that runs: one push, both done, Activity records it once", async () => {
    forkingStack(testRemoteStatus(STACK_AHEAD))
    const release = holdPushes()
    const first = pushWholeStack("auth")
    const second = pushWholeStack("auth")
    release()
    expect(await Promise.all([first, second])).toEqual([true, true])
    expect(calls).toEqual(["push stack:auth"])
    expect(records).toEqual(["push the whole stack branch of auth"])
  })

  it("says a failed push once: the request that joined it fails with it", async () => {
    forkingStack(testRemoteStatus(STACK_AHEAD))
    const release = holdPushes()
    stores.pushError = "Connection reset"
    const reports: string[] = []
    const first = pushWholeStack("auth", { report: (title, error) => reports.push(`first: ${title}: ${String(error)}`) })
    const second = pushWholeStack("auth", { report: (title, error) => reports.push(`second: ${title}: ${String(error)}`) })
    release()
    expect(await Promise.all([first, second])).toEqual([false, false])
    expect(calls).toEqual(["push stack:auth"])
    expect(reports).toEqual(["first: Cannot push the whole auth stack: Connection reset"])
  })

  it("pushes what a sync rebuilt while the push it joined ran", async () => {
    forkingStack(testRemoteStatus(STACK_AHEAD))
    const release = holdPushes()
    const first = pushWholeStack("auth")
    let synced!: () => void
    stores.synced = new Promise(resolve => (synced = resolve))
    const second = pushWholeStack("auth")
    release()
    expect(await first).toBe(true)
    // the sync rebuilt the stack branch with a commit the remote lacks
    stores.stackBranches.auth = { ...stores.stackBranches.auth!, remoteStatus: testRemoteStatus({ ...STACK_AHEAD, remoteTip: "tip" }) }
    synced()
    expect(await second).toBe(true)
    expect(calls).toEqual(["push stack:auth", "push stack:auth"])
  })

  it("is one request for a second restart of the same robot: one push, one restart, one toast", async () => {
    forkingStack(testRemoteStatus(STACK_AHEAD))
    robots.stack = { ...STACK_ROBOT }
    const release = holdPushes()
    const first = pushWholeStack("auth", { restartRobot: STACK_ROBOT })
    const second = pushWholeStack("auth", { restartRobot: STACK_ROBOT })
    release()
    expect(await Promise.all([first, second])).toEqual([true, true])
    expect(calls).toEqual(["cancel whole", "push stack:auth", "restart whole"])
    expect(toasts.map(toast => toast.title)).toEqual(["Safe Push restarted"])
  })

  it("restarts after the push it joined, and pushes nothing of its own: the remote has the stack branch", async () => {
    forkingStack(testRemoteStatus(STACK_AHEAD))
    robots.stack = { ...STACK_ROBOT, status: "failure" }
    const release = holdPushes()
    // Request Review… of its pull request pushes it, then Restart Safe Push of its failed robot is asked
    const request = pushWholeStack("auth")
    const restart = pushWholeStack("auth", { restartRobot: STACK_ROBOT })
    release()
    expect(await Promise.all([request, restart])).toEqual([true, true])
    expect(calls).toEqual(["push stack:auth", "restart whole"])
    expect(toasts.map(toast => toast.title)).toEqual(["Safe Push restarted"])
  })

  it("leaves the stack branch to a push of it alone that runs: Push with Children waits, then sends only the branches", async () => {
    forkingStack(testRemoteStatus(STACK_AHEAD))
    const release = holdPushes()
    const alone = pushWholeStack("auth")
    const withChildren = pushWithChildren("auth")
    release()
    expect(await Promise.all([alone, withChildren])).toEqual([true, true])
    expect(calls).toEqual(["push stack:auth", "push auth login session"])
  })

  it("joins Push with Children that takes the stack branch along, and pushes it alone after one that failed", async () => {
    forkingStack(testRemoteStatus(STACK_AHEAD))
    let release = holdPushes()
    let withChildren = pushWithChildren("auth")
    // the request of the stack branch alone comes while the push of all of them runs
    await vi.waitFor(() => expect(calls).toHaveLength(1))
    let alone = pushWholeStack("auth")
    release()
    expect(await Promise.all([withChildren, alone])).toEqual([true, true])
    expect(calls).toEqual(["push auth login session stack:auth"])

    // a push of all of them that fails leaves the stack branch to the request of it alone
    calls.length = 0
    forkingStack(testRemoteStatus(STACK_AHEAD))
    stores.pushError = "Connection reset"
    release = holdPushes()
    withChildren = pushWithChildren("auth")
    await vi.waitFor(() => expect(calls).toHaveLength(1))
    alone = pushWholeStack("auth")
    release()
    expect(await Promise.all([withChildren, alone])).toEqual([false, true])
    // auth is on the remote since the first push
    expect(calls).toEqual(["push login session stack:auth", "push stack:auth"])
  })
})

describe("pushWithChildren", () => {
  it("waits for the sync, then pushes the children as the sync leaves them", async () => {
    robots.running = undefined
    stores.branches.auth = { ...stores.branches.auth!, remoteStatus: testRemoteStatus({ remoteTip: "tip" }) }
    stores.descendants.auth = ["login"]
    let synced!: () => void
    stores.synced = new Promise(resolve => (synced = resolve))
    const pushing = pushWithChildren("auth")
    // the row shows the push while it waits, and nothing goes out
    expect(stores.stackPushes).toEqual({ auth: [] })
    expect(calls).toEqual([])
    // the sync brings a commit of login for the remote; auth is on the remote as it is
    addBranch("login", "auth", false)
    synced()
    expect(await pushing).toBe(true)
    expect(calls).toEqual(["push login"])
    expect(toasts.map(toast => toast.title)).toEqual(["Pushed login"])
  })

  describe("the stack branch of the whole stack", () => {
    it("goes along after the branches once it is on the remote, leased on its remote head, in the one toast", async () => {
      forkingStack(testRemoteStatus({ commitsAhead: 2, unpushedCommits: ["a", "b"], remoteTip: "stack-old" }))
      expect(await pushWithChildren("auth")).toBe(true)
      expect(calls).toEqual(["push auth login session stack:auth"])
      expect(stores.expectedRemotes).toEqual([null, null, null, "stack-old"])
      expect(stores.stackBranches.auth!.remoteStatus?.remoteTip).toBe("tip")
      expect(records).toEqual(["push auth", "push login", "push session", "push the whole stack branch of auth"])
      expect(toasts.map(toast => [toast.title, text(toast.description)])).toEqual([
        ["Pushed auth, login and session", "3 commits · created 3 remote branches · pushed the whole stack branch"],
      ])
    })

    it("goes along while its pull request is open, though the remote doesn't have it yet", async () => {
      forkingStack(testRemoteStatus({ exists: false, commitsAhead: 3, remoteTip: null }))
      stores.stackPullRequests.auth = { number: 12, state: "open", url: "https://github.com/JetBrains/ultimate/pull/12" } as PullRequest
      expect(await pushWithChildren("auth")).toBe(true)
      expect(calls).toEqual(["push auth login session stack:auth"])
      expect(stores.expectedRemotes.at(-1)).toBeNull()
    })

    it("stays local while it is neither on the remote nor in an open pull request, and while the sync didn't build it", async () => {
      forkingStack(testRemoteStatus({ exists: false, commitsAhead: 3, remoteTip: null }))
      stores.stackPullRequests.auth = { number: 12, state: "closed" } as PullRequest
      expect(await pushWithChildren("auth")).toBe(true)
      stores.stackBranches.auth = { ...stores.stackBranches.auth!, state: "Conflict", remoteStatus: testRemoteStatus({ commitsAhead: 2, remoteTip: "stack-old" }) }
      addBranch("login", "auth")
      expect(await pushWithChildren("auth")).toBe(true)
      expect(calls).toEqual(["push auth login session", "push login"])
    })

    it("is pushed alone when the branches are on the remote, with the pull request of the whole stack", async () => {
      forkingStack(testRemoteStatus({ commitsAhead: 2, unpushedCommits: ["a", "b"], remoteTip: "stack-old" }))
      for (const name of ["auth", "login", "session"]) {
        stores.branches[name] = { ...stores.branches[name]!, remoteStatus: testRemoteStatus({ remoteTip: "tip" }) }
      }
      const pullRequest = { number: 12, state: "open", url: "https://github.com/JetBrains/ultimate/pull/12" } as PullRequest
      stores.stackPullRequests.auth = pullRequest
      expect(await pushWithChildren("auth")).toBe(true)
      expect(calls).toEqual(["push stack:auth"])
      const [toast] = toasts
      expect([toast?.title, text(toast?.description), toast?.actionProps?.children]).toEqual(["Pushed the whole stack branch of auth", "2 commits", "Open Pull Request"])
      toast!.actionProps!.onClick()
      expect(calls.at(-1)).toBe(`open ${pullRequest.url}`)
    })

    it("refuses the whole push when its remote moved with commits it lacks, and says so as the stack branch", async () => {
      forkingStack(testRemoteStatus({ commitsAhead: 2, unpushedCommits: ["a", "b"], remoteTip: "stack-old" }))
      stores.movedInStack = [{ branch: "stack:auth", commitsBehind: 1 }]
      expect(await pushWithChildren("auth")).toBe(false)
      expect(stores.stackBranches.auth!.remoteStatus?.remoteTip).toBe("theirs")
      expect(stores.branches.auth!.remoteStatus?.remoteTip).toBeNull()
      expect(records).toEqual(["push the whole stack branch of auth"])
      expect(toasts.map(toast => [toast.title, toast.description])).toEqual([
        ["Cannot push the whole stack branch of auth", "Its remote branch has a commit the stack branch lacks: sync, then push again"],
      ])
    })

    it("is held once merged Safe Pushes landed every branch of the stack: Push with Children leaves it, its push alone says why", async () => {
      forkingStack(testRemoteStatus({ commitsAhead: 2, unpushedCommits: ["a", "b"], remoteTip: "stack-old" }))
      stores.stackPullRequests.auth = { number: 12, state: "open", url: "https://github.com/JetBrains/ultimate/pull/12" } as PullRequest
      // a branch with work goes, the stack branch along: nothing landed yet
      expect(await pushWithChildren("auth")).toBe(true)
      expect(calls).toEqual(["push auth login session stack:auth"])
      calls.length = 0
      toasts.length = 0

      const merged = { ...RUNNING, id: "landed", status: "success" as const, branchName: "login" }
      for (const name of ["auth", "login", "session"]) {
        stores.branches[name] = { ...stores.branches[name]!, remoteStatus: testRemoteStatus({ commitsAhead: 1, unpushedCommits: ["c"], remoteTip: "old" }) }
        robots.landed[name] = merged
      }
      stores.stackBranches.auth = { ...stores.stackBranches.auth!, remoteStatus: testRemoteStatus({ commitsAhead: 2, unpushedCommits: ["a", "b"], remoteTip: "stack-old" }) }
      robots.stackLanded.auth = "Landed in master with login and session"
      // nothing of the stack is left to push: no branch and no stack branch goes
      expect(await pushWithChildren("auth")).toBe(true)
      expect(await pushWholeStack("auth")).toBe(false)
      expect(calls).toEqual([])
      expect(toasts.map(toast => [toast.title, toast.description])).toEqual([
        ["Cannot push the whole auth stack", "Landed in master with login and session: nothing of it is left to push"],
      ])
    })
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

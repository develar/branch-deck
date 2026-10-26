// The pushes of a stack of the page (`stack-push.ts`, until they are intents of the engine, s9-13), over a mocked backend:
// the push of a branch alone is the intent `Push` of the engine (`operations::push`, its tests there and in E2E)
import { createElement, Fragment } from "react"
import type { ReactNode } from "react"
import { renderToStaticMarkup } from "react-dom/server"
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { PatronusRobot, PullRequest, Reason, StackBranchAlong } from "@/lib/bindings"
import { resetEventToasts } from "@/lib/event-toast"
import { pushForAction, pushStack, pushUpTo, pushWholeStack, pushWithChildren } from "@/lib/git/stack-push"
import type { Branch, PushOrders, RemoteStatus, StackBranch } from "@/stores/branch-sync"
import { createPullRequestDialogStore } from "@/stores/create-pull-request-dialog"
import { CREATE, forcePush, PUSH, testRemoteStatus, UP_TO_DATE } from "@/stores/test-branch"
import { unpushedBranch } from "@/test-support/push-mocks"
import { resetToasts, toasts } from "@/test-support/toast-mock"

const { calls, records, robots, stores } = vi.hoisted(() => ({
  /** What reached the backend, Patronus, the clipboard and the browser, in order */
  calls: [] as string[],
  /** What Activity recorded */
  records: [] as string[],
  robots: {
    /** The running Safe Push of `auth`, which holds its push */
    running: undefined as PatronusRobot | undefined,
    /** What Patronus answers the robots of Rust's check of the push: a robot that finished holds nothing */
    status: "running" as string,
    /** The branches a running Safe Push of a branch built on them lands: branch -> its robot */
    spans: {} as Record<string, PatronusRobot>,
    /** The holds the next repository view releases (`whenRepositoryViewCurrent`): a restack placed them below the robot */
    released: [] as string[],
    /** The branches a merged Safe Push of a branch built on them landed: branch -> its robot */
    landed: {} as Record<string, PatronusRobot>,
    /** The running Safe Push of the whole stack of `auth`, which holds the push of its stack branch */
    stack: undefined as PatronusRobot | undefined,
    /** The whole stacks every branch of which merged Safe Pushes landed: root -> why nothing is left */
    stackLanded: {} as Record<string, string>,
    /** The same whole stacks as Rust holds their stack branch: root -> the branches whose robots landed them */
    stackLandedBy: {} as Record<string, string[]>,
    /** Why the push an action needs can't be made, as Rust decides it (`BranchLanding.push`): branch -> the reason */
    pushBlockers: {} as Record<string, Reason>,
    /** The repository of each cancel and restart Patronus was asked, in order */
    askedIn: [] as string[],
    /** The repository of each robot Activity recorded */
    recordedIn: [] as (string | undefined)[],
    /** Runs while Patronus cancels a robot: what the user does meanwhile */
    whileCanceling: () => {},
    /** Runs while the repository view of the robots is brought up to date (`whenRepositoryViewCurrent`) */
    whileViewing: () => {},
  },
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
    /**
     * The remote heads the next pushes went over in Rust, in turn: the remote had moved without commits the branch lacks,
     * so the backend pushed again over it (`pushedOver`); for a stack push, of the branch at `branch`
     */
    pushedOver: [] as { branch: string; remoteTip: string }[],
    /** The stack pushes wait for it, once they are asked: what a second request sees while a push runs */
    pushGate: Promise.resolve(),
    /** Why the next stack push fails, as the backend says it; null: it pushes */
    pushError: null as string | null,
    /** Runs while git pushes: what the user does meanwhile */
    whilePushing: () => {},
    /** Runs while GitHub retargets a pull request: what the user does meanwhile */
    whileRetargeting: () => {},
    /** The repository the main window shows */
    selected: "/repo",
  },
}))

vi.mock("@tauri-apps/api/core", () => import("@/test-support/channel-mock"))

vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))
vi.mock("@tauri-apps/plugin-opener", async () => (await import("@/test-support/push-mocks")).openerMock(calls))
vi.mock("@/lib/clipboard", async () => (await import("@/test-support/push-mocks")).clipboardMock(calls))
vi.mock("@/components/ui/toast", async () => (await import("@/test-support/toast-mock")).toastModule)

/**
 * What the session of the repository does with the outcome of a push: the remote statuses go into the rows and the
 * stack branches it sent, which the page reads from the patch (`whenSessionViewAt`, mocked as resolved)
 */
function applyToSession(branches: readonly { branchName: string; wholeStack?: boolean }[], statuses: readonly RemoteStatusFields[]): void {
  branches.forEach((branch, index) => {
    const status = statuses[index]
    if (status === undefined) {
      return
    }
    if (branch.wholeStack === true) {
      const stack = stores.stackBranches[branch.branchName]
      if (stack !== undefined) {
        stores.stackBranches[branch.branchName] = { ...stack, remoteStatus: remoteStatusOf(status) }
      }
    } else if (stores.branches[branch.branchName] !== undefined) {
      stores.branches[branch.branchName] = { ...stores.branches[branch.branchName]!, remoteStatus: remoteStatusOf(status) }
    }
  })
}

vi.mock("@/model/store", async () => (await import("@/test-support/push-mocks")).modelStoreMock(() => stores.pullRequests))

vi.mock("@/lib/bindings", () => ({
  commands: {
    /** One push of the branches, "push auth login@<tip> stack:auth" (the stack branch of the whole stack of auth) */
    pushBranches: async (params: { branches: { branchName: string; expectedRemote: string | null; tip?: string | null; wholeStack?: boolean }[] }) => {
      const nameOf = (branch: { branchName: string; wholeStack?: boolean }) => (branch.wholeStack === true ? `stack:${branch.branchName}` : branch.branchName)
      calls.push(`push ${params.branches.map(branch => (branch.tip == null ? nameOf(branch) : `${branch.branchName}@${branch.tip}`)).join(" ")}`)
      stores.expectedRemotes.push(...params.branches.map(branch => branch.expectedRemote))
      await stores.pushGate
      stores.whilePushing()
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
      applyToSession(params.branches, remoteStatuses)
      const over = stores.pushedOver.shift()
      // the statuses the push went over: the moved remote at its new head, the others as the rows showed them
      const pushedOver =
        over === undefined
          ? []
          : params.branches.map(branch =>
              nameOf(branch) === over.branch
                ? { branchName: branch.branchName, remoteExists: true, remoteTip: over.remoteTip }
                : { branchName: branch.branchName, remoteExists: branch.expectedRemote !== null, remoteTip: branch.expectedRemote },
            )
      return {
        status: "ok",
        data: {
          outcome:
            moved === undefined
              ? { type: "pushed", remoteStatuses, pushedOver }
              : { type: "remoteMoved", branchName: wholeStack ? moved.branch.slice("stack:".length) : moved.branch, wholeStack, remoteStatuses },
          version: null,
        },
      }
    },
  },
}))

vi.mock("@/stores/branch-sync", async () => {
  const { getPushHold } = await import("@/stores/patronus")
  const { openPullRequestOf } = await import("@/test-support/open-pull-request")
  return {
    branchSyncStore: { getState: () => ({ baselineBranch: "origin/master", branches: stores.branches }), subscribe: () => () => {} },
    getBranch: (name: string) => stores.branches[name],
    getStackBranch: (root: string) => stores.stackBranches[root],
    // the pushes of a branch as Rust orders them (`domain_policy::push::branch_push`, pinned by push_test.rs), over the
    // stores of these tests: a fixture, the page decides none of it
    getPushOrders: (name: string) => pushOrdersOf(name, getPushHold),
    getOpenPullRequest: (name: string) => openPullRequestOf(stores.pullRequests[name]),
    getStackNotBuilt: (root: string): Reason | null => {
      const stack = stores.stackBranches[root]
      if (stack === undefined || stack.members.length === 0) {
        return { code: "noStackBranch", root }
      }
      return stack.state === "Built" && stack.tip !== null
        ? null
        : { code: "stackNotBuilt", root, state: stack.state, blockedBy: stack.blockedBy?.member ?? null, conflict: stack.conflict, error: stack.error }
    },
    getStackDescendants: (name: string) => stores.descendants[name] ?? [],
    getUnparkedStackDescendants: (name: string) => stores.descendants[name] ?? [],
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
    toRemoteStatus: remoteStatusOf,
  }
})

/**
 * The pushes of `name` over the stores of these tests, as Rust orders them: the ancestors with work up to the nearest
 * held one (a landed one left out), the push of an action above the nearest one that asks first, Push with Children of
 * the branch and its descendants with work, and the stack branch along
 */
function pushOrdersOf(name: string, holdOf: (name: string) => { kind: string } | null): PushOrders {
  const work = (it: string) => {
    const branch = stores.branches[it]
    const kind = branch?.remoteStatus?.action.kind
    return branch !== undefined && kind !== undefined && kind !== "upToDate" && branch.status !== "Blocked" && !branch.hasError
  }
  const asks = (it: string) => {
    const action = stores.branches[it]?.remoteStatus?.action
    return work(it) && action?.kind === "force" && action.drops
  }
  const ancestors: string[] = []
  for (
    let base = stores.branches[name]?.stack?.effectiveBase ?? null;
    base !== null && stores.branches[base] !== undefined;
    base = stores.branches[base]?.stack?.effectiveBase ?? null
  ) {
    ancestors.unshift(base)
  }
  const chain = ({ stop }: { stop: boolean }) => {
    const sent: string[] = []
    for (const ancestor of [...ancestors].reverse()) {
      const hold = holdOf(ancestor)
      if (hold?.kind === "landed") {
        continue
      }
      if (hold !== null || (stop && asks(ancestor))) {
        break
      }
      if (work(ancestor)) {
        sent.unshift(ancestor)
      }
    }
    return [...sent, name]
  }
  const action = chain({ stop: true })
  const descendants = stores.descendants[name] ?? []
  const free = (it: string) => holdOf(it) === null
  const stack = stores.stackBranches[name]
  const stackRemote = stack?.remoteStatus ?? null
  let stackBranch: StackBranchAlong | null = null
  if (stack !== undefined && stack.members.length > 0) {
    const hold = stackHoldOf(name)
    if (stackRemote?.exists !== true && stores.stackPullRequests[name]?.state !== "open") {
      stackBranch = { kind: "unpublished" }
    } else if (hold !== null) {
      stackBranch = { kind: "held", reason: hold }
    } else if (stack.state !== "Built" || stack.tip === null) {
      stackBranch = { kind: "notBuilt", reason: { code: "stackNotBuilt", root: name, state: stack.state, blockedBy: null, conflict: stack.conflict, error: stack.error } }
    } else if (stackRemote?.action.kind === "upToDate") {
      stackBranch = { kind: "upToDate" }
    } else {
      stackBranch = stackRemote?.action.kind === "force" && stackRemote.action.drops ? { kind: "dropsRemote" } : { kind: "along" }
    }
  }
  const order = [...(work(name) && free(name) ? [name] : []), ...descendants.filter(it => work(it) && !asks(it) && free(it))]
  return {
    stack: chain({ stop: false }),
    action,
    button: work(name) && !asks(name) && action.length > 1 ? action : [],
    children:
      descendants.length === 0
        ? null
        : { order, askFirst: descendants.filter(it => asks(it) && free(it)), stackBranch, offers: order.some(it => it !== name) || stackBranch?.kind === "along" },
  }
}

/** The remote status of the fields the mocked pushes answer */
interface RemoteStatusFields {
  remoteExists: boolean
  remoteTip: string | null
  commitsBehind?: number
}

function remoteStatusOf(status: RemoteStatusFields): RemoteStatus {
  const commitsBehind = status.commitsBehind ?? 0
  // a remote that moved still lacks the branch's commit
  const commitsAhead = status.remoteTip === "tip" ? 0 : 1
  return {
    exists: status.remoteExists,
    unpushedCommits: [],
    commitsAhead,
    commitsBehind,
    othersBehind: commitsBehind,
    commitsAmended: 0,
    commitsRestacked: 0,
    commitsLanded: 0,
    commitsReworded: 0,
    ancestorCommitsAhead: 0,
    myCommitsAhead: 0,
    fastForward: true,
    lastPushTime: 1,
    remoteTip: status.remoteTip,
    droppedCommits: [],
    // as Rust answers the action of these counts
    action: !status.remoteExists ? CREATE : commitsBehind > 0 ? forcePush({ drops: true }) : commitsAhead > 0 ? PUSH : UP_TO_DATE,
  }
}

/** What holds the push of the stack branch of `root`, as Rust decides it (`StackLanding.hold`) */
function stackHoldOf(root: string): Reason | null {
  const landedBy = robots.stackLandedBy[root]
  if (landedBy !== undefined) {
    return { code: "stackLanded", root, by: landedBy.map(branch => ({ branch, robotId: `landed-${branch}`, wholeStack: false })), own: false, target: "master" }
  }
  return root === "auth" && robots.stack?.status === "running" ? { code: "stackHeld", root, robotId: robots.stack.id, mode: robots.stack.pushMode } : null
}

vi.mock("@/stores/patronus", async () => ({
  patronusStore: (await import("@/test-support/quiet-store")).quietStoreOf({ repositories: {} }),
  getBranchRobot: () => undefined,
  getStackRobot: () => undefined,
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
  getBranchLanding: (name: string) => ({ hold: null, start: null, dryRun: null, replaces: null, push: robots.pushBlockers[name] ?? null }),
  getStackLanding: (root: string) => ({ hold: stackHoldOf(root), start: null, dryRun: null, replaces: null }),
  whenRepositoryViewCurrent: async () => {
    robots.whileViewing()
    robots.spans = Object.fromEntries(Object.entries(robots.spans).filter(([name]) => !robots.released.includes(name)))
  },
}))

// the cancel and the restart of a robot are intents of the engine: what reaches Patronus is the test
vi.mock("@/lib/run-intent", () => ({
  runIntent: async (repositoryPath: string, intent: { kind: string; robotId: string }) => {
    const known = [robots.running, robots.stack, ...Object.values(robots.spans)].find(robot => robot?.id === intent.robotId)
    const robot = known ?? ({ ...RUNNING, id: intent.robotId } as PatronusRobot)
    robots.askedIn.push(repositoryPath)
    if (intent.kind === "cancelRobot") {
      calls.push(`cancel ${robot.id}`)
      robots.whileCanceling()
      // the session has it canceling: it holds nothing
      for (const held of [robots.running, robots.stack, ...Object.values(robots.spans)]) {
        if (held?.id === robot.id) {
          held.status = "canceling"
        }
      }
      return { kind: "ran", outcome: { effects: [], result: { kind: "robot", robot: { ...robot, status: "canceling" }, canceled: null, alreadyFinished: false } } }
    }
    calls.push(`restart ${robot.id}`)
    return {
      kind: "ran",
      outcome: { effects: [], result: { kind: "robot", robot: { ...robot, id: "restarted", url: "https://patronus/restarted" }, canceled: null, alreadyFinished: false } },
    }
  },
  runFailure: () => null,
}))

vi.mock("@/stores/pull-requests", async () => ({
  ...(await import("@/test-support/push-mocks")).pullRequestsMock(() => stores.pullRequests),
  getStackPullRequest: (root: string) => stores.stackPullRequests[root],
  // the engine retargets it into the target, "master", and Activity records it as the run of its intent does
  retargetPullRequest: async (name: string) => {
    calls.push(`retarget ${name} master`)
    stores.whileRetargeting()
    if (stores.retargetError !== null) {
      throw new Error(stores.retargetError)
    }
    const before = stores.pullRequests[name]!
    records.push(`Retargeted #${before.number} to master: ${name} · it went into ${(before.baseRefName ?? "").replace("user/virtual/", "")}`)
    stores.pullRequests[name] = { ...before, baseRefName: "master" }
  },
}))
vi.mock("@/stores/activity", async () => ({
  ...(await import("@/stores/test-store-mocks")).activityMock,
  recordPush: (_repositoryPath: string, branchName: string) => {
    records.push(`push ${branchName}`)
  },
  recordRobot: (title: string, _description: string, _robot: PatronusRobot, repositoryPath?: string) => {
    records.push(title)
    robots.recordedIn.push(repositoryPath)
  },
  recordRetarget: (title: string, description: string) => {
    records.push(`${title}: ${description}`)
  },
}))
vi.mock("@/stores/repository", async () => ({
  repositoryStore: (await import("@/test-support/quiet-store")).quietStore,
  vcsRequestFactory: { createRequest: () => ({ repositoryPath: stores.selected }) },
  getSelectedRepositoryPath: () => stores.selected,
  isSelectedRepository: (path: string) => path === stores.selected,
  getRepositorySwitchReason: (path: string) => (path === stores.selected ? null : `${path} is no longer the open repository`),
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

/** A branch of one commit the remote lacks; `remote: false` when it isn't on the remote yet */
function addBranch(name: string, parent: string | null, remote = true): void {
  stores.branches[name] = unpushedBranch(name, parent, remote, { commits: [{ hash: `${name}-tip` }] as unknown as Branch["commits"], myEmail: "me@example.com" })
}

/** The text of a toast description, which names a branch in medium weight */
function text(description: unknown): string {
  return renderToStaticMarkup(createElement(Fragment, null, description as ReactNode)).replace(/<[^>]+>/g, "")
}

beforeEach(() => {
  calls.length = 0
  records.length = 0
  resetEventToasts()
  resetToasts()
  robots.running = { ...RUNNING }
  robots.status = "running"
  robots.spans = {}
  robots.released = []
  robots.landed = {}
  robots.stack = undefined
  robots.stackLanded = {}
  robots.stackLandedBy = {}
  robots.pushBlockers = {}
  robots.askedIn = []
  robots.recordedIn = []
  robots.whileCanceling = () => {}
  robots.whileViewing = () => {}
  stores.branches = {}
  stores.pullRequests = {}
  stores.expectedRemotes = []
  stores.movedRemotes = []
  stores.pushedOver = []
  stores.retargetError = null
  stores.movedInStack = []
  stores.descendants = {}
  stores.stackBranches = {}
  stores.stackPullRequests = {}
  stores.synced = Promise.resolve()
  stores.pushGate = Promise.resolve()
  stores.pushError = null
  stores.selected = "/repo"
  stores.whilePushing = () => {}
  stores.whileRetargeting = () => {}
  // a root with stacks on: a stack without a parent
  addBranch("auth", null, false)
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

  it("cancels the robot the start replaces at the first branch it holds, and pushes the ancestors its decision pushes with it", async () => {
    const run = { ...RUNNING, id: "oauth-run", branchName: "oauth" }
    robots.spans.auth = run
    robots.spans.login = run
    await pushStack("oauth", { order: ["auth", "login", "oauth"], replacesRobot: run, announce: false })
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

  it("joins a second push of the stack from a row whose stack push runs: one push, both pushed", async () => {
    const first = pushStack("oauth")
    expect(stores.stackPushes).toEqual({ oauth: [] })
    expect(await pushStack("oauth")).toBe(true)
    expect(await first).toBe(true)
    expect(calls).toEqual(["push auth login oauth"])
    expect(stores.stackPushes).toEqual({})
  })

  it("joins the push of an action into the stack push of its row that runs: one push, the action pushed", async () => {
    let synced!: () => void
    stores.synced = new Promise(resolve => (synced = resolve))
    // the push button of oauth, then a Safe Push of oauth while the stack push waits
    const first = pushStack("oauth", { stopAtConfirm: true })
    const action = pushForAction("oauth")
    synced()
    expect(await action).toEqual({ pushed: true })
    expect(await first).toBe(true)
    expect(calls).toEqual(["push auth login oauth"])
  })

  it("says why the stack push an action joined failed, in the action's words", async () => {
    // the action joins while the stack push sends, which the remote refuses
    stores.pushError = "the remote refused it"
    let sent!: () => void
    stores.pushGate = new Promise(resolve => (sent = resolve))
    const failing = pushStack("oauth", { stopAtConfirm: true })
    const refused = pushForAction("oauth")
    await vi.waitFor(() => expect(calls).toEqual(["push auth login oauth"]))
    sent()
    expect(await refused).toEqual({ pushed: false, reason: "the remote refused it" })
    expect(await failing).toBe(false)
    expect(calls).toEqual(["push auth login oauth"])
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

  it("says a stack push Rust made again over a remote that moved from what it went over, in one push of the page", async () => {
    stores.pushedOver = [{ branch: "login", remoteTip: "theirs" }]
    expect(await pushStack("oauth")).toBe(true)
    expect(calls).toEqual(["push auth login oauth"])
    expect(toasts.map(toast => toast.title)).toEqual(["Pushed auth, login and oauth"])
    // login went over the remote it moved to, which had its older commit: a plain push of one commit, as Rust answered
    expect(text(toasts[0]!.description)).toBe("3 commits · created 2 remote branches")
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
const STACK_AHEAD = { commitsAhead: 2, unpushedCommits: ["a", "b"], remoteTip: "stack-old", action: PUSH }

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
      forkingStack(testRemoteStatus({ commitsAhead: 2, unpushedCommits: ["a", "b"], remoteTip: "stack-old", action: PUSH }))
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
      forkingStack(testRemoteStatus({ exists: false, commitsAhead: 3, remoteTip: null, action: CREATE }))
      stores.stackPullRequests.auth = { number: 12, state: "open", url: "https://github.com/JetBrains/ultimate/pull/12" } as PullRequest
      expect(await pushWithChildren("auth")).toBe(true)
      expect(calls).toEqual(["push auth login session stack:auth"])
      expect(stores.expectedRemotes.at(-1)).toBeNull()
    })

    it("stays local while it is neither on the remote nor in an open pull request, and while the sync didn't build it", async () => {
      forkingStack(testRemoteStatus({ exists: false, commitsAhead: 3, remoteTip: null, action: CREATE }))
      stores.stackPullRequests.auth = { number: 12, state: "closed" } as PullRequest
      expect(await pushWithChildren("auth")).toBe(true)
      stores.stackBranches.auth = { ...stores.stackBranches.auth!, state: "Conflict", remoteStatus: testRemoteStatus({ commitsAhead: 2, remoteTip: "stack-old", action: PUSH }) }
      addBranch("login", "auth")
      expect(await pushWithChildren("auth")).toBe(true)
      expect(calls).toEqual(["push auth login session", "push login"])
    })

    it("is pushed alone when the branches are on the remote, with the pull request of the whole stack", async () => {
      forkingStack(testRemoteStatus({ commitsAhead: 2, unpushedCommits: ["a", "b"], remoteTip: "stack-old", action: PUSH }))
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
      forkingStack(testRemoteStatus({ commitsAhead: 2, unpushedCommits: ["a", "b"], remoteTip: "stack-old", action: PUSH }))
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
      forkingStack(testRemoteStatus({ commitsAhead: 2, unpushedCommits: ["a", "b"], remoteTip: "stack-old", action: PUSH }))
      stores.stackPullRequests.auth = { number: 12, state: "open", url: "https://github.com/JetBrains/ultimate/pull/12" } as PullRequest
      // a branch with work goes, the stack branch along: nothing landed yet
      expect(await pushWithChildren("auth")).toBe(true)
      expect(calls).toEqual(["push auth login session stack:auth"])
      calls.length = 0
      toasts.length = 0

      const merged = { ...RUNNING, id: "landed", status: "success" as const, branchName: "login" }
      for (const name of ["auth", "login", "session"]) {
        stores.branches[name] = { ...stores.branches[name]!, remoteStatus: testRemoteStatus({ commitsAhead: 1, unpushedCommits: ["c"], remoteTip: "old", action: PUSH }) }
        robots.landed[name] = merged
      }
      stores.stackBranches.auth = {
        ...stores.stackBranches.auth!,
        remoteStatus: testRemoteStatus({ commitsAhead: 2, unpushedCommits: ["a", "b"], remoteTip: "stack-old", action: PUSH }),
      }
      robots.stackLanded.auth = "Landed in master with login and session"
      robots.stackLandedBy.auth = ["login", "session"]
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
    robots.pushBlockers.login = { code: "dropsRemote", branch: "auth", behind: 2 }
    stores.branches.auth = {
      ...stores.branches.auth!,
      remoteStatus: testRemoteStatus({ exists: true, commitsAhead: 1, commitsBehind: 2, fastForward: false, remoteTip: "theirs", action: forcePush({ drops: true }) }),
    }
    expect(await pushForAction("login")).toEqual({
      pushed: false,
      reason: "The remote branch of auth has 2 commits that auth lacks: push it from its row, which shows them",
    })
    expect(calls).toEqual([])
  })

  it("says a branch gone from the list in place", async () => {
    expect(await pushForAction("gone")).toEqual({ pushed: false, reason: "gone is no longer in the branch list" })
  })

  it("refuses the action of a branch the last sync did not build, or built on one, instead of going on with its remote", async () => {
    // the remote has an older build: "nothing to push" would let the action go on with it
    stores.branches.auth = {
      ...stores.branches.auth!,
      status: "MergeConflict",
      hasError: true,
      statusText: "merge conflict",
      remoteStatus: testRemoteStatus({ exists: true, remoteTip: "tip" }),
    }
    robots.pushBlockers.auth = { code: "cantPush", branch: "auth", block: "Conflict" }
    expect(await pushForAction("auth")).toEqual({ pushed: false, reason: "auth can't be pushed: merge conflict" })
    addBranch("login", "auth", false)
    robots.pushBlockers.login = robots.pushBlockers.auth
    expect(await pushForAction("login")).toEqual({ pushed: false, reason: "auth can't be pushed: merge conflict" })
    expect(calls).toEqual([])
  })
})

describe("a push that started in another repository", () => {
  beforeEach(() => {
    robots.running = undefined
  })

  /** The sync the push waits for ends after the main window opened `/other`, whose `auth` is another branch */
  function switchDuringSync(): () => void {
    let finish!: () => void
    stores.synced = new Promise<void>(resolve => {
      finish = resolve
    })
    return () => {
      stores.selected = "/other"
      finish()
    }
  }

  it("pushes no stack of the same names there", async () => {
    addBranch("login", "auth")
    const finish = switchDuringSync()
    const pushed = pushStack("login")
    finish()
    expect(await pushed).toBe(false)
    expect(calls).toEqual([])
    expect(stores.stackPushes).toEqual({})
  })

  it("refuses the push of an action in place", async () => {
    const finish = switchDuringSync()
    const pushed = pushForAction("auth")
    finish()
    expect(await pushed).toEqual({ pushed: false, reason: "/repo is no longer the open repository" })
    expect(calls).toEqual([])
  })

  /** The user opens `/other`, whose `auth` is another branch */
  function switchRepository(): void {
    stores.selected = "/other"
  }

  it("pushes the branches over the robot a start replaces after a switch during the cancel", async () => {
    addBranch("login", "auth", false)
    const run = { ...RUNNING, id: "login-run", branchName: "login" }
    robots.spans.auth = run
    robots.spans.login = run
    robots.whileCanceling = switchRepository
    expect(await pushStack("login", { order: ["auth", "login"], replacesRobot: run, announce: false })).toBe(true)
    expect(calls).toEqual(["cancel login-run", "push auth login"])
    expect(robots.askedIn).toEqual(["/repo"])
  })

  it("pushes and restarts the whole stack's robot in the repository of the push after a switch during the cancel", async () => {
    forkingStack(testRemoteStatus(STACK_AHEAD))
    robots.stack = { ...STACK_ROBOT }
    robots.whileCanceling = switchRepository
    expect(await pushWholeStack("auth", { restartRobot: STACK_ROBOT })).toBe(true)
    expect(calls).toEqual(["cancel whole", "push stack:auth", "restart whole"])
    expect(robots.askedIn).toEqual(["/repo", "/repo"])
    expect(robots.recordedIn).toEqual(["/repo"])
  })

  it("retargets no pull request of the branch of the same name there after a switch during a retarget of the stack", async () => {
    addBranch("login", "auth")
    addBranch("ui", "login")
    stores.pullRequests.login = { number: 8, state: "open", baseRefName: "user/virtual/auth", url: "u8" } as PullRequest
    stores.pullRequests.ui = { number: 9, state: "open", baseRefName: "user/virtual/login", url: "u9" } as PullRequest
    // while GitHub retargets login, the user opens /other, whose ui has its own pull request into a virtual branch
    stores.whileRetargeting = () => {
      stores.whileRetargeting = () => {}
      switchRepository()
      stores.pullRequests.ui = { number: 42, state: "open", baseRefName: "user/virtual/core", url: "u42" } as PullRequest
    }
    expect(await pushStack("ui")).toBe(false)
    expect(calls).toEqual(["retarget login master"])
    expect(records).toEqual(["Retargeted #8 to master: login · it went into auth"])
    expect(stores.pullRequests.ui?.baseRefName).toBe("user/virtual/core")
    expect(toasts.at(-1)).toMatchObject({ title: "Cannot push ui", description: "/repo is no longer the open repository" })
  })

  /** During the push the user opens /other, whose branch of the same name has its own pull request #42 */
  function switchToBranchWithPullRequest(name: string): void {
    stores.whilePushing = () => {
      switchRepository()
      stores.pullRequests[name] = { number: 42, state: "open", baseRefName: "user/virtual/core", url: "https://github.com/JetBrains/ultimate/pull/42" } as PullRequest
    }
  }

  it("offers no pull request of the branches of the same names there in the toast of a stack push that ends after a switch", async () => {
    addBranch("login", "auth")
    switchToBranchWithPullRequest("login")
    expect(await pushStack("login")).toBe(true)
    const toast = toasts.at(-1)!
    expect(toast.title).toBe("Pushed auth and login")
    expect(text(toast.description)).not.toContain("retarget")
    expect(toast.actionProps).toBeUndefined()
  })
})

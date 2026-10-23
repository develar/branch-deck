// @spec spec/branch/next-step.spec.md
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { StoreApi } from "zustand/vanilla"
import type { BranchReviewReadiness, GetBranchReviewReadinessParams, PullRequest, ReviewReadiness } from "@/lib/bindings"
import { stackReadinessKey } from "@/lib/whole-stack"
import type { StackBranch } from "@/stores/branch-sync"
import {
  READINESS_BATCH_SIZE,
  demandReviewReadiness,
  freshReadiness,
  getReviewReadiness,
  recheckReviewReadiness,
  resetReviewReadiness,
  scheduleReadinessScan,
  showReviewReadiness,
  watchReviewReadiness,
} from "@/stores/review-readiness"

const { state } = vi.hoisted(() => ({
  state: {
    tips: {} as Record<string, string>,
    pullRequests: {} as Record<string, PullRequest>,
    stacks: {} as Record<string, Pick<StackBranch, "state" | "tip" | "members">>,
    stackPullRequests: {} as Record<string, PullRequest>,
    calls: [] as GetBranchReviewReadinessParams[],
    readiness: { kind: "notRequired" } as ReviewReadiness,
    /** Resolves the calls in flight; null answers at once */
    pending: null as (() => void) | null,
    /** The resolvers of the held calls, in the order they were made */
    held: [] as (() => void)[],
    key: "/repo\nuser",
    hold: false,
    refreshes: 0,
  },
}))

const stores = vi.hoisted(() => ({ branchSync: null as unknown, patronus: null as unknown, pullRequests: null as unknown }))

vi.mock("@/lib/bindings", () => ({
  commands: {
    getBranchReviewReadiness: async (params: GetBranchReviewReadinessParams) => {
      state.calls.push(params)
      if (state.hold) {
        await new Promise<void>(resolve => {
          state.held.push(resolve)
          state.pending = () => state.held.splice(0).forEach(release => release())
        })
      }
      const data: BranchReviewReadiness[] = params.branches.map(branch => ({ branchName: branch.branchName, tip: branch.tip, requirement: null, readiness: state.readiness }))
      return { status: "ok", data }
    },
  },
}))

vi.mock("@/lib/branch-review-window", () => ({ rememberReviewGroups: () => {} }))
vi.mock("@/stores/repository", () => import("@/test-support/selected-repository-mock"))

vi.mock("@/stores/branch-sync", async () => {
  const { createStore: create } = await import("zustand/vanilla")
  stores.branchSync = create(() => ({ baselineBranch: "origin/master" }))
  return {
    branchSyncStore: stores.branchSync,
    getBranch: (name: string) => (state.tips[name] === undefined ? undefined : { name, commits: [{ hash: state.tips[name] }] }),
    getBranchTableOrder: () => Object.keys(state.tips),
    getStackBranch: (root: string) => state.stacks[root],
  }
})

vi.mock("@/stores/patronus", async () => {
  const { createStore: create } = await import("zustand/vanilla")
  stores.patronus = create(() => ({}))
  return {
    patronusStore: stores.patronus,
    isPatronusAvailable: () => true,
    getPatronusGitHubRepository: () => "acme/monorepo",
    repositoryKey: (repositoryPath: string, branchPrefix: string) => `${repositoryPath}\n${branchPrefix}`,
    currentKey: () => state.key,
  }
})

vi.mock("@/stores/pull-requests", async () => {
  const { createStore: create } = await import("zustand/vanilla")
  stores.pullRequests = create(() => ({}))
  return {
    pullRequestsStore: stores.pullRequests,
    getPullRequest: (name: string) => state.pullRequests[name],
    getStackPullRequest: (root: string) => state.stackPullRequests[root],
    refreshPullRequests: () => {
      state.refreshes++
      return Promise.resolve()
    },
  }
})

function pullRequest(number: number, reviewers: PullRequest["reviewers"] = []): PullRequest {
  return {
    number,
    title: "t",
    url: `u/${number}`,
    state: "open",
    isDraft: false,
    reviewDecision: null,
    headRefName: "user/virtual/auth",
    baseRefName: "master",
    headRefOid: "tip",
    checks: null,
    updatedAt: 0,
    reviewers,
    author: "me",
  }
}

/** Runs the scans and the answers due */
async function settle(): Promise<void> {
  await vi.runAllTimersAsync()
}

/** The watch, with every branch shown as a row of the screen shows it */
function watchShown(...args: Parameters<typeof watchReviewReadiness>): () => void {
  const hides = Object.keys(state.tips).map(name => showReviewReadiness(name))
  const unwatchReadiness = watchReviewReadiness(...args)
  return () => {
    unwatchReadiness()
    hides.forEach(hide => hide())
  }
}

const missing: ReviewReadiness = { kind: "missing", groups: [], notCounted: [], reason: "approvalsMissing" }

let unwatch: () => void = () => {}

beforeEach(() => {
  vi.useFakeTimers()
  resetReviewReadiness()
  state.tips = { auth: "a1" }
  state.pullRequests = {}
  state.stacks = {}
  state.stackPullRequests = {}
  state.calls = []
  state.readiness = { kind: "notRequired" }
  state.hold = false
  state.pending = null
  state.held = []
  state.key = "/repo\nuser"
  state.refreshes = 0
})

afterEach(() => {
  unwatch()
  vi.useRealTimers()
})

describe("review readiness", () => {
  it("asks for the wanted branches in one batch, with the start the robot would take", async () => {
    state.tips = { auth: "a1", login: "l1" }
    state.pullRequests = { login: pullRequest(7, [{ login: "ilya", state: "requested", requested: true }]) }
    unwatch = watchShown(order => order)
    await settle()
    expect(state.calls).toHaveLength(1)
    expect(state.calls[0]!.branches).toEqual([
      { branchName: "auth", tip: "a1", start: { kind: "rest" } },
      { branchName: "login", tip: "l1", start: { kind: "pullRequest", number: 7 }, reviewers: [{ login: "ilya", state: "requested", requested: true }], author: "me" },
    ])
    expect(state.calls[0]!.recheck).toBe(false)
    expect(getReviewReadiness("auth")).toMatchObject({ readiness: { kind: "notRequired" }, stale: false })
  })

  it("asks for the whole stack its header shows as its stack branch with its pull request, right before its root, only while the sync built it", async () => {
    state.tips = { docs: "d1", air: "a1" }
    state.stacks = { air: { state: "Built", tip: "s1", members: ["air", "side"] } }
    state.stackPullRequests = { air: { ...pullRequest(12), headRefName: "user/stack/air" } }
    const key = stackReadinessKey("air")
    const hide = showReviewReadiness(key)
    unwatch = watchShown(order => order)
    await settle()
    expect(state.calls).toHaveLength(1)
    expect(state.calls[0]!.branches.map(branch => branch.branchName)).toEqual(["docs", key, "air"])
    expect(state.calls[0]!.branches[1]).toEqual({ branchName: key, tip: "s1", source: "user/stack/air", start: { kind: "pullRequest", number: 12 }, reviewers: [], author: "me" })
    expect(getReviewReadiness(key)).toMatchObject({ readiness: { kind: "notRequired" }, tip: "s1", pullRequest: 12, stale: false })

    // a stack branch the sync didn't build is not what its Safe Push would land: nothing to ask for its kept tip
    state.stacks = { air: { state: "Conflict", tip: "s2", members: ["air", "side"] } }
    state.tips = { docs: "d2", air: "a1" }
    scheduleReadinessScan()
    await settle()
    expect(state.calls).toHaveLength(2)
    expect(state.calls[1]!.branches.map(branch => branch.branchName)).toEqual(["docs"])
    hide()
  })

  it("asks nothing again while the input stays: another sync, the same reviews", async () => {
    unwatch = watchShown(order => order)
    await settle()
    ;(stores.branchSync as StoreApi<{ baselineBranch: string }>).setState({ baselineBranch: "origin/master" })
    scheduleReadinessScan()
    await settle()
    expect(state.calls).toHaveLength(1)
  })

  it("keeps the answer of the old input, stale, while a new review loads: the row doesn't flicker", async () => {
    state.readiness = missing
    state.pullRequests = { auth: pullRequest(7) }
    unwatch = watchShown(order => order)
    await settle()
    state.hold = true
    state.readiness = { kind: "approved", groups: [] }
    state.pullRequests = { auth: pullRequest(7, [{ login: "ilya", state: "approved", requested: false }]) }
    scheduleReadinessScan()
    await vi.advanceTimersByTimeAsync(0)
    expect(getReviewReadiness("auth")).toMatchObject({ readiness: missing, stale: true })
    expect(freshReadiness(getReviewReadiness("auth"), "auth")).toBeNull()
    state.pending!()
    await settle()
    expect(getReviewReadiness("auth")).toMatchObject({ readiness: { kind: "approved" }, stale: false })
    expect(state.calls).toHaveLength(2)
  })

  it("keeps one batch in flight, of at most READINESS_BATCH_SIZE branches", async () => {
    state.tips = Object.fromEntries(Array.from({ length: READINESS_BATCH_SIZE + 2 }, (_, index) => [`b${index}`, `t${index}`]))
    state.hold = true
    unwatch = watchShown(order => order)
    await vi.advanceTimersByTimeAsync(0)
    scheduleReadinessScan()
    await vi.advanceTimersByTimeAsync(0)
    expect(state.calls).toHaveLength(1)
    expect(state.calls[0]!.branches).toHaveLength(READINESS_BATCH_SIZE)
    state.hold = false
    state.pending!()
    await settle()
    expect(state.calls.map(call => call.branches.length)).toEqual([READINESS_BATCH_SIZE, 2])
  })

  it("asks a demanded branch at once in its own call while the background batch is in flight", async () => {
    state.tips = Object.fromEntries(Array.from({ length: READINESS_BATCH_SIZE }, (_, index) => [`b${index}`, `t${index}`]))
    state.tips.auth = "a1"
    state.hold = true
    unwatch = watchShown(order => order.filter(name => name !== "auth"))
    await vi.advanceTimersByTimeAsync(0)
    expect(state.calls.map(call => call.branches.length)).toEqual([READINESS_BATCH_SIZE])
    // the Safe Push popover of auth opens: its readiness goes out before the batch answers
    const release = demandReviewReadiness("auth")
    await vi.advanceTimersByTimeAsync(0)
    expect(state.calls.map(call => call.branches.map(branch => branch.branchName))).toEqual([Object.keys(state.tips).filter(name => name !== "auth"), ["auth"]])
    // asked once: another scan while it is in flight sends nothing more
    scheduleReadinessScan()
    await vi.advanceTimersByTimeAsync(0)
    expect(state.calls).toHaveLength(2)
    state.held[1]!()
    await vi.advanceTimersByTimeAsync(0)
    expect(freshReadiness(getReviewReadiness("auth"), "auth")).toMatchObject({ tip: "a1" })
    expect(getReviewReadiness("b0")).toBeUndefined()
    state.hold = false
    state.pending!()
    await settle()
    expect(state.calls).toHaveLength(2)
    release()
  })

  it("keeps the entry when the batch answers what the demand beside it answered: no surface renders again", async () => {
    state.hold = true
    unwatch = watchShown(order => order)
    await vi.advanceTimersByTimeAsync(0)
    const release = demandReviewReadiness("auth")
    await vi.advanceTimersByTimeAsync(0)
    expect(state.calls.map(call => call.branches.map(branch => branch.branchName))).toEqual([["auth"], ["auth"]])
    state.held[1]!()
    await vi.advanceTimersByTimeAsync(0)
    const answered = getReviewReadiness("auth")
    expect(answered).toMatchObject({ stale: false, tip: "a1" })
    state.hold = false
    state.pending!()
    await settle()
    expect(getReviewReadiness("auth")).toBe(answered)
    release()
  })

  it("asks the branches of another repository at once while the batch of the one before is in flight", async () => {
    state.hold = true
    unwatch = watchShown(order => order)
    await vi.advanceTimersByTimeAsync(0)
    expect(state.calls.map(call => call.repositoryPath)).toEqual(["/repo"])
    state.key = "/other\nuser"
    scheduleReadinessScan()
    await vi.advanceTimersByTimeAsync(0)
    expect(state.calls.map(call => call.repositoryPath)).toEqual(["/repo", "/other"])
    state.hold = false
    state.pending!()
    await settle()
  })

  it("asks again with recheck after the pull requests from GitHub, the answer stale meanwhile", async () => {
    unwatch = watchShown(order => order)
    await settle()
    const recheck = recheckReviewReadiness("auth")
    expect(getReviewReadiness("auth")?.stale).toBe(true)
    await recheck
    await settle()
    expect(state.refreshes).toBe(1)
    expect(state.calls.at(-1)).toMatchObject({ branches: [{ branchName: "auth" }], recheck: true })
    expect(getReviewReadiness("auth")?.stale).toBe(false)
  })

  it("tells once when a new review made the waiting pull request ready", async () => {
    state.readiness = {
      kind: "missing",
      groups: [{ name: "IJPL", needed: 1, approvedBy: [], requested: [], suggested: [], changesRequestedBy: [] }],
      notCounted: [],
      reason: "approvalsMissing",
    }
    state.pullRequests = { auth: pullRequest(7, [{ login: "ilya", state: "requested", requested: true }]) }
    const told: string[] = []
    unwatch = watchShown(
      order => order,
      news => told.push(`${news.branchName} #${news.pullRequest}`),
    )
    await settle()
    expect(told).toEqual([])
    state.readiness = { kind: "approved", groups: [{ name: "IJPL", approvedBy: [{ login: "ilya", fullName: "Ilya" }] }] }
    state.pullRequests = { auth: pullRequest(7, [{ login: "ilya", state: "approved", requested: false }]) }
    scheduleReadinessScan()
    await settle()
    expect(told).toEqual(["auth #7"])
    // asked again (a recheck): the same approval is told no second time
    await recheckReviewReadiness("auth")
    await settle()
    expect(told).toEqual(["auth #7"])
  })

  it("drops the recheck of a branch without a tip: the other branches are asked", async () => {
    state.tips = { auth: "a1", login: "l1" }
    unwatch = watchShown(order => order)
    await settle()
    const recheck = recheckReviewReadiness("gone")
    await recheck
    state.tips = { auth: "a2", login: "l1" }
    scheduleReadinessScan()
    await settle()
    expect(state.calls.at(-1)).toMatchObject({ branches: [{ branchName: "auth", tip: "a2" }], recheck: false })
  })

  it("is not fresh for an input the branch moved past while another batch is in flight", async () => {
    state.tips = { auth: "a1", login: "l1" }
    unwatch = watchShown(order => order)
    await settle()
    // login is asked again and held; auth moves meanwhile
    state.hold = true
    state.tips = { auth: "a1", login: "l2" }
    scheduleReadinessScan()
    await vi.advanceTimersByTimeAsync(0)
    state.tips = { auth: "a2", login: "l2" }
    expect(freshReadiness(getReviewReadiness("auth"), "auth")).toBeNull()
    scheduleReadinessScan()
    await vi.advanceTimersByTimeAsync(0)
    expect(getReviewReadiness("auth")?.stale).toBe(true)
    state.hold = false
    state.pending!()
    await settle()
    expect(freshReadiness(getReviewReadiness("auth"), "auth")).toMatchObject({ tip: "a2" })
  })

  it("asks only the branches whose next step depends on the readiness", async () => {
    state.tips = { auth: "a1", login: "l1" }
    unwatch = watchShown(order => order.filter(name => name === "login"))
    await settle()
    expect(state.calls.map(call => call.branches.map(branch => branch.branchName))).toEqual([["login"]])
  })

  it("asks only the branches a surface shows, and keeps asking the hidden ones that wait for an approval", async () => {
    state.tips = { auth: "a1", login: "l1", oauth: "o1" }
    state.pullRequests = { login: pullRequest(8, [{ login: "ilya", state: "requested", requested: true }]) }
    state.readiness = missing
    unwatch = watchReviewReadiness(order => order)
    // the rows of auth and login are on the screen; oauth is in a collapsed stack
    const hideAuth = showReviewReadiness("auth")
    const hideLogin = showReviewReadiness("login")
    await settle()
    expect(state.calls.map(call => call.branches.map(branch => branch.branchName))).toEqual([["auth", "login"]])
    // the stack collapses: auth, without a pull request, is asked no more; login waits for Ilya in #8
    hideAuth()
    hideLogin()
    state.tips = { auth: "a2", login: "l1", oauth: "o1" }
    state.pullRequests = { login: pullRequest(8, [{ login: "ilya", state: "approved", requested: false }]) }
    scheduleReadinessScan()
    await settle()
    expect(state.calls.map(call => call.branches.map(branch => branch.branchName))).toEqual([["auth", "login"], ["login"]])
    // shown again (the stack expands, the preview of oauth): asked for what changed meanwhile, the old answer stale till then
    expect(getReviewReadiness("auth")).toMatchObject({ tip: "a1", stale: false })
    state.hold = true
    const hideOauth = showReviewReadiness("oauth")
    const hideAgain = showReviewReadiness("auth")
    await vi.advanceTimersByTimeAsync(0)
    expect(getReviewReadiness("auth")?.stale).toBe(true)
    state.hold = false
    state.pending!()
    await settle()
    expect(state.calls.at(-1)!.branches.map(branch => branch.branchName)).toEqual(["auth", "oauth"])
    hideOauth()
    hideAgain()
  })
})

// @spec spec/branch/next-step.spec.md
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { StoreApi } from "zustand/vanilla"
import type { BranchReviewReadiness, GetBranchReviewReadinessParams, PullRequest, ReviewReadiness } from "@/lib/bindings"
import {
  READINESS_BATCH_SIZE,
  freshReadiness,
  getReviewReadiness,
  recheckReviewReadiness,
  resetReviewReadiness,
  scheduleReadinessScan,
  watchReviewReadiness,
} from "@/stores/review-readiness"

const { state } = vi.hoisted(() => ({
  state: {
    tips: {} as Record<string, string>,
    pullRequests: {} as Record<string, PullRequest>,
    calls: [] as GetBranchReviewReadinessParams[],
    readiness: { kind: "notRequired" } as ReviewReadiness,
    /** Resolves the call in flight; null answers at once */
    pending: null as (() => void) | null,
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
          state.pending = resolve
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
    currentKey: () => "/repo\nuser",
  }
})

vi.mock("@/stores/pull-requests", async () => {
  const { createStore: create } = await import("zustand/vanilla")
  stores.pullRequests = create(() => ({}))
  return {
    pullRequestsStore: stores.pullRequests,
    getPullRequest: (name: string) => state.pullRequests[name],
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

const missing: ReviewReadiness = { kind: "missing", groups: [], notCounted: [], reason: "approvalsMissing" }

let unwatch: () => void = () => {}

beforeEach(() => {
  vi.useFakeTimers()
  resetReviewReadiness()
  state.tips = { auth: "a1" }
  state.pullRequests = {}
  state.calls = []
  state.readiness = { kind: "notRequired" }
  state.hold = false
  state.pending = null
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
    unwatch = watchReviewReadiness(order => order)
    await settle()
    expect(state.calls).toHaveLength(1)
    expect(state.calls[0]!.branches).toEqual([
      { branchName: "auth", tip: "a1", start: { kind: "rest" } },
      { branchName: "login", tip: "l1", start: { kind: "pullRequest", number: 7 }, reviewers: [{ login: "ilya", state: "requested", requested: true }], author: "me" },
    ])
    expect(state.calls[0]!.recheck).toBe(false)
    expect(getReviewReadiness("auth")).toMatchObject({ readiness: { kind: "notRequired" }, stale: false })
  })

  it("asks nothing again while the input stays: another sync, the same reviews", async () => {
    unwatch = watchReviewReadiness(order => order)
    await settle()
    ;(stores.branchSync as StoreApi<{ baselineBranch: string }>).setState({ baselineBranch: "origin/master" })
    scheduleReadinessScan()
    await settle()
    expect(state.calls).toHaveLength(1)
  })

  it("keeps the answer of the old input, stale, while a new review loads: the row doesn't flicker", async () => {
    state.readiness = missing
    state.pullRequests = { auth: pullRequest(7) }
    unwatch = watchReviewReadiness(order => order)
    await settle()
    state.hold = true
    state.readiness = { kind: "approved", groups: [] }
    state.pullRequests = { auth: pullRequest(7, [{ login: "ilya", state: "approved", requested: false }]) }
    scheduleReadinessScan()
    await vi.advanceTimersByTimeAsync(0)
    expect(getReviewReadiness("auth")).toMatchObject({ readiness: missing, stale: true })
    expect(freshReadiness(getReviewReadiness("auth"))).toBeNull()
    state.pending!()
    await settle()
    expect(getReviewReadiness("auth")).toMatchObject({ readiness: { kind: "approved" }, stale: false })
    expect(state.calls).toHaveLength(2)
  })

  it("keeps one batch in flight, of at most READINESS_BATCH_SIZE branches", async () => {
    state.tips = Object.fromEntries(Array.from({ length: READINESS_BATCH_SIZE + 2 }, (_, index) => [`b${index}`, `t${index}`]))
    state.hold = true
    unwatch = watchReviewReadiness(order => order)
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

  it("asks again with recheck after the pull requests from GitHub, the answer stale meanwhile", async () => {
    unwatch = watchReviewReadiness(order => order)
    await settle()
    const recheck = recheckReviewReadiness("auth")
    expect(getReviewReadiness("auth")?.stale).toBe(true)
    await recheck
    await settle()
    expect(state.refreshes).toBe(1)
    expect(state.calls.at(-1)).toMatchObject({ branches: [{ branchName: "auth" }], recheck: true })
    expect(getReviewReadiness("auth")?.stale).toBe(false)
  })

  it("asks only the branches whose next step depends on the readiness", async () => {
    state.tips = { auth: "a1", login: "l1" }
    unwatch = watchReviewReadiness(order => order.filter(name => name === "login"))
    await settle()
    expect(state.calls.map(call => call.branches.map(branch => branch.branchName))).toEqual([["login"]])
  })
})

// @spec spec/branch/next-step.spec.md
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { internalError } from "@/lib/api-error"
import type { ApiError, Fact, FactsChanges, FactsView, FactState, PullRequest, ReviewReadiness, ReviewReadinessBranch, SubjectReadiness } from "@/lib/bindings"
import { stackReadinessKey } from "@/lib/whole-stack"
import { currentDemand, resetDemand, showReviewReadiness } from "@/model/demand"
import type { StackBranch } from "@/stores/branch-sync"
import { applyReadinessFacts, freshReadiness, getReviewReadiness, recheckReviewReadiness, resetReviewReadiness, watchReviewReadiness } from "@/stores/review-readiness"
import { emptyView } from "@/test-support/fake-sessions"

const { state } = vi.hoisted(() => ({
  state: {
    tips: {} as Record<string, string>,
    pullRequests: {} as Record<string, PullRequest>,
    stacks: {} as Record<string, Pick<StackBranch, "state" | "tip" | "members">>,
    /** What the page asked of the session, in order */
    asked: [] as string[],
  },
}))

vi.mock("@/lib/branch-review-window", () => ({ rememberReviewGroups: () => {} }))
vi.mock("@/stores/repository", () => import("@/test-support/selected-repository-mock"))
vi.mock("@/model/store", () => ({
  refreshSessionFacts: (_path: string, family: string, reason: string, subject?: string) => {
    state.asked.push(`${family} ${reason}${subject === undefined ? "" : ` ${subject}`}`)
    return Promise.resolve(true)
  },
}))

vi.mock("@/stores/branch-sync", async () => {
  const { createStore: create } = await import("zustand/vanilla")
  return {
    branchSyncStore: create(() => ({ baselineBranch: "origin/master" })),
    getBranch: (name: string) => (state.tips[name] === undefined ? undefined : { name, commits: [{ hash: state.tips[name] }] }),
    getBranchTableOrder: () => Object.keys(state.tips),
    getStackBranch: (root: string) => state.stacks[root],
    // where Rust reads the open pull request goes: these go into the target
    getOpenPullRequest: (subject: string) => (state.pullRequests[subject] === undefined ? undefined : { number: state.pullRequests[subject].number, base: { kind: "target" } }),
  }
})

vi.mock("@/stores/patronus", async () => {
  const { createStore: create } = await import("zustand/vanilla")
  return {
    patronusStore: create(() => ({})),
    askRobotProblemsAgain: () => {
      state.asked.push("robotProblems manual")
    },
    getPatronusGitHubRepository: () => "acme/monorepo",
    repositoryKey: (repositoryPath: string, branchPrefix: string) => `${repositoryPath}\n${branchPrefix}`,
    currentKey: () => "/repo\nuser",
  }
})

vi.mock("@/stores/pull-requests", async () => {
  const { createStore: create } = await import("zustand/vanilla")
  return {
    pullRequestsStore: create(() => ({})),
    getPullRequest: (name: string) => state.pullRequests[name],
    getStackPullRequest: () => undefined,
    refreshPullRequests: () => {
      state.asked.push("pullRequests manual")
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

/** The ask of `auth` the session made: its tip `t1`, from #7 with its reviewers */
function ask(tip = "t1"): ReviewReadinessBranch {
  return { branchName: "auth", tip, start: { kind: "pullRequest", number: 7 }, reviewers: state.pullRequests.auth?.reviewers ?? [], author: "me" }
}

function fact(state: FactState, readiness: ReviewReadiness | null, asked = ask()): Fact<SubjectReadiness> {
  return { value: readiness === null ? null : { ask: asked, requirement: null, readiness }, state, seq: 1, observedAt: 1 }
}

function facts(readiness: Record<string, Fact<SubjectReadiness>>): FactsView {
  return { ...emptyView().facts, readiness }
}

const missing: ReviewReadiness = { kind: "missing", groups: [], notCounted: [], reason: "approvalsMissing" }

beforeEach(() => {
  state.tips = { auth: "t1", ui: "u1" }
  state.pullRequests = { auth: pullRequest(7) }
  state.stacks = {}
  state.asked = []
})

afterEach(() => {
  resetReviewReadiness()
  resetDemand()
})

describe("review readiness", () => {
  it("shows the readiness the session answered for the input the branch has, and none while it loads", () => {
    applyReadinessFacts("/repo", facts({ auth: fact({ kind: "fresh" }, missing), ui: fact({ kind: "loading" }, null) }), null)
    const entry = getReviewReadiness("auth")
    expect(entry).toMatchObject({ readiness: missing, stale: false, tip: "t1", pullRequest: 7 })
    expect(freshReadiness(entry, "auth")).toBe(entry)
    expect(getReviewReadiness("ui")).toBeUndefined()
  })

  it("keeps the answer of the old input, stale, while a new review loads: the row doesn't flicker", () => {
    applyReadinessFacts("/repo", facts({ auth: fact({ kind: "fresh" }, missing), ui: fact({ kind: "fresh" }, { kind: "notRequired" }, { ...ask("u1"), branchName: "ui" }) }), null)
    const ui = getReviewReadiness("ui")
    const change: FactsChanges = { readiness: { auth: fact({ kind: "stale", reason: null }, missing) } }
    applyReadinessFacts("/repo", facts({ ...change.readiness }), change)
    expect(getReviewReadiness("auth")).toMatchObject({ readiness: missing, stale: true })
    expect(freshReadiness(getReviewReadiness("auth"), "auth")).toBeNull()
    // a subject the patch doesn't name keeps its object; one it removes goes
    expect(getReviewReadiness("ui")).toBe(ui)
    applyReadinessFacts("/repo", facts({}), { removedReadiness: ["ui"] })
    expect(getReviewReadiness("ui")).toBeUndefined()
  })

  it("is unavailable when the session can't ask Patronus, and when its read failed after an answer", () => {
    const loggedOut: ApiError = { ...internalError("Run ./safePush.cmd once to log in to Patronus"), code: "not_logged_in" }
    applyReadinessFacts("/repo", facts({ auth: fact({ kind: "unavailable", reason: loggedOut }, null) }), null)
    const unavailable = getReviewReadiness("auth")
    expect(unavailable?.readiness).toEqual({ kind: "unavailable", reason: "notLoggedIn", message: loggedOut.message })
    // an answer of no input: whatever the branch has now, the form doesn't wait for it
    state.tips.auth = "t2"
    expect(freshReadiness(unavailable, "auth")).toBe(unavailable)

    const failed: ApiError = { ...internalError("Patronus answered 500"), code: "patronus_error" }
    applyReadinessFacts("/repo", facts({ auth: fact({ kind: "stale", reason: failed }, missing, ask("t2")) }), null)
    expect(getReviewReadiness("auth")).toMatchObject({ readiness: { kind: "unavailable", reason: "error", message: "Patronus answered 500" }, stale: false })
  })

  it("is not fresh for an input the branch moved past before the session's patch of it", () => {
    applyReadinessFacts("/repo", facts({ auth: fact({ kind: "fresh" }, missing) }), null)
    state.tips.auth = "t2"
    expect(freshReadiness(getReviewReadiness("auth"), "auth")).toBeNull()
    state.tips.auth = "t1"
    // a new review is another input too
    state.pullRequests.auth = pullRequest(7, [{ login: "ikorennoy", state: "approved", requested: false }])
    expect(freshReadiness(getReviewReadiness("auth"), "auth")).toBeNull()
  })

  it("asks again with recheck after the pull requests from GitHub, the answer stale meanwhile", async () => {
    applyReadinessFacts("/repo", facts({ auth: fact({ kind: "fresh" }, missing) }), null)
    const recheck = recheckReviewReadiness("auth")
    expect(getReviewReadiness("auth")?.stale).toBe(true)
    await recheck
    expect(state.asked).toEqual(["robotProblems manual", "pullRequests manual", "readiness manual auth"])
  })

  it("tells the session the shown subjects whose step waits for the readiness, the whole stack by its root", async () => {
    state.stacks = { auth: { state: "Built", tip: "s1", members: ["auth", "ui"] } }
    const hides = [showReviewReadiness("ui"), showReviewReadiness(stackReadinessKey("auth")), showReviewReadiness("auth")]
    const stop = watchReviewReadiness(order => order.filter(key => key !== "ui"))
    await Promise.resolve()
    expect(currentDemand()).toMatchObject({ visibleBranches: ["auth"], visibleStacks: ["auth"] })
    hides.forEach(hide => hide())
    await Promise.resolve()
    expect(currentDemand()).toMatchObject({ visibleBranches: [], visibleStacks: [] })
    stop()
  })
})

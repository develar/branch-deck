import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { internalError } from "@/lib/api-error"
import type { Fact, FactsView, PullRequest, PullRequestAvailability } from "@/lib/bindings"
import {
  applyPullRequestsFacts,
  getGitHubRepository,
  getPullRequest,
  getPullRequestListing,
  isPullRequestsLoading,
  peekBranchRules,
  pullRequestsStore,
  refreshPullRequests,
  resetPullRequests,
  retargetPullRequest,
} from "@/stores/pull-requests"
import { emptyView } from "@/test-support/fake-sessions"

const { session } = vi.hoisted(() => ({
  session: {
    /** The writes that reached GitHub, with the repository each named */
    writes: [] as string[],
    /** What the page asked the session to read again, with the repository */
    refreshed: [] as string[],
  },
}))

// the retarget is an intent of the engine: what reaches it is the test
vi.mock("@/lib/run-intent", () => ({
  runIntent: async (repositoryPath: string, intent: { kind: string; number: number }) => {
    session.writes.push(`${intent.kind} #${intent.number} in ${repositoryPath}`)
    return { kind: "ran" }
  },
  runFailure: () => null,
}))

vi.mock("@/model/store", () => ({
  refreshSessionFacts: async (repositoryPath: string, family: string, reason: string) => {
    session.refreshed.push(`${family} ${reason} in ${repositoryPath}`)
    return true
  },
  whenSessionViewAt: async () => {},
}))

vi.mock("@/stores/repository", () => import("@/test-support/selected-repository-mock"))

function pullRequest(branch: string, number = 1): PullRequest {
  return {
    number,
    title: "t",
    url: "u",
    state: "open",
    isDraft: false,
    reviewDecision: null,
    headRefName: `user/virtual/${branch}`,
    baseRefName: "master",
    headRefOid: "tip",
    checks: null,
    updatedAt: 0,
    reviewers: [],
    author: "me",
  }
}

const AVAILABLE: PullRequestAvailability = { status: "available", owner: "JetBrains", name: "ultimate" }

/** The facts of a view whose pull requests head is `head` with `pullRequests` */
function facts(head: Fact<PullRequestAvailability>, pullRequests: Record<string, PullRequest> = {}): FactsView {
  return { ...emptyView().facts, pullRequests: { head, repository: null, viewerPermission: null, pullRequests, stackPullRequests: {} } }
}

function head(value: PullRequestAvailability | null, state: Fact<PullRequestAvailability>["state"]): Fact<PullRequestAvailability> {
  return { value, state, seq: 1, observedAt: 1000 }
}

const LISTED = facts(head(AVAILABLE, { kind: "fresh" }), { auth: pullRequest("auth") })

beforeEach(() => {
  resetPullRequests()
  session.writes = []
  session.refreshed = []
})

describe("applyPullRequestsFacts", () => {
  it("is loading until GitHub's first answer, which a failure is too: no pull request is known meanwhile", () => {
    applyPullRequestsFacts("/repo", facts(head(null, { kind: "loading" })), null)
    expect(isPullRequestsLoading()).toBe(true)
    expect(getPullRequestListing()).toBe("loading")
    applyPullRequestsFacts("/repo", facts(head({ status: "error", message: "offline" }, { kind: "unavailable", reason: internalError("offline") })), null)
    expect(isPullRequestsLoading()).toBe(false)
    expect(getPullRequestListing()).toBe("unlisted")
  })

  it("keeps the last listing through a refresh GitHub failed for a while, never once its token is gone", () => {
    applyPullRequestsFacts("/repo", LISTED, null)
    expect(getPullRequestListing()).toBe("listed")
    const limited = facts(head({ status: "rateLimited", retryAt: 1 }, { kind: "stale", reason: internalError("rate limit") }), LISTED.pullRequests.pullRequests)
    applyPullRequestsFacts("/repo", limited, { pullRequests: { head: limited.pullRequests.head } })
    expect(getPullRequestListing()).toBe("listed")
    expect(getPullRequest("auth")?.number).toBe(1)
    const gone = facts(head({ status: "signedOut" }, { kind: "unavailable", reason: internalError("No GitHub token") }))
    applyPullRequestsFacts("/repo", gone, { pullRequests: { head: gone.pullRequests.head, removedPullRequests: ["auth"] } })
    expect(getPullRequestListing()).toBe("unlisted")
    expect(getPullRequest("auth")).toBeUndefined()
  })

  it("names the GitHub repository while GitHub lists its pull requests", () => {
    applyPullRequestsFacts("/repo", LISTED, null)
    expect(getGitHubRepository()).toEqual({ owner: "JetBrains", name: "ultimate" })
    applyPullRequestsFacts("/repo", facts(head({ status: "notGitHub" }, { kind: "fresh" })), null)
    expect(getGitHubRepository()).toBeNull()
    expect(getPullRequestListing()).toBe("unlisted")
  })

  it("stores the pull requests of the view as they are, the record kept while a patch names none", () => {
    applyPullRequestsFacts("/repo", LISTED, null)
    const stored = pullRequestsStore.getState().repositories["/repo\nuser"]!
    expect(stored.pullRequests).toBe(LISTED.pullRequests.pullRequests)
    applyPullRequestsFacts("/repo", LISTED, { robots: {} })
    expect(pullRequestsStore.getState().repositories["/repo\nuser"]).toBe(stored)
  })

  it("reads the rules of a base the session read, none before", () => {
    expect(peekBranchRules("master")).toBeNull()
    const rules = { master: { value: { dismissStaleReviewsOnPush: true }, state: { kind: "fresh" as const }, seq: 1, observedAt: 1 } }
    applyPullRequestsFacts("/repo", { ...LISTED, rules }, { rules })
    expect(peekBranchRules("master")).toBe("dismiss")
  })
})

describe("the writes and the refresh", () => {
  it("asks the session for the pull requests, the refresh button showing it runs", async () => {
    applyPullRequestsFacts("/repo", LISTED, null)
    const asked = refreshPullRequests("manual")
    expect(pullRequestsStore.getState().repositories["/repo\nuser"]?.isRefreshing).toBe(true)
    await asked
    expect(pullRequestsStore.getState().repositories["/repo\nuser"]?.isRefreshing).toBe(false)
    await refreshPullRequests("afterWrite")
    expect(session.refreshed).toEqual(["pullRequests manual in /repo", "pullRequests afterWrite in /repo"])
  })

  it("retargets the pull request of a branch through the engine, in the repository of its listing", async () => {
    applyPullRequestsFacts("/repo", LISTED, null)
    await retargetPullRequest("auth")
    expect(session.writes).toEqual(["retarget #1 in /repo"])
  })
})

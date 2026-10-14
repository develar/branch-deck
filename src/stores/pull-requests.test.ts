import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import type { PullRequest, PullRequestsResult } from "@/lib/bindings"
import { FOCUS_REFRESH_AGE_MS, MIN_REFRESH_GAP_MS, PUSH_RECHECK_DELAY_MS, getGitHubRepository, getPullRequest, pullRequestsStore, refreshPullRequests, refreshPullRequestsAfterPush, resetPullRequests } from "@/stores/pull-requests"

const { gh } = vi.hoisted(() => ({
  gh: {
    calls: 0,
    result: null as PullRequestsResult | null,
    /** Resolves the call in flight; null answers at once */
    pending: null as (() => void) | null,
    hold: false,
  },
}))

vi.mock("@/lib/bindings", () => ({
  commands: {
    getPullRequests: async () => {
      gh.calls++
      if (gh.hold) {
        await new Promise<void>((resolve) => {
          gh.pending = resolve
        })
      }
      return { status: "ok", data: gh.result }
    },
  },
}))

vi.mock("@/stores/repository", () => import("@/test-support/selected-repository-mock"))

function pullRequest(headRefOid: string): PullRequest {
  return {
    number: 1,
    title: "t",
    url: "u",
    state: "open",
    isDraft: false,
    reviewDecision: null,
    headRefName: "user/virtual/auth",
    headRefOid,
    checks: null,
    updatedAt: 0,
    reviewers: [],
  }
}

function answer(headRefOid = "tip"): PullRequestsResult {
  return { availability: { status: "available", owner: "JetBrains", name: "ultimate" }, pullRequests: { auth: pullRequest(headRefOid) } }
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(1_000_000)
  resetPullRequests()
  gh.calls = 0
  gh.result = answer()
  gh.hold = false
  gh.pending = null
})

afterEach(() => {
  vi.useRealTimers()
})

describe("refreshPullRequests", () => {
  it("asks once a minute at most after syncs", async () => {
    await refreshPullRequests("sync")
    expect(gh.calls).toBe(1)
    expect(getPullRequest("auth")?.headRefOid).toBe("tip")

    vi.advanceTimersByTime(MIN_REFRESH_GAP_MS - 1)
    await refreshPullRequests("sync")
    expect(gh.calls).toBe(1)

    vi.advanceTimersByTime(1)
    await refreshPullRequests("sync")
    expect(gh.calls).toBe(2)
  })

  it("asks on focus only when the pull requests are older than two minutes", async () => {
    await refreshPullRequests("focus")
    expect(gh.calls).toBe(1)

    vi.advanceTimersByTime(MIN_REFRESH_GAP_MS)
    await refreshPullRequests("focus")
    expect(gh.calls).toBe(1)

    vi.advanceTimersByTime(FOCUS_REFRESH_AGE_MS - MIN_REFRESH_GAP_MS)
    await refreshPullRequests("focus")
    expect(gh.calls).toBe(2)
  })

  it("asks right after a push and on a manual refresh, ignoring the gap", async () => {
    await refreshPullRequests("sync")
    await refreshPullRequests("push")
    await refreshPullRequests("manual")
    expect(gh.calls).toBe(3)
  })

  it("keeps one request in flight; a push during it asks again after it", async () => {
    gh.hold = true
    const first = refreshPullRequests("sync")
    await vi.advanceTimersByTimeAsync(0)
    expect(pullRequestsStore.getState().repositories["/repo\nuser"]?.isRefreshing).toBe(true)

    // a sync and a focus during the request add nothing
    void refreshPullRequests("sync")
    void refreshPullRequests("focus")
    void refreshPullRequests("push")
    void refreshPullRequests("push")
    expect(gh.calls).toBe(1)

    gh.hold = false
    gh.pending!()
    await first
    // the follow-up for both pushes
    expect(gh.calls).toBe(2)
    expect(pullRequestsStore.getState().repositories["/repo\nuser"]?.isRefreshing).toBe(false)
  })

  it("keeps the shown pull requests when a refresh fails", async () => {
    await refreshPullRequests("sync")
    gh.result = null
    const { commands } = await import("@/lib/bindings")
    const spy = vi.spyOn(commands, "getPullRequests").mockResolvedValueOnce({ status: "error", error: { message: "offline" } } as never)
    await refreshPullRequests("manual")
    spy.mockRestore()
    const entry = pullRequestsStore.getState().repositories["/repo\nuser"]!
    expect(entry.availability).toEqual({ status: "error", message: "offline" })
    expect(entry.pullRequests.auth?.number).toBe(1)
  })

  it("names the GitHub repository while gh lists its pull requests", async () => {
    expect(getGitHubRepository()).toBeNull()
    await refreshPullRequests("sync")
    expect(getGitHubRepository()).toEqual({ owner: "JetBrains", name: "ultimate" })

    gh.result = { availability: { status: "notAuthenticated" }, pullRequests: {} }
    await refreshPullRequests("manual")
    expect(getGitHubRepository()).toBeNull()
  })
})

describe("refreshPullRequestsAfterPush", () => {
  it("looks again a little later while GitHub still shows the old head", async () => {
    gh.result = answer("old")
    await refreshPullRequestsAfterPush("auth", "new")
    expect(gh.calls).toBe(1)

    gh.result = answer("new")
    await vi.advanceTimersByTimeAsync(PUSH_RECHECK_DELAY_MS)
    expect(gh.calls).toBe(2)
    expect(getPullRequest("auth")?.headRefOid).toBe("new")
  })

  it("does not look again when the pull request shows the pushed commit", async () => {
    await refreshPullRequestsAfterPush("auth", "tip")
    await vi.advanceTimersByTimeAsync(PUSH_RECHECK_DELAY_MS)
    expect(gh.calls).toBe(1)
  })
})

import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { PullRequest, PullRequestsResult } from "@/lib/bindings"
import {
  FOCUS_REFRESH_AGE_MS,
  MIN_REFRESH_GAP_MS,
  PUSH_RECHECK_DELAY_MS,
  getGitHubRepository,
  getPullRequest,
  mergePullRequests,
  pullRequestsStore,
  refreshPullRequests,
  refreshPullRequestsAfterPush,
  requestReviewers,
  resetPullRequests,
  retargetPullRequest,
  createPullRequest,
} from "@/stores/pull-requests"

const { gh } = vi.hoisted(() => ({
  gh: {
    calls: 0,
    result: null as PullRequestsResult | null,
    /** Resolves the call in flight; null answers at once */
    pending: null as (() => void) | null,
    hold: false,
    /** The writes that reached GitHub, with the repository each named */
    writes: [] as string[],
    /** The repository of each list of the pull requests, in order */
    listedIn: [] as string[],
  },
}))

vi.mock("@/lib/bindings", () => ({
  commands: {
    getPullRequests: async (params: { repositoryPath: string }) => {
      gh.calls++
      gh.listedIn.push(params.repositoryPath)
      // what GitHub had when it was asked
      const result = gh.result
      if (gh.hold) {
        await new Promise<void>(resolve => {
          gh.pending = resolve
        })
      }
      return { status: "ok", data: result }
    },
    createPullRequest: async (params: { repositoryPath: string; branchPrefix: string; branchName: string }) => {
      gh.writes.push(`create ${params.branchName} in ${params.repositoryPath} (${params.branchPrefix})`)
      return { status: "ok", data: { created: true, pullRequest: { ...pullRequest("tip"), number: 9 }, reviewers: [], assigned: false } }
    },
    retargetPullRequest: async (params: { repositoryPath: string; pullRequestNumber: number; base: string }) => {
      gh.writes.push(`retarget #${params.pullRequestNumber} onto ${params.base} in ${params.repositoryPath}`)
      return { status: "ok", data: null }
    },
    requestPullRequestReviewers: async (params: { repositoryPath: string; pullRequestNumber: number; reviewers: string[] }) => {
      gh.writes.push(`request ${params.reviewers.join(" ")} on #${params.pullRequestNumber} in ${params.repositoryPath}`)
      return { status: "ok", data: { requested: params.reviewers, refused: [] } }
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
    author: "me",
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
  gh.writes = []
  gh.listedIn = []
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

  it("keeps a retarget made while a refresh is in flight, then asks GitHub again", async () => {
    gh.result = { ...answer(), pullRequests: { auth: { ...pullRequest("tip"), baseRefName: "user/virtual/core" } } }
    await refreshPullRequests("manual")
    gh.hold = true
    vi.setSystemTime(1_000_000 + MIN_REFRESH_GAP_MS)
    const held = refreshPullRequests("sync")
    await vi.advanceTimersByTimeAsync(0)
    expect(gh.calls).toBe(2)
    // the search in flight answered before the retarget: its base is the old one
    await retargetPullRequest("auth", "master")
    expect(getPullRequest("auth")?.baseRefName).toBe("master")
    gh.result = { ...answer(), pullRequests: { auth: { ...pullRequest("tip"), baseRefName: "master", title: "retargeted" } } }
    gh.pending!()
    await vi.advanceTimersByTimeAsync(0)
    // the old answer arrived: the retarget stays, and the retarget asked once more after it
    expect(getPullRequest("auth")?.baseRefName).toBe("master")
    expect(gh.calls).toBe(3)
    gh.hold = false
    gh.pending!()
    await held
    // GitHub's answer after the retarget is the truth
    expect(getPullRequest("auth")).toMatchObject({ baseRefName: "master", title: "retargeted" })
    expect(gh.writes).toEqual(["retarget #1 onto master in /repo"])
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

  it("names the GitHub repository while GitHub lists its pull requests", async () => {
    expect(getGitHubRepository()).toBeNull()
    await refreshPullRequests("sync")
    expect(getGitHubRepository()).toEqual({ owner: "JetBrains", name: "ultimate" })

    gh.result = { availability: { status: "notAuthenticated", source: "gh" }, pullRequests: {} }
    await refreshPullRequests("manual")
    expect(getGitHubRepository()).toBeNull()
  })
})

describe("refreshPullRequestsAfterPush", () => {
  const selected = { repositoryPath: "/repo", branchPrefix: "user" }

  it("looks again a little later while GitHub still shows the old head", async () => {
    gh.result = answer("old")
    await refreshPullRequestsAfterPush("auth", "new", selected)
    expect(gh.calls).toBe(1)

    gh.result = answer("new")
    await vi.advanceTimersByTimeAsync(PUSH_RECHECK_DELAY_MS)
    expect(gh.calls).toBe(2)
    expect(getPullRequest("auth")?.headRefOid).toBe("new")
  })

  it("does not look again when the pull request shows the pushed commit", async () => {
    await refreshPullRequestsAfterPush("auth", "tip", selected)
    await vi.advanceTimersByTimeAsync(PUSH_RECHECK_DELAY_MS)
    expect(gh.calls).toBe(1)
  })

  it("refreshes the repository the push went to, after a switch too, never the selected one", async () => {
    gh.result = answer("old")
    await refreshPullRequestsAfterPush("auth", "new", { repositoryPath: "/other", branchPrefix: "them" })
    await vi.advanceTimersByTimeAsync(PUSH_RECHECK_DELAY_MS)
    // its pull request still showed the old head: the second look goes there too
    expect(gh.listedIn).toEqual(["/other", "/other"])
    expect(pullRequestsStore.getState().repositories["/other\nthem"]?.pullRequests.auth?.headRefOid).toBe("old")
    expect(getPullRequest("auth")).toBeUndefined()
  })
})

describe("mergePullRequests", () => {
  it("keeps the pull requests a refresh answers unchanged, and the record when none changed", () => {
    const previous = { auth: pullRequest("tip"), login: pullRequest("old") }
    expect(mergePullRequests(previous, { auth: pullRequest("tip"), login: pullRequest("old") })).toBe(previous)

    const next = mergePullRequests(previous, { auth: pullRequest("tip"), login: pullRequest("new") })
    expect(next).not.toBe(previous)
    expect(next.auth).toBe(previous.auth)
    expect(next.login!.headRefOid).toBe("new")

    expect(mergePullRequests(previous, { auth: pullRequest("tip") })).toEqual({ auth: previous.auth })
  })
})

describe("the writes of a pull request asked in another repository than the selected one", () => {
  const other = { repositoryPath: "/other", branchPrefix: "them" }

  it("creates the pull request in the repository the dialog was opened in", async () => {
    await refreshPullRequests("manual")
    const request = { ...other, branchName: "auth", head: "them/virtual/auth", base: "master", title: "t", body: "", reviewers: [], assignSelf: false, waitForMirror: false }
    await createPullRequest(request)
    expect(gh.writes).toEqual(["create auth in /other (them)"])
    // the selected repository's auth keeps its own pull request; the new one is the other repository's
    expect(getPullRequest("auth")?.number).toBe(1)
    expect(pullRequestsStore.getState().repositories["/other\nthem"]?.pullRequests.auth?.number).toBe(9)
  })

  it("asks the reviewers on the pull request of its repository, not on the one of the same branch selected now", async () => {
    await refreshPullRequests("manual")
    pullRequestsStore.setState(state => ({
      repositories: { ...state.repositories, "/other\nthem": { ...state.repositories["/repo\nuser"]!, pullRequests: { auth: { ...pullRequest("tip"), number: 7 } } } },
    }))
    await requestReviewers("auth", ["ann"], other)
    expect(gh.writes).toEqual(["request ann on #7 in /other"])
    expect(getPullRequest("auth")?.reviewers).toEqual([])
    expect(pullRequestsStore.getState().repositories["/other\nthem"]?.pullRequests.auth?.reviewers).toEqual([{ login: "ann", state: "requested", requested: true }])
  })
})

// @spec spec/repository/review-requests.spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { ReviewRequest, ReviewRequestsResult } from "@/lib/bindings"
import { resetOsNotification, setWindowFocus } from "@/lib/os-notification"
import { MAX_BACKOFF_MS, POLL_INTERVAL_MS } from "@/lib/review-requests"
import { approveAndSafePush, closeReviewRequest, openReviewRequest, refreshReviewRequests, resetReviewRequests, reviewRequestsStore, submitReview } from "@/stores/review-requests"
import { reviewRequest as request } from "@/test-support/review-request"

const { gh } = vi.hoisted(() => ({
  gh: {
    selected: "/repo",
    result: null as ReviewRequestsResult | null,
    calls: [] as { repositoryPath: string; githubRepository: string | null }[],
    prunes: [] as { repositoryPath: string; keep: number[] }[],
    toasts: [] as { title: string; description?: string; actionProps?: { children: string; onClick: () => void } }[],
    notifications: [] as { title: string; body: string }[],
    activity: [] as string[],
    reviews: [] as unknown[],
    reviewError: null as string | null,
    head: "",
    safePushes: [] as { number: number; githubRepository: string | null }[],
    safePushResult: null as unknown,
    announced: [] as string[],
  },
}))

vi.mock("@/lib/bindings", () => ({
  commands: {
    getReviewRequests: async (params: { repositoryPath: string; githubRepository: string | null }) => {
      gh.calls.push(params)
      return { status: "ok", data: gh.result }
    },
    getPullRequestHead: async () => ({ status: "ok", data: gh.head }),
    submitPullRequestReview: async (params: unknown) => {
      gh.reviews.push(params)
      return gh.reviewError === null ? { status: "ok", data: null } : { status: "error", error: { message: gh.reviewError } }
    },
    pruneReviewRefs: async (params: { repositoryPath: string; keep: number[] }) => {
      gh.prunes.push(params)
      return { status: "ok", data: null }
    },
  },
}))

vi.mock("@/stores/repository", () => ({
  getSelectedProject: () => ({ path: gh.selected }),
  useSelectedProject: () => ({ path: gh.selected }),
}))

vi.mock("@/stores/pull-requests", () => ({ getGitHubRepository: () => ({ owner: "acme", name: "monorepo" }) }))

vi.mock("@/stores/activity", () => ({
  recordReviewRequest: (_path: string, request: ReviewRequest) => gh.activity.push(`#${request.number}`),
  recordReview: (_path: string, title: string) => gh.activity.push(title),
  recordRobot: (title: string) => gh.activity.push(`robot: ${title}`),
}))

vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))

vi.mock("@/stores/patronus", () => ({
  startPatronusFromReviewedPullRequest: async (number: number, githubRepository: string | null) => {
    gh.safePushes.push({ number, githubRepository })
    if (gh.safePushResult instanceof Error) {
      throw gh.safePushResult
    }
    return gh.safePushResult
  },
  watchRobot: (_robot: unknown, label: string) => gh.announced.push(`watch ${label}`),
}))

vi.mock("@/lib/patronus", () => ({ describeRobot: () => "auth → master", openRobotUrl: () => {} }))

vi.mock("@/components/ui/toast", () => ({
  toast: {
    add: (options: (typeof gh.toasts)[number]) => {
      gh.toasts.push(options)
      return "toast"
    },
    close: () => {},
  },
}))

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: async () => true,
  requestPermission: async () => "granted",
  sendNotification: (options: { title: string; body: string }) => gh.notifications.push(options),
}))

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: async () => {} }))

function available(...requests: ReviewRequest[]): ReviewRequestsResult {
  return { availability: { status: "available", owner: "acme", name: "monorepo" }, totalCount: requests.length, requests }
}

function entry(path = "/repo") {
  return reviewRequestsStore.getState().repositories[path]!
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(1_000_000_000)
  resetReviewRequests()
  resetOsNotification()
  gh.selected = "/repo"
  gh.result = available()
  gh.calls = []
  gh.prunes = []
  gh.toasts = []
  gh.notifications = []
  gh.activity = []
  gh.reviews = []
  gh.reviewError = null
  gh.head = ""
  gh.safePushes = []
  gh.safePushResult = null
  gh.announced = []
})

describe("answers", () => {
  it("tells nothing for the first answer, then one toast with Review for what is new, recorded in Activity", async () => {
    gh.result = available(request(1))
    await refreshReviewRequests("sync")
    expect(gh.calls).toEqual([{ repositoryPath: "/repo", githubRepository: "acme/monorepo" }])
    expect(gh.toasts).toEqual([])

    gh.result = available(request(1), request(2), request(3))
    await refreshReviewRequests("manual")
    expect(gh.toasts.map(it => [it.title, it.description, it.actionProps?.children])).toEqual([["2 reviews requested", "#2 Change 2 · #3 Change 3", "Review"]])
    expect(gh.activity).toEqual(["#2", "#3"])
    expect([...entry().unread]).toEqual(["2@1002", "3@1003"])

    gh.toasts[0]!.actionProps!.onClick()
    expect(reviewRequestsStore.getState().openReview?.number).toBe(2)
    expect([...entry().unread]).toEqual(["3@1003"])
  })

  it("tells a re-request, keeps the list of an answer that changed nothing", async () => {
    gh.result = available(request(1))
    await refreshReviewRequests("sync")
    const first = entry().requests
    await refreshReviewRequests("manual")
    expect(entry().requests).toBe(first)

    gh.result = available(request(1, { waitingSince: 5_000, reRequested: true }))
    await refreshReviewRequests("manual")
    expect(gh.toasts.map(it => it.title)).toEqual(["bob requested your review"])
  })

  it("tells in the background by an OS notification, and in a toast on focus", async () => {
    await refreshReviewRequests("sync")
    setWindowFocus(false)
    gh.result = available(request(7))
    await refreshReviewRequests("manual")
    await vi.waitFor(() => expect(gh.notifications).toEqual([{ title: "bob requested your review", body: "#7 Change 7" }]))
    expect(gh.toasts).toEqual([])
    setWindowFocus(true)
    expect(gh.toasts.map(it => it.title)).toEqual(["bob requested your review"])
  })

  it("keeps the rows when a refresh fails, drops them without a token", async () => {
    gh.result = available(request(1))
    await refreshReviewRequests("sync")
    const checkedAt = entry().checkedAt
    gh.result = { availability: { status: "error", message: "offline" }, totalCount: 0, requests: [] }
    vi.advanceTimersByTime(1_000)
    await refreshReviewRequests("manual")
    expect(entry().requests.map(it => it.number)).toEqual([1])
    expect(entry().availability).toEqual({ status: "error", message: "offline" })
    expect(entry().checkedAt).toBe(checkedAt)

    gh.result = { availability: { status: "notAuthenticated", source: "gh" }, totalCount: 0, requests: [] }
    await refreshReviewRequests("manual")
    expect(entry().requests).toEqual([])
  })

  it("is silent for the first answer after a repository switch", async () => {
    gh.result = available(request(1))
    await refreshReviewRequests("sync")
    gh.selected = "/other"
    gh.result = available(request(5))
    await refreshReviewRequests("sync")
    gh.selected = "/repo"
    gh.result = available(request(1), request(2))
    await refreshReviewRequests("sync")
    expect(gh.toasts).toEqual([])
  })
})

describe("polling", () => {
  it("polls every five minutes, focused or not, and backs off while GitHub fails", async () => {
    await refreshReviewRequests("sync")
    setWindowFocus(false)
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(gh.calls).toHaveLength(2)

    gh.result = { availability: { status: "error", message: "offline" }, totalCount: 0, requests: [] }
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(gh.calls).toHaveLength(3)
    // the first failure doubles the interval
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(gh.calls).toHaveLength(3)
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(gh.calls).toHaveLength(4)
    await vi.advanceTimersByTimeAsync(MAX_BACKOFF_MS)
    expect(gh.calls).toHaveLength(5)
  })

  it("stops while GitHub rejects the token, until a focus asks again", async () => {
    gh.result = { availability: { status: "notAuthenticated", source: "gh" }, totalCount: 0, requests: [] }
    await refreshReviewRequests("sync")
    await vi.advanceTimersByTimeAsync(3 * POLL_INTERVAL_MS)
    expect(gh.calls).toHaveLength(1)
    gh.result = available(request(1))
    await refreshReviewRequests("focus")
    expect(gh.calls).toHaveLength(2)
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(gh.calls).toHaveLength(3)
  })

  it("skips a poll while offline", async () => {
    await refreshReviewRequests("sync")
    vi.stubGlobal("navigator", { onLine: false })
    try {
      await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
      expect(gh.calls).toHaveLength(1)
    } finally {
      vi.unstubAllGlobals()
    }
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS)
    expect(gh.calls).toHaveLength(2)
  })
})

describe("review refs", () => {
  it("prunes once after an answer with the requests, never after a failure, and keeps the open dialog's", async () => {
    gh.result = { availability: { status: "error", message: "offline" }, totalCount: 0, requests: [] }
    await refreshReviewRequests("sync")
    expect(gh.prunes).toEqual([])

    gh.result = available(request(4))
    await refreshReviewRequests("manual")
    await refreshReviewRequests("manual")
    expect(gh.prunes).toEqual([{ repositoryPath: "/repo", keep: [] }])

    openReviewRequest("/repo", 4)
    closeReviewRequest()
    // nothing was written since the last prune
    expect(gh.prunes).toHaveLength(1)
  })
})

describe("reviews", () => {
  const HEAD = "0123456789abcdef0123456789abcdef01234567"

  it("approves the commit the user saw, says so with Open Pull Request, and asks GitHub again", async () => {
    gh.result = available(request(7), request(8))
    await refreshReviewRequests("sync")
    openReviewRequest("/repo", 7)

    gh.result = available(request(8))
    expect(await submitReview("/repo", request(7), "approve", HEAD)).toBe(true)
    expect(gh.reviews).toEqual([{ repositoryPath: "/repo", githubRepository: "acme/monorepo", pullRequestNumber: 7, event: "approve", commitId: HEAD, body: null }])
    expect(gh.toasts.map(it => [it.title, it.description, it.actionProps?.children])).toEqual([["Approved #7", "Change 7", "Open Pull Request"]])
    expect(gh.activity).toEqual(["Approved #7"])
    expect(entry().reviewed[7]).toEqual({ event: "approve", commitId: HEAD })
    expect(entry().busy).toEqual({})
    await vi.waitFor(() => expect(entry().requests.map(it => it.number)).toEqual([8]))
    // the dialog keeps the approved request, at its place in the list
    expect(reviewRequestsStore.getState().openReview).toMatchObject({ number: 7, index: 0 })
  })

  it("requests changes with the comment, and says why GitHub refused", async () => {
    gh.result = available(request(7))
    await refreshReviewRequests("sync")
    expect(await submitReview("/repo", request(7), "requestChanges", HEAD, "Sort by the snapshot")).toBe(true)
    expect(gh.reviews).toMatchObject([{ event: "requestChanges", body: "Sort by the snapshot" }])
    expect(gh.toasts.at(-1)?.title).toBe("Requested changes on #7")

    gh.reviewError = "User can only have one pending review per pull request"
    expect(await submitReview("/repo", request(7), "approve", HEAD)).toBe(false)
    expect(gh.toasts.at(-1)).toMatchObject({ title: "Cannot approve pull request #7", description: "User can only have one pending review per pull request" })
    expect(entry().reviewed[7]).toEqual({ event: "requestChanges", commitId: HEAD })
  })
})

describe("Safe Push of a reviewed pull request", () => {
  const HEAD = "0123456789abcdef0123456789abcdef01234567"

  it("approves, checks the head GitHub shows and asks Patronus; its thumbs up is the start", async () => {
    gh.result = available(request(7))
    await refreshReviewRequests("sync")
    gh.head = HEAD
    expect(await approveAndSafePush("/repo", request(7), HEAD)).toBe(true)
    expect(gh.reviews).toMatchObject([{ event: "approve", commitId: HEAD }])
    expect(gh.safePushes).toEqual([{ number: 7, githubRepository: "acme/monorepo" }])
    // no robot of the user's: the toast opens the pull request
    expect(gh.toasts.map(it => [it.title, it.description, it.actionProps?.children])).toEqual([
      ["Approved #7", "Change 7", "Open Pull Request"],
      ["Safe Push started", "#7 Change 7", "Open Pull Request"],
    ])
    expect(gh.activity).toEqual(["Approved #7", "Safe Push started on #7"])

    // approved already: no second approval; a robot of the user's is announced as any
    gh.safePushResult = { id: "robot", url: "https://patronus/robot" }
    expect(await approveAndSafePush("/repo", request(7), HEAD)).toBe(true)
    expect(gh.reviews).toHaveLength(1)
    // the robot of no branch is watched: polled, its merge told, listed in the header
    expect(gh.announced).toEqual(["watch #7 Change 7"])
    expect(gh.toasts.at(-1)).toMatchObject({ title: "Safe Push started", description: "auth → master", actionProps: { children: "Open in Patronus" } })
    expect(gh.activity.at(-1)).toBe("robot: Safe Push started")
    expect(entry().busy).toEqual({})
  })

  it("refuses a pull request that got commits since the review, and says the reason of the bot", async () => {
    gh.result = available(request(7))
    await refreshReviewRequests("sync")
    gh.head = "f".repeat(40)
    expect(await approveAndSafePush("/repo", request(7), HEAD)).toBe(false)
    expect(gh.safePushes).toEqual([])
    expect(gh.toasts.at(-1)).toMatchObject({ title: "Cannot start the Safe Push", description: "#7 has new commits since you reviewed 01234567: review ffffffff first" })

    gh.head = HEAD
    gh.safePushResult = new Error("A robot is already running for the branch.")
    expect(await approveAndSafePush("/repo", request(7), HEAD)).toBe(false)
    expect(gh.toasts.at(-1)).toMatchObject({ title: "Cannot start the Safe Push", description: "A robot is already running for the branch." })
  })

  it("tells the window that asked, records Activity in the main window, and asks Patronus for the selected repository only", async () => {
    gh.result = available(request(7))
    await refreshReviewRequests("sync")
    gh.head = HEAD
    const told: unknown[] = []
    const feedback = { toast: (toast: unknown) => told.push(toast), error: (title: string, error: unknown) => told.push({ title, error }) }
    expect(await approveAndSafePush("/repo", request(7), HEAD, feedback)).toBe(true)
    // no toast in the main window: the window says it
    expect(gh.toasts).toEqual([])
    expect(told).toEqual([
      { title: "Approved #7", description: "Change 7", target: { kind: "pullRequest", url: request(7).url } },
      { title: "Safe Push started", description: "#7 Change 7", target: { kind: "pullRequest", url: request(7).url } },
    ])
    expect(gh.activity).toEqual(["Approved #7", "Safe Push started on #7"])

    // the main window shows another repository: Patronus would be asked for it
    gh.selected = "/other"
    told.length = 0
    expect(await approveAndSafePush("/repo", request(7), HEAD, feedback)).toBe(false)
    expect(told).toEqual([{ title: "Cannot start the Safe Push", error: "Select repo in the main window: Patronus is asked for the repository it shows" }])
    expect(gh.safePushes).toHaveLength(1)
    // a review goes to the repository of the request all the same
    expect(await submitReview("/repo", request(7), "requestChanges", HEAD, "Sort", feedback)).toBe(true)
    expect(gh.reviews.at(-1)).toMatchObject({ repositoryPath: "/repo", githubRepository: "acme/monorepo", event: "requestChanges" })
  })
})

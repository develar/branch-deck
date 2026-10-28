// @spec spec/repository/review-requests.spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { Intent, News, OperationEvent, PullRequestAvailability, ReviewRequest, RunningOperation } from "@/lib/bindings"
import { resetEventToasts, toastKeys } from "@/lib/event-toast"
import { resetOsNotification, setWindowFocus } from "@/lib/os-notification"
import {
  applyReviewRequestsFacts,
  approveAndSafePush,
  connectReviewWindow,
  getReviewBusy,
  openRequestOnGitHub,
  openReviewRequest,
  refreshReviewRequests,
  resetReviewRequests,
  reviewRequestsStore,
  submitReview,
} from "@/stores/review-requests"
import { emptyView } from "@/test-support/fake-sessions"
import { reviewRequest as request } from "@/test-support/review-request"
import { describeToast, openToasts, resetToasts, toasts } from "@/test-support/toast-mock"

/** What GitHub answers of the review requests, as the session reads it */
interface Listing {
  readonly availability: PullRequestAvailability
  readonly totalCount: number
  readonly requests: readonly ReviewRequest[]
}

const { gh } = vi.hoisted(() => ({
  gh: {
    selected: "/repo",
    result: null as Listing | null,
    /** The refreshes the page asked the session for: the family and the reason of each */
    refreshes: [] as string[],
    notifications: [] as { title: string; body: string }[],
    activity: [] as string[],
    reviews: [] as unknown[],
    reviewError: null as string | null,
    head: "",
    safePushes: [] as { number: number }[],
    safePushResult: null as unknown,
    announced: [] as string[],
    /** The actions the session decided, by subject (`pull:<number>`) */
    actions: {} as Record<string, unknown[]>,
  },
}))

vi.mock("@/lib/bindings", async importOriginal => ({
  // the constants of the bindings (the defaults of the settings Rust reads); the commands are the test's
  ...(await importOriginal<typeof import("@/lib/bindings")>()),
  commands: {},
}))

/**
 * The engine as the page sees it run a `SubmitReview` (`operations::review`): the review (GitHub refusing it with
 * `reviewError`), then for a Safe Push the step `starting`, the head GitHub shows (another than the reviewed commit
 * skips the start with the reason Rust words), and the comment the bot accepts (`safePushResult`: the robot the user's
 * robots list, or an Error the bot's reason)
 */
vi.mock("@/lib/run-intent", async importOriginal => ({
  ...(await importOriginal<typeof import("@/lib/run-intent")>()),
  runIntent: async (repositoryPath: string, intent: Intent, onEvent: (event: OperationEvent) => void = () => {}) => {
    if (intent.kind !== "submitReview") {
      throw new Error(`no ${intent.kind} here`)
    }
    gh.reviews.push({ repositoryPath, ...intent })
    const effects: unknown[] = []
    const failed = (message: string) => ({ kind: "failed", error: { code: "networkError", message } })
    if (intent.verdict !== null) {
      effects.push({
        effect: { kind: "submitReview", number: intent.number, verdict: intent.verdict, commit: intent.commit },
        status: gh.reviewError === null ? { kind: "done" } : failed(gh.reviewError),
      })
    }
    let result: unknown = { kind: "none" }
    if (intent.thenLand && gh.reviewError === null) {
      onEvent({ kind: "step", op: "op", step: { kind: "starting" } })
      const effect = { kind: "startRobot", route: { kind: "comment", number: intent.number } }
      if (gh.head !== intent.commit) {
        const reason = { code: "plan_changed", message: `#${intent.number} has new commits since you reviewed ${intent.commit.slice(0, 8)}: review ${gh.head.slice(0, 8)} first` }
        effects.push({ effect, status: { kind: "skipped", reason } })
      } else if (gh.safePushResult instanceof Error) {
        effects.push({ effect, status: failed(gh.safePushResult.message) })
      } else {
        gh.safePushes.push({ number: intent.number })
        effects.push({ effect, status: { kind: "done" } })
        result = { kind: "review", comment: { number: intent.number, url: "u", accepted: true }, robot: gh.safePushResult }
      }
    }
    return { kind: "ran", plan: {}, outcome: { op: "op", effects, newsKey: null, versionAfter: null, result, stats: {} } }
  },
}))

vi.mock("@/stores/branch-sync", () => ({ branchSyncStore: { getState: () => ({ operations: {} }), subscribe: () => () => {} } }))

const { shown } = vi.hoisted(() => ({
  /** The repositories the main window was asked to show, and why it can't (null: it shows it) */
  shown: { paths: [] as string[], reason: null as string | null },
}))

vi.mock("@/lib/show-repository", () => ({
  showRepository: async (repositoryPath: string) => {
    shown.paths.push(repositoryPath)
    return shown.reason
  },
}))

vi.mock("@/stores/repository", async () => ({
  repositoryStore: (await import("@/test-support/quiet-store")).quietStore,
  getSelectedProject: () => ({ path: gh.selected }),
  useSelectedProject: () => ({ path: gh.selected }),
}))

vi.mock("@/stores/activity", () => ({
  recordReviewRequest: (_path: string, request: ReviewRequest) => gh.activity.push(`#${request.number}`),
}))

vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))

vi.mock("@/stores/patronus", () => ({ watchRobot: (_robot: unknown, label: string) => gh.announced.push(`watch ${label}`) }))

vi.mock("@/lib/patronus", () => ({ describeRobot: () => "auth → master", openRobotUrl: () => {} }))

vi.mock("@/components/ui/toast", async () => (await import("@/test-support/toast-mock")).toastModule)

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: async () => true,
  requestPermission: async () => "granted",
  sendNotification: (options: { title: string; body: string }) => gh.notifications.push(options),
}))

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: async () => {} }))

// the session lists the review requests again for the refresh button: what GitHub answers now
vi.mock("@/model/store", () => ({
  refreshSessionFacts: async (repositoryPath: string, family: string, reason: string) => {
    gh.refreshes.push(`${family} ${reason}`)
    if (reason === "manual") {
      listed(gh.result!, [], repositoryPath)
    }
    return true
  },
  getRowPart: (subject: string, part: string) => (part === "actions" ? gh.actions[subject] : undefined),
}))

function available(...requests: ReviewRequest[]): Listing {
  return { availability: { status: "available", owner: "acme", name: "monorepo" }, totalCount: requests.length, requests }
}

let lastNews = 0

/**
 * The session listed the review requests of `repositoryPath` (`listing`, as its view has them: the rows of the last
 * answer with requests) and tells the requests of `told` as new
 */
function listed(listing: Listing, told: number[] = [], repositoryPath = "/repo"): void {
  const view = emptyView()
  const available = listing.availability.status === "available"
  const reviewRequests = {
    head: { value: listing.availability, state: { kind: "fresh" as const }, seq: lastNews + 1, observedAt: 1_000_000 },
    requests: [...(available ? listing.requests : (reviewRequestsStore.getState().repositories[repositoryPath]?.requests ?? []))],
    totalCount: listing.totalCount,
    checkedAt: available ? 1_000_000 : 0,
  }
  const news: News[] = told.length === 0 ? [] : [{ id: ++lastNews, event: { kind: "reviewRequested", numbers: told } }]
  applyReviewRequestsFacts(repositoryPath, { ...view.facts, reviewRequests }, { reviewRequests }, news)
}

function entry(path = "/repo") {
  return reviewRequestsStore.getState().repositories[path]!
}

/** The review window as the store sees it: the opens it got, the request it shows */
const reviewWindow = { opens: [] as { number: number; index: number; intent: string | null }[], shown: null as number | null }
connectReviewWindow({
  open: (_repositoryPath, request, index, intent) => {
    reviewWindow.opens.push({ number: request.number, index, intent })
    reviewWindow.shown = request.number
  },
  rowsOf: () => [],
})

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(1_000_000_000)
  resetReviewRequests()
  resetOsNotification()
  reviewWindow.opens = []
  reviewWindow.shown = null
  gh.selected = "/repo"
  gh.result = available()
  gh.refreshes = []
  lastNews = 0
  resetEventToasts()
  resetToasts()
  gh.notifications = []
  gh.activity = []
  gh.reviews = []
  gh.reviewError = null
  gh.head = ""
  gh.safePushes = []
  gh.safePushResult = null
  gh.announced = []
  gh.actions = {}
})

describe("answers", () => {
  it("tells the requests the session tells new in one toast with Review, unread until opened, recorded in Activity", async () => {
    listed(available(request(1)))
    expect(toasts).toEqual([])
    listed(available(request(1), request(2), request(3)), [2, 3])
    expect(toasts.map(it => [it.title, it.description, it.actionProps?.children])).toEqual([["2 reviews requested", "#2 Change 2 · #3 Change 3", "Review"]])
    expect(gh.activity).toEqual(["#2", "#3"])
    expect([...entry().unread]).toEqual(["2@1002", "3@1003"])

    // Review of the toast: the review window of the first one
    toasts[0]!.actionProps!.onClick()
    expect(reviewWindow.opens).toEqual([{ number: 2, index: 1, intent: null }])
    expect([...entry().unread]).toEqual(["3@1003"])
    // a request that left the list is unread no more
    listed(available(request(1)))
    expect([...entry().unread]).toEqual([])
  })

  it("replaces the toast of a request with the review of it, and closes a toast of several once none of them waits", async () => {
    const HEAD = "0123456789abcdef0123456789abcdef01234567"
    listed(available(request(1)))
    listed(available(request(1), request(7)), [7])
    expect(openToasts().map(describeToast)).toEqual([["bob requested your review", "#7 Change 7", "Review", undefined]])
    const asked = openToasts()[0]!
    gh.result = available(request(1))
    expect(await submitReview("/repo", request(7), "approve", HEAD)).toBe(true)
    // the same toast: what the user did with the request, no Review of a reviewed one
    expect(openToasts()).toEqual([asked])
    expect(openToasts().map(describeToast)).toEqual([["Approved #7", "Change 7", "Open Pull Request", undefined]])

    resetToasts()
    listed(available(request(1), request(2), request(3)), [2, 3])
    expect(openToasts().map(describeToast)).toEqual([["2 reviews requested", "#2 Change 2 · #3 Change 3", "Review", undefined]])
    // #2 was reviewed elsewhere: Review opens the one that still waits
    listed(available(request(1), request(3)))
    openToasts()[0]!.actionProps!.onClick!()
    expect(reviewWindow.opens).toEqual([{ number: 3, index: 1, intent: null }])

    listed(available(request(1), request(2), request(3)), [2, 3])
    expect(openToasts()).toHaveLength(1)
    // none of them waits: nothing is left to review
    listed(available(request(1)))
    expect(openToasts()).toEqual([])
  })

  it("keeps the list and the rows of an answer that changed nothing", async () => {
    listed(available(request(1), request(2)))
    const first = entry().requests
    listed(available(request(1), request(2)))
    expect(entry().requests).toBe(first)
    listed(available(request(1), request(2, { title: "Renamed" })))
    expect(entry().requests[0]).toBe(first[0])
    expect(entry().requests[1]!.title).toBe("Renamed")
  })

  it("tells in the background by an OS notification, and in a toast on focus", async () => {
    listed(available())
    setWindowFocus(false)
    listed(available(request(7)), [7])
    await vi.waitFor(() => expect(gh.notifications).toEqual([{ title: "bob requested your review", body: "#7 Change 7" }]))
    expect(toasts).toEqual([])
    setWindowFocus(true)
    expect(toasts.map(it => it.title)).toEqual(["bob requested your review"])
  })
})

describe("refresh", () => {
  it("asks the session on the refresh button, refreshing until its answer is in", async () => {
    listed(available(request(1)))
    gh.result = available(request(1), request(2))
    const refresh = refreshReviewRequests()
    expect(entry().isRefreshing).toBe(true)
    await refresh
    expect((({ isRefreshing, requests }) => [isRefreshing, requests.map(it => it.number)])(entry())).toEqual([false, [1, 2]])
    expect(gh.refreshes).toEqual(["reviewRequests manual"])
  })

  it("tells the session a request was opened on GitHub: the next focus asks at once", () => {
    listed(available(request(1)), [1])
    openRequestOnGitHub(request(1))
    expect(gh.refreshes).toEqual(["reviewRequests afterWrite"])
    expect([...entry().unread]).toEqual([])
  })
})

describe("reviews", () => {
  const HEAD = "0123456789abcdef0123456789abcdef01234567"

  it("approves the commit the user saw, says so with Open Pull Request, and asks GitHub again", async () => {
    listed(available(request(7), request(8)))
    openReviewRequest("/repo", 7)

    expect(await submitReview("/repo", request(7), "approve", HEAD)).toBe(true)
    expect(gh.reviews).toEqual([{ repositoryPath: "/repo", kind: "submitReview", number: 7, verdict: "approve", commit: HEAD, body: null, thenLand: false }])
    expect(toasts.map(it => [it.title, it.description, it.actionProps?.children])).toEqual([["Approved #7", "Change 7", "Open Pull Request"]])
    expect(entry().reviewed[7]).toEqual({ event: "approve", commitId: HEAD })
    // the session asks GitHub again (`operations::review`): the request leaves the list, the window keeps its place
    listed(available(request(8)))
    expect(entry().requests.map(it => it.number)).toEqual([8])
    expect(reviewWindow.opens).toEqual([{ number: 7, index: 0, intent: null }])
  })

  it("requests changes with the comment, and says why GitHub refused", async () => {
    listed(available(request(7)))
    expect(await submitReview("/repo", request(7), "requestChanges", HEAD, "Sort by the snapshot")).toBe(true)
    expect(gh.reviews).toMatchObject([{ verdict: "requestChanges", body: "Sort by the snapshot" }])
    expect(toasts.at(-1)?.title).toBe("Requested changes on #7")

    gh.reviewError = "User can only have one pending review per pull request"
    expect(await submitReview("/repo", request(7), "approve", HEAD)).toBe(false)
    expect(toasts.at(-1)).toMatchObject({ title: "Cannot approve pull request #7", description: "User can only have one pending review per pull request" })
    expect(entry().reviewed[7]).toEqual({ event: "requestChanges", commitId: HEAD })
  })

  it("says what runs for a pull request from the reviews of the session: the review, then its Safe Push", () => {
    const running = (thenLand: boolean, verdict: "approve" | "requestChanges" | null, step: "writing" | "starting" | "checking"): RunningOperation => ({
      id: `${verdict}-${step}`,
      intent: { kind: "submitReview", number: 7, verdict, commit: HEAD, body: null, thenLand },
      resources: [{ kind: "pullRequest", number: 7, field: "review" }],
      step: { kind: step },
      origin: { kind: "thisProcess" },
      moves: [],
    })
    expect(getReviewBusy({ a: running(false, "requestChanges", "writing") })).toEqual({ 7: "requestChanges" })
    expect(getReviewBusy({ a: running(true, "approve", "writing") })).toEqual({ 7: "approve" })
    expect(getReviewBusy({ a: running(true, "approve", "starting") })).toEqual({ 7: "safePush" })
    expect(getReviewBusy({ a: running(true, null, "checking") })).toEqual({ 7: "safePush" })
    expect(getReviewBusy({})).toEqual({})
  })
})

describe("Safe Push of a reviewed pull request", () => {
  const HEAD = "0123456789abcdef0123456789abcdef01234567"

  it("approves, checks the head GitHub shows and asks Patronus; its thumbs up is the start, in the toast of the approval", async () => {
    listed(available(request(7)))
    gh.head = HEAD
    expect(await approveAndSafePush("/repo", request(7), HEAD)).toBe(true)
    expect(gh.reviews).toMatchObject([{ verdict: "approve", commit: HEAD, thenLand: true }])
    expect(gh.safePushes).toEqual([{ number: 7 }])
    // no robot of the user's: the toast opens the pull request; "Approved #7" became the start in place
    expect(toasts.map(it => [it.title, it.description, it.actionProps?.children])).toEqual([["Safe Push started", "#7 Change 7", "Open Pull Request"]])

    // approved already (the session says so): the Safe Push alone; a robot of the user's is announced as any
    gh.actions["pull:7"] = [{ id: "approve", availability: { kind: "disabled", reason: { code: "approved", commit: HEAD } } }]
    gh.safePushResult = { id: "robot", url: "https://patronus/robot" }
    expect(await approveAndSafePush("/repo", request(7), HEAD)).toBe(true)
    expect(gh.reviews.at(-1)).toMatchObject({ verdict: null, thenLand: true })
    // the robot of no branch is watched: polled, its merge told, listed in the header
    expect(gh.announced).toEqual(["watch #7 Change 7"])
    expect(toasts.at(-1)).toMatchObject({ title: "Safe Push started", description: "auth → master", actionProps: { children: "Open in Patronus" } })
  })

  it("refuses a pull request that got commits since the review, and says the reason of the bot", async () => {
    listed(available(request(7)))
    gh.head = "f".repeat(40)
    expect(await approveAndSafePush("/repo", request(7), HEAD)).toBe(false)
    expect(gh.safePushes).toEqual([])
    expect(toasts.at(-1)).toMatchObject({ title: "Cannot start the Safe Push", description: "#7 has new commits since you reviewed 01234567: review ffffffff first" })
    // the approval stays
    expect(entry().reviewed[7]).toEqual({ event: "approve", commitId: HEAD })

    gh.head = HEAD
    gh.safePushResult = new Error("Patronus rejected the comment on pull request #7: A robot is already running for the branch.")
    expect(await approveAndSafePush("/repo", request(7), HEAD)).toBe(false)
    expect(toasts.at(-1)).toMatchObject({
      title: "Cannot start the Safe Push",
      description: "Patronus rejected the comment on pull request #7: A robot is already running for the branch.",
    })
  })

  it("tells the window that asked and switches the main window to the request's repository first", async () => {
    listed(available(request(7)))
    gh.head = HEAD
    const told: unknown[] = []
    const feedback = { toast: (toast: unknown) => told.push(toast), error: (title: string, error: unknown) => told.push({ title, error }) }
    expect(await approveAndSafePush("/repo", request(7), HEAD, feedback)).toBe(true)
    // no toast in the main window: the window says it
    expect(toasts).toEqual([])
    // one toast of the pull request in the window: the start takes the place of the approval
    const key = toastKeys.review("/repo", 7)
    expect(told).toEqual([
      { key, title: "Approved #7", description: "Change 7", target: { kind: "pullRequest", url: request(7).url } },
      { key, title: "Safe Push started", description: "#7 Change 7", target: { kind: "pullRequest", url: request(7).url } },
    ])

    // the main window shows another repository: it switches to the request's one first, or says why it can't
    gh.selected = "/other"
    told.length = 0
    shown.reason = "repo is no longer among the repositories of the main window"
    expect(await approveAndSafePush("/repo", request(7), HEAD, feedback)).toBe(false)
    expect(shown.paths).toEqual(["/repo"])
    expect(told).toEqual([{ title: "Cannot start the Safe Push", error: "repo is no longer among the repositories of the main window" }])
    expect(gh.safePushes).toHaveLength(1)
    // a review goes to the repository of the request all the same
    expect(await submitReview("/repo", request(7), "requestChanges", HEAD, "Sort", feedback)).toBe(true)
    expect(gh.reviews.at(-1)).toMatchObject({ repositoryPath: "/repo", verdict: "requestChanges" })
  })

  it("hands the error of a failed approval to its feedback as the engine answered it, and starts nothing", async () => {
    listed(available(request(7)))
    gh.reviewError = "GitHub answered 403"
    const told: unknown[] = []
    const feedback = { toast: (toast: unknown) => told.push(toast), error: (title: string, error: unknown) => told.push({ title, error }) }
    expect(await approveAndSafePush("/repo", request(7), HEAD, feedback)).toBe(false)
    expect(told).toEqual([{ title: "Cannot approve pull request #7", error: { code: "networkError", message: "GitHub answered 403" } }])
    expect(gh.safePushes).toEqual([])
  })
})

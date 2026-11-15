// @spec openspec/specs/repository/review-requests/spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { ApiError, ReviewRequest } from "@/lib/bindings"
import type { CommandResult } from "@/lib/command"

const { engine } = vi.hoisted(() => ({
  engine: {
    /** The fetches that reached the engine */
    asked: [] as string[],
    /** How each next fetch ends, in order */
    answers: [] as CommandResult<unknown>[],
  },
}))

vi.mock("@tauri-apps/api/core", () => import("@/test-support/channel-mock"))
vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))
vi.mock("@/model/intents", async importOriginal => ({
  ...(await importOriginal<typeof import("@/model/intents")>()),
  executeIntent: async (params: { intent: { kind: string; number?: number; top?: number } }) => {
    engine.asked.push(`${params.intent.kind} ${params.intent.number ?? params.intent.top ?? ""}`)
    return engine.answers.shift()
  },
  cancelIntent: async () => {},
}))

const { reviewFetchKey, reviewFetchStore, retryReviewFetch, retryReviewStackFetch, reviewStackFetchKey, startReviewFetch, startReviewStackFetch, stopReviewFetch } =
  await import("@/stores/review-fetch")

const REQUEST = { number: 101, headRefOid: "head", baseRefOid: "base" } as ReviewRequest
const CREDENTIALS: ApiError = {
  code: "internal",
  message: "GitHub asked for credentials: store HTTPS credentials of github.com in git (gh auth setup-git does)",
  details: null,
} as ApiError
const FETCHED = { status: "ok", data: { effects: [], result: { kind: "reviewRange", base: "base", head: "head", commits: [] } } } as unknown as CommandResult<unknown>
const RANGE = { base: "base", head: "head", commits: [] }
const FETCHED_STACK = { status: "ok", data: { effects: [], result: { kind: "reviewStack", range: RANGE, layers: [] } } } as unknown as CommandResult<unknown>
/** The layers of the stack of #102 on #101 as the review requests have them */
const LAYERS = [REQUEST, { ...REQUEST, number: 102, baseRefOid: "head", headRefOid: "top" }]

function statusOf(key: string) {
  return reviewFetchStore.getState().fetches[key]?.status
}

function shown() {
  return reviewFetchStore.getState().fetches[reviewFetchKey("/repo", REQUEST)]?.status
}

async function settled(): Promise<void> {
  await vi.waitFor(() => expect(shown()).not.toBe("fetching"))
}

describe("the fetch of the pull request a review window shows", () => {
  beforeEach(() => {
    stopReviewFetch()
    engine.asked = []
    engine.answers = []
  })

  it("keeps a failure on screen when the window asks for the same request again, and fetches it again only on Retry", async () => {
    engine.answers.push({ status: "error", error: CREDENTIALS })
    startReviewFetch("/repo", REQUEST)
    await settled()
    expect(shown()).toBe("failed")

    // the window asks again as the session gives it the same request anew (a patch of the review requests): the
    // failure, with its Retry, stays, and nothing is fetched
    startReviewFetch("/repo", { ...REQUEST })
    expect(shown()).toBe("failed")
    expect(engine.asked).toEqual(["fetchReview 101"])

    engine.answers.push(FETCHED)
    retryReviewFetch("/repo", REQUEST)
    expect(shown()).toBe("fetching")
    await settled()
    expect(shown()).toBe("fetched")
    expect(engine.asked).toEqual(["fetchReview 101", "fetchReview 101"])
  })

  it("fetches a pushed request at once, also after a failure of the one before, whose entry goes", async () => {
    engine.answers.push({ status: "error", error: CREDENTIALS }, FETCHED)
    startReviewFetch("/repo", REQUEST)
    await settled()
    const pushed = { ...REQUEST, headRefOid: "pushed" }
    startReviewFetch("/repo", pushed)
    await vi.waitFor(() => expect(statusOf(reviewFetchKey("/repo", pushed))).toBe("fetched"))
    expect(engine.asked).toEqual(["fetchReview 101", "fetchReview 101"])
    expect(shown(), "the failure of the request before the push").toBeUndefined()
  })
})

describe("the fetch of the stack a review window shows", () => {
  beforeEach(() => {
    stopReviewFetch()
    engine.asked = []
    engine.answers = []
  })

  it("keeps a failure on screen when the window asks for the same stack again, and fetches it again only on Retry", async () => {
    const key = reviewStackFetchKey("/repo", 102, 101, LAYERS)
    engine.answers.push({ status: "error", error: CREDENTIALS })
    startReviewStackFetch("/repo", key, 102, 101)
    await vi.waitFor(() => expect(statusOf(key)).toBe("failed"))

    // the window asks again as the session gives it the same layers anew: the failure, with its Retry, stays
    startReviewStackFetch(
      "/repo",
      reviewStackFetchKey(
        "/repo",
        102,
        101,
        LAYERS.map(layer => ({ ...layer })),
      ),
      102,
      101,
    )
    expect(statusOf(key)).toBe("failed")
    expect(engine.asked).toEqual(["fetchReviewStack 102"])

    engine.answers.push(FETCHED_STACK)
    retryReviewStackFetch("/repo", key, 102, 101)
    expect(statusOf(key)).toBe("fetching")
    await vi.waitFor(() => expect(statusOf(key)).toBe("fetched"))
    expect(engine.asked).toEqual(["fetchReviewStack 102", "fetchReviewStack 102"])
  })

  it("fetches the stack at once when a layer was pushed or rebased, also after a failure, whose entry goes", async () => {
    const key = reviewStackFetchKey("/repo", 102, 101, LAYERS)
    engine.answers.push({ status: "error", error: CREDENTIALS }, FETCHED_STACK, FETCHED_STACK)
    startReviewStackFetch("/repo", key, 102, 101)
    await vi.waitFor(() => expect(statusOf(key)).toBe("failed"))
    // a push to the top layer and a rebase of the bottom one are other stacks, as the window shows them
    const pushed = reviewStackFetchKey("/repo", 102, 101, [LAYERS[0]!, { ...LAYERS[1]!, headRefOid: "pushed" }])
    startReviewStackFetch("/repo", pushed, 102, 101)
    await vi.waitFor(() => expect(statusOf(pushed)).toBe("fetched"))
    expect(statusOf(key), "the failure of the stack before the push").toBeUndefined()
    const rebased = reviewStackFetchKey("/repo", 102, 101, [
      { ...LAYERS[0]!, baseRefOid: "rebased" },
      { ...LAYERS[1]!, headRefOid: "pushed" },
    ])
    startReviewStackFetch("/repo", rebased, 102, 101)
    await vi.waitFor(() => expect(statusOf(rebased)).toBe("fetched"))
    expect(engine.asked).toEqual(["fetchReviewStack 102", "fetchReviewStack 102", "fetchReviewStack 102"])
  })
})

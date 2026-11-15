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
  executeIntent: async (params: { intent: { kind: string; number?: number } }) => {
    engine.asked.push(`${params.intent.kind} ${params.intent.number ?? ""}`)
    return engine.answers.shift()
  },
  cancelIntent: async () => {},
}))

const { reviewFetchKey, reviewFetchStore, retryReviewFetch, startReviewFetch, stopReviewFetch } = await import("@/stores/review-fetch")

const REQUEST = { number: 101, headRefOid: "head", baseRefOid: "base" } as ReviewRequest
const CREDENTIALS: ApiError = {
  code: "internal",
  message: "GitHub asked for credentials: store HTTPS credentials of github.com in git (gh auth setup-git does)",
  details: null,
} as ApiError
const FETCHED = { status: "ok", data: { effects: [], result: { kind: "reviewRange", base: "base", head: "head", commits: [] } } } as unknown as CommandResult<unknown>

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

  it("fetches a pushed request at once, also after a failure of the one before", async () => {
    engine.answers.push({ status: "error", error: CREDENTIALS }, FETCHED)
    startReviewFetch("/repo", REQUEST)
    await settled()
    const pushed = { ...REQUEST, headRefOid: "pushed" }
    startReviewFetch("/repo", pushed)
    await vi.waitFor(() => expect(reviewFetchStore.getState().fetches[reviewFetchKey("/repo", pushed)]?.status).toBe("fetched"))
    expect(engine.asked).toEqual(["fetchReview 101", "fetchReview 101"])
  })
})

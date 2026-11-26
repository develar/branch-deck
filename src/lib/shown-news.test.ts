import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { Intent, News } from "@/lib/bindings"
import { tellShownNews } from "@/lib/shown-news"

const { runIntent, notifyError } = vi.hoisted(() => ({
  // the close fails to delete the branches of the review; the open runs through
  runIntent: vi.fn(async (_repositoryPath: string, intent: Intent) => ({
    kind: "ran",
    plan: {},
    outcome: {
      result: { kind: "none" },
      effects:
        intent.kind === "closeShownReview"
          ? [{ effect: { kind: "deleteReview", branch: "login" }, status: { kind: "failed", error: { code: "pushRefused", message: "refused" } } }]
          : intent.kind === "extendShownReview" || intent.kind === "deleteClosedShownReview"
            ? [
                {
                  effect: { kind: "extendReview", branch: "api", number: 14, fixes: [] },
                  status: { kind: "failed", error: { code: "pushRefused", message: "stale info" } },
                },
              ]
            : [],
    },
  })),
  notifyError: vi.fn(),
}))

vi.mock("@/model/intents", async importOriginal => ({ ...(await importOriginal<typeof import("@/model/intents")>()), runIntent }))
vi.mock("@/lib/errors", async importOriginal => ({ ...(await importOriginal<typeof import("@/lib/errors")>()), notifyError }))

describe("tellShownNews", () => {
  beforeEach(() => {
    runIntent.mockClear()
    notifyError.mockClear()
  })

  it("opens the review after landing whose Safe Push merged, closes the approved one, and toasts what failed", async () => {
    const news: News[] = [
      { id: 1, event: { kind: "shownReviewLanded", branch: "auth", number: 12 } },
      { id: 2, event: { kind: "shownReviewApproved", branch: "login", number: 13 } },
      { id: 3, event: { kind: "robotMerged", robotId: "r1", closes: [], landed: [] } },
    ]
    tellShownNews("/repo", news)
    await vi.waitFor(() => expect(notifyError.mock.calls.map(([title]) => title)).toEqual(["Cannot close the review after landing of login"]))
    expect(runIntent.mock.calls).toEqual([
      ["/repo", { kind: "openShownReview", branch: "auth" }],
      ["/repo", { kind: "closeShownReview", branch: "login", allowUnapproved: false }],
    ])
  })

  it("deletes the branches of a review after landing closed on GitHub and toasts a failure of it", async () => {
    tellShownNews("/repo", [{ id: 5, event: { kind: "shownReviewClosedElsewhere", branch: "ui", number: 15 } }])
    await vi.waitFor(() => expect(notifyError.mock.calls.map(([title]) => title)).toEqual(["Cannot delete the branches of the closed review after landing of ui"]))
    expect(runIntent.mock.calls).toEqual([["/repo", { kind: "deleteClosedShownReview", branch: "ui", number: 15 }]])
  })

  it("adds the fix that landed to its review after landing and toasts a failure of it", async () => {
    tellShownNews("/repo", [{ id: 4, event: { kind: "shownReviewFixLanded", branch: "api", number: 14, robotId: "r2" } }])
    await vi.waitFor(() => expect(notifyError.mock.calls.map(([title]) => title)).toEqual(["Cannot add the fix to the review after landing of api"]))
    expect(runIntent.mock.calls).toEqual([["/repo", { kind: "extendShownReview", branch: "api" }]])
  })
})

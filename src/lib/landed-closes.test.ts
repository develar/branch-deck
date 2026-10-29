import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { Intent, News } from "@/lib/bindings"
import { tellLandedNews } from "@/lib/landed-closes"

const asked: { repositoryPath: string; intent: Intent }[] = []
const errors: string[] = []

vi.mock("@/model/intents", async importOriginal => ({
  ...(await importOriginal<typeof import("@/model/intents")>()),
  runIntent: async (repositoryPath: string, intent: Intent) => {
    asked.push({ repositoryPath, intent })
    const failed = {
      effect: { kind: "closePullRequest", branch: "auth", number: 5, comment: "c" },
      status: { kind: "failed", error: { code: "networkError", message: "GitHub is down" } },
    }
    return { kind: "ran", plan: {}, outcome: { effects: [failed], result: { kind: "none" } } }
  },
}))

vi.mock("@/lib/errors", async importOriginal => ({
  ...(await importOriginal<typeof import("@/lib/errors")>()),
  notifyError: (title: string) => errors.push(title),
}))

describe("tellLandedNews", () => {
  beforeEach(() => {
    asked.length = 0
    errors.length = 0
  })

  it("asks the session to close what each landing without a robot landed, and toasts each close that failed", async () => {
    const news: News[] = [
      { id: 1, event: { kind: "landed", landing: "landing-1", closes: [{ branch: "auth", number: 5 }] } },
      { id: 2, event: { kind: "robotMerged", robotId: "r1", closes: [{ branch: "login", number: 6 }] } },
    ]
    tellLandedNews("/repo", news)
    await vi.waitFor(() => expect(errors).toEqual(["Cannot close pull request #5"]))
    // the merge of a robot closes its own with its toast
    expect(asked).toEqual([{ repositoryPath: "/repo", intent: { kind: "closeLandedPullRequests", landing: { by: "told", id: "landing-1" } } }])
  })
})

import { describe, expect, it, vi } from "vite-plus/test"
import { BRANCH_REVIEW_WINDOW } from "@/lib/window-label"

const { reads } = vi.hoisted(() => ({ reads: [] as unknown[] }))

vi.mock("@/stores/commit-files", () => ({ loadFilesOfSubject: async () => {} }))
vi.mock("@/stores/file-diffs", () => ({ loadFileDiffs: () => {} }))
vi.mock("@/stores/settings/diff-view-settings", () => ({ getDiffWhitespacePolicy: () => "default" }))
vi.mock("@/lib/bindings", () => ({
  commands: {
    getBranchReview: (params: unknown) => {
      reads.push(params)
      return Promise.resolve({ status: "ok", data: { segments: [] } })
    },
  },
}))

const params = { repositoryPath: "/repo", baselineBranch: "origin/master", chain: ["auth", "login"] }

describe("readBranchReview", () => {
  it("gives the first read of the page the read its load started, once, and only for the same review", async () => {
    vi.stubGlobal("window", { __INIT_DATA__: { ...params, groups: [] } })
    const { readBranchReview, startReviewWindowReads } = await import("@/lib/review-window-reads")
    startReviewWindowReads(BRANCH_REVIEW_WINDOW)
    expect(reads).toHaveLength(1)
    const started = readBranchReview({ ...params, chain: ["auth", "login"] })
    expect(reads).toHaveLength(1)
    await expect(started).resolves.toMatchObject({ status: "ok" })
    // taken: a later read (a sync moved a tip) asks again
    void readBranchReview(params)
    expect(reads).toHaveLength(2)
    // another review never gets a started read
    startReviewWindowReads(BRANCH_REVIEW_WINDOW)
    void readBranchReview({ ...params, chain: ["login"] })
    expect(reads).toHaveLength(4)
  })
})

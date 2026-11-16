import { describe, expect, it, vi } from "vite-plus/test"
import { commandError } from "@/lib/api-error"

const { prefetches, warnings, answers } = vi.hoisted(() => ({
  prefetches: [] as unknown[],
  warnings: [] as string[],
  answers: [] as (() => Promise<unknown>)[],
}))

vi.mock("@/lib/bindings", async importOriginal => ({
  ...(await importOriginal<typeof import("@/lib/bindings")>()),
  commands: {
    prefetchReview: (params: unknown) => {
      prefetches.push(params)
      return answers.shift()!()
    },
  },
}))
vi.mock("@/lib/errors", async importOriginal => ({
  ...(await importOriginal<typeof import("@/lib/errors")>()),
  logWarning: (title: string) => warnings.push(title),
  logError: (title: string) => {
    throw new Error(`an error logged: ${title}`)
  },
  notifyError: (title: string) => {
    throw new Error(`an error told: ${title}`)
  },
}))
vi.mock("@/lib/window-management", () => ({ openSubWindow: () => Promise.resolve() }))
vi.mock("@/stores/repository", () => ({ getSelectedProject: () => ({ path: "/repo" }), getDiffRepositoryPath: () => "/repo" }))
vi.mock("@/stores/branch-sync", () => ({
  branchSyncStore: { getState: () => ({ baselineBranch: "origin/master" }) },
  getBranch: (name: string) => ({ name, commits: [{ hash: "abc" }] }),
  getStackAncestors: () => [],
  mainLineOf: () => [],
}))
vi.mock("@/lib/review-groups", () => ({ reviewGroupsOf: () => [] }))
vi.mock("@/stores/review-requests", () => ({ markReviewRequestRead: () => {} }))
vi.mock("@/stores/unapplied-branches", () => ({ getUnappliedBranch: () => undefined }))

const { openBranchReviewWindow } = await import("@/lib/review-window")

describe("the read ahead of a review", () => {
  it("is a warning where the IPC fails, no rejection of the page, and the next open reads ahead again; one that read is not read again", async () => {
    // the IPC fails as WebKit fails a request its page no longer waits for: the bindings answer its Error as an error
    answers.push(() => Promise.resolve({ status: "error", error: commandError(new TypeError("Load failed"), null) }))
    await expect(openBranchReviewWindow("auth")).resolves.toBe(true)
    await vi.waitFor(() => expect(warnings).toEqual(["Cannot read the review of a branch ahead"]))

    answers.push(() => Promise.resolve({ status: "ok", data: null }))
    await openBranchReviewWindow("auth")
    await openBranchReviewWindow("auth")
    expect(prefetches).toHaveLength(2)
    expect(warnings).toHaveLength(1)
  })
})

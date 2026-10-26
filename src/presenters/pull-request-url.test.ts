import { describe, expect, it, vi } from "vite-plus/test"
import type { PullRequest } from "@/lib/bindings"
import { formatNewPullRequestBlocker } from "@/presenters/pull-request-url"

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: async () => {} }))
vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))

describe("formatNewPullRequestBlocker", () => {
  it("says why GitHub's form and Create can't make a new pull request: a live one; a branch not on the remote can, it is pushed first", () => {
    expect(formatNewPullRequestBlocker("auth", null)).toBeNull()
    expect(formatNewPullRequestBlocker("auth", { number: 8, state: "open" } as PullRequest)).toBe("auth already has pull request #8")
    expect(formatNewPullRequestBlocker("auth", { number: 8, state: "merged" } as PullRequest)).toBe("Pull request #8 of auth is merged")
  })
})

import { describe, expect, it, vi } from "vite-plus/test"
import { formatPullRequestIncludes } from "@/lib/create-pull-request"
import type { CreatePullRequestFacts } from "@/lib/create-pull-request-context"
import { toCreatePullRequestContext } from "@/lib/create-pull-request-context"
import type { Branch } from "@/stores/branch-sync"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

const { branches } = vi.hoisted(() => ({ branches: {} as Record<string, Branch> }))

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: async () => {} }))
vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))
vi.mock("@/lib/bindings", () => ({ commands: {} }))
vi.mock("@/lib/git/push", () => ({ pushForAction: async () => ({ pushed: true }) }))
vi.mock("@/stores/activity", async () => (await import("@/stores/test-store-mocks")).activityMock)
vi.mock("@/stores/branch-sync", () => ({
  branchSyncStore: { getState: () => ({ baselineBranch: "origin/master" }) },
  getBranch: (name: string) => branches[name],
  // the ancestors as the view places them, by the effective base of each
  getStackAncestors: (name: string) => {
    const found: string[] = []
    for (let base = branches[name]?.stack?.effectiveBase ?? null; base !== null; base = branches[base]?.stack?.effectiveBase ?? null) {
      found.unshift(base)
    }
    return found
  },
  // the branches on the remote as they are: an action pushes the branch alone
  getPushOrders: (name: string) => ({ stack: [name], action: [name], button: [], start: [name], children: null }),
  getStackNotBuilt: () => null,
}))
vi.mock("@/stores/patronus", () => ({ getPushHold: () => null, getPatronusGitHubRepository: () => null }))
vi.mock("@/stores/pull-requests", () => ({ getGitHubRepository: () => ({ owner: "JetBrains", name: "ultimate" }), getPullRequest: () => undefined }))
vi.mock("@/stores/repository", () => ({ getFullBranchName: (name: string) => `user/virtual/${name}`, getEffectiveBranchPrefix: () => "user" }))

const FACTS: CreatePullRequestFacts = {
  repositoryPath: "/repo",
  pullRequest: undefined,
  availability: null,
  patronusAvailable: false,
  review: { status: "loading" },
  withoutReview: null,
}

/** A branch of `commitCount` commits on `parent`, on the remote as it is */
function add(name: string, parent: string | null, commitCount: number): Branch {
  const branch = testBranch(name, parent, { commitCount, remoteStatus: testRemoteStatus({ exists: true }) })
  branches[name] = branch
  return branch
}

describe("the context of the Create pull request dialog", () => {
  it("creates the pull request of a stacked branch into the baseline, with the commits of the branches it is built on", () => {
    add("auth", null, 4)
    add("login", "auth", 6)
    const oauth = add("oauth", "login", 2)
    const context = toCreatePullRequestContext(oauth, FACTS)
    expect(context).toMatchObject({ head: "user/virtual/oauth", base: "master", includes: { ancestors: ["auth", "login"], commitCount: 12 } })
    expect(formatPullRequestIncludes("oauth", context.includes!)).toBe("Includes the commits of auth and login, which oauth is built on: 3 branches · 12 commits")
    expect(formatPullRequestIncludes("login", toCreatePullRequestContext(branches.login!, FACTS).includes!)).toBe(
      "Includes the commits of auth, which login is built on: 2 branches · 10 commits",
    )
  })

  it("includes nothing more for a branch built on the baseline", () => {
    const docs = add("docs", null, 1)
    expect(toCreatePullRequestContext(docs, FACTS)).toMatchObject({ base: "master", includes: null })
  })
})

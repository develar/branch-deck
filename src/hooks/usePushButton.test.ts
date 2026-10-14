import { describe, expect, it } from "vitest"
import { getPushButtonState } from "@/hooks/usePushButton"
import type { DroppedCommit } from "@/lib/bindings"
import type { RemoteStatus } from "@/stores/branch-sync"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

function commits(count: number): DroppedCommit[] {
  return Array.from({ length: count }, (_, index) => ({ id: `c${index}`, subject: `Fix ${index}`, author: "Ann", authorTime: 0 }))
}

function state(remote: Partial<RemoteStatus>) {
  return getPushButtonState(testBranch("login", null, { stack: null, remoteStatus: testRemoteStatus(remote) }), false, false)
}

describe("getPushButtonState", () => {
  it("lists the remote commits a force push drops under its description, with how many more there are", () => {
    const push = state({ unpushedCommits: ["a"], commitsAhead: 1, myCommitsAhead: 1, commitsBehind: 12, fastForward: false, droppedCommits: commits(10) })
    expect(push.text).toBe("Force Push (1)")
    expect(push.tooltip).toBe("Force push replaces the remote branch.\n1 commit to push.\nThe remote has 12 commits your branch doesn't (by Ann).")
    expect(push.dropped).toEqual({ commits: commits(10), more: 2 })
    expect(state({ commitsBehind: 2, fastForward: false, droppedCommits: commits(2) }).dropped).toEqual({ commits: commits(2), more: 0 })
  })

  it("lists nothing for a push that drops no remote commit", () => {
    // a rebased branch: the push replaces the remote commits with their copies
    expect(state({ commitsAhead: 1, myCommitsAhead: 1, fastForward: false }).dropped).toBeNull()
    expect(state({ commitsAhead: 1, myCommitsAhead: 1 }).dropped).toBeNull()
    expect(state({ exists: false, commitsAhead: 1 }).dropped).toBeNull()
  })

  it("counts the commits the push sends in every label, the user's share in the description", () => {
    expect(state({ commitsAhead: 3, myCommitsAhead: 2 })).toMatchObject({ text: "Push (3)", tooltip: "2 of 3 commits to push are yours." })
    expect(state({ exists: false, commitsAhead: 3, myCommitsAhead: 0 })).toMatchObject({
      text: "Create Remote (3)",
      tooltip: "3 commits to push, none of them yours.\nThe branch isn't on the remote yet: the push creates it.",
    })
    expect(state({ commitsAhead: 3, myCommitsAhead: 1, commitsBehind: 1, fastForward: false, droppedCommits: commits(1) })).toMatchObject({
      text: "Force Push (3)",
      tooltip: "Force push replaces the remote branch.\n1 of 3 commits to push is yours.\nThe remote has 1 commit your branch doesn't (by Ann).",
    })
    // a force push that sends nothing only drops the remote commits
    expect(state({ commitsBehind: 2, fastForward: false, droppedCommits: commits(2) })).toMatchObject({
      text: "Force Push",
      tooltip: "Force push replaces the remote branch.\nThe remote has 2 commits your branch doesn't (by Ann).",
    })
  })

  it("says that everything is pushed, and when, for an up to date branch", () => {
    expect(state({})).toMatchObject({ text: "Push", tooltip: "Everything is pushed.", disabled: true })
    expect(state({ lastPushTime: 1704139200 }).tooltip).toMatch(/^Everything is pushed\.\n\nPushed /)
  })
})

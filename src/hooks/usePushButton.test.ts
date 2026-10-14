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
    expect(push.tooltip).toBe("Force push 1 of your commits.\nYou have 1 of your commits not on remote (1 total).\nRemote includes 12 commits you removed or changed locally.")
    expect(push.dropped).toEqual({ commits: commits(10), more: 2 })
    expect(state({ commitsBehind: 2, fastForward: false, droppedCommits: commits(2) }).dropped).toEqual({ commits: commits(2), more: 0 })
  })

  it("lists nothing for a push that drops no remote commit", () => {
    // a rebased branch: the push replaces the remote commits with their copies
    expect(state({ commitsAhead: 1, myCommitsAhead: 1, fastForward: false }).dropped).toBeNull()
    expect(state({ commitsAhead: 1, myCommitsAhead: 1 }).dropped).toBeNull()
    expect(state({ exists: false, commitsAhead: 1 }).dropped).toBeNull()
  })
})

import { describe, expect, it } from "vitest"
import { getLaneNode, getStatusBadge } from "@/hooks/useStatusBadge"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

describe("getStatusBadge", () => {
  it("says landed, in the merged hue, for a branch the baseline has all of: its push state no longer matters", () => {
    const branch = testBranch("feature-auth", null, { stack: null, remoteStatus: testRemoteStatus({ exists: false, commitsAhead: 1, myCommitsAhead: 1 }) })
    expect(getStatusBadge(branch)).toMatchObject({ text: "not pushed", color: "primary" })
    expect(getStatusBadge(branch, "origin/master")).toEqual({ text: "landed", tooltip: "Already in origin/master. Update to archive the branch.", color: "info" })
    // a sync or an error still comes first
    expect(getStatusBadge({ ...branch, status: "Error", statusText: "failed" }, "origin/master")).toMatchObject({ text: "failed", color: "error" })
  })
})

describe("getLaneNode", () => {
  it("takes the colour of the status and is filled while the branch has work for the remote", () => {
    const notPushed = testBranch("login", "auth", { remoteStatus: testRemoteStatus({ exists: false, commitsAhead: 1, myCommitsAhead: 1 }) })
    expect(getLaneNode(notPushed)).toEqual({ color: "primary", filled: true })
    expect(getLaneNode({ ...notPushed, remoteStatus: testRemoteStatus() })).toEqual({ color: "neutral", filled: false })
    // landed: nothing to push, the merged hue
    expect(getLaneNode(notPushed, "origin/master")).toEqual({ color: "info", filled: false })
    expect(getLaneNode({ ...notPushed, status: "MergeConflict", statusText: "merge conflict" })).toEqual({ color: "error", filled: true })
    expect(getLaneNode({ ...notPushed, status: "Blocked", remoteStatus: testRemoteStatus() })).toEqual({ color: "warning", filled: false })
    // a push that drops remote commits
    expect(getLaneNode({ ...notPushed, remoteStatus: testRemoteStatus({ commitsAhead: 1, myCommitsAhead: 1, commitsBehind: 2, fastForward: false }) })).toEqual({ color: "warning", filled: true })
  })
})

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

  it("is a filled node in the push colour for an amended or restacked branch: its push replaces older versions", () => {
    const restacked = testBranch("login", "auth", { remoteStatus: testRemoteStatus({ commitsRestacked: 1, fastForward: false }) })
    expect(getStatusBadge(restacked)).toMatchObject({ text: "restacked", color: "primary" })
    expect(getLaneNode(restacked)).toEqual({ color: "primary", filled: true })
    const amended = testBranch("auth", null, { remoteStatus: testRemoteStatus({ commitsAhead: 1, myCommitsAhead: 1, commitsAmended: 1, fastForward: false }) })
    expect(getLaneNode(amended)).toEqual({ color: "primary", filled: true })
    // someone else's commit beside them is still a drop
    expect(getLaneNode({ ...restacked, remoteStatus: testRemoteStatus({ commitsRestacked: 1, commitsBehind: 1, fastForward: false }) })).toEqual({ color: "warning", filled: true })
  })

  it("takes the colour of a robot that failed on the branch as it is, filled, unless the status says more", () => {
    const pushed = testBranch("login", "auth", { remoteStatus: testRemoteStatus() })
    expect(getLaneNode(pushed, null, "error")).toEqual({ color: "error", filled: true })
    // failed for missing code-owner approvals
    expect(getLaneNode(pushed, null, "warning")).toEqual({ color: "warning", filled: true })
    // a block and a landed branch say more than the robot, as their status text does
    expect(getLaneNode({ ...pushed, status: "Blocked" }, null, "error")).toEqual({ color: "warning", filled: false })
    expect(getLaneNode(pushed, "origin/master", "error")).toEqual({ color: "info", filled: false })
  })
})

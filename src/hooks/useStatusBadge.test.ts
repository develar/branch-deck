import { describe, expect, it } from "vite-plus/test"
import { getStatusBadge } from "@/hooks/useStatusBadge"
import { CREATE, forcePush, testBranch, testRemoteStatus } from "@/stores/test-branch"

describe("getStatusBadge", () => {
  it("says landed, in the merged hue, for a branch the baseline has all of: its push state no longer matters", () => {
    const branch = testBranch("feature-auth", null, { stack: null, remoteStatus: testRemoteStatus({ exists: false, commitsAhead: 1, myCommitsAhead: 1, action: CREATE }) })
    expect(getStatusBadge(branch)).toMatchObject({ text: "not pushed", color: "primary" })
    expect(getStatusBadge(branch, "origin/master")).toEqual({ text: "landed", tooltip: "Already in origin/master. Update to archive the branch.", color: "info" })
    // a sync or an error still comes first
    expect(getStatusBadge({ ...branch, status: "Error", statusText: "failed" }, "origin/master")).toMatchObject({ text: "failed", color: "error" })
  })

  it("is in the push colour for an amended or restacked branch: its push replaces older versions", () => {
    const restacked = testBranch("login", "auth", { remoteStatus: testRemoteStatus({ commitsRestacked: 1, fastForward: false, action: forcePush({ replaces: true }) }) })
    expect(getStatusBadge(restacked)).toMatchObject({ text: "restacked", color: "primary" })
    // someone else's commit beside them is still a drop
    expect(
      getStatusBadge({
        ...restacked,
        remoteStatus: testRemoteStatus({ commitsRestacked: 1, commitsBehind: 1, fastForward: false, action: forcePush({ drops: true, replaces: true }) }),
      }).color,
    ).toBe("warning")
  })
})

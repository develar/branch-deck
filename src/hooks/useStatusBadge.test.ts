import { describe, expect, it } from "vitest"
import { getStatusBadge } from "@/hooks/useStatusBadge"
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

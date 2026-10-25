import { describe, expect, it } from "vite-plus/test"
import { getReviewCommitState, isReviewCommitOutdated } from "@/lib/review-commit"
import { CREATE, forcePush, PUSH, testRemoteStatus } from "@/stores/test-branch"

describe("getReviewCommitState", () => {
  it("compares with the remote branch and the remote status by content, not with the hash of the local tip", () => {
    // the sync rebuilt the branch on a newer baseline: new hashes, the commits the remote has
    const rebuilt = testRemoteStatus({ remoteTip: "taken" })
    expect(getReviewCommitState("taken", "rebuilt", rebuilt)).toBe("rebased")
    // pushed since: the remote branch moved on, the branch has nothing it lacks
    expect(getReviewCommitState("taken", "newer", testRemoteStatus({ remoteTip: "newer" }))).toBe("changedOnRemote")
    // work the remote lacks, or remote commits the branch lacks: a push is due
    const ahead = testRemoteStatus({ remoteTip: "taken", unpushedCommits: ["c"], commitsAhead: 1, myCommitsAhead: 1, action: PUSH })
    expect(getReviewCommitState("taken", "fixed", ahead)).toBe("changedLocally")
    expect(getReviewCommitState("taken", "dropped", testRemoteStatus({ remoteTip: "taken", commitsBehind: 1, action: forcePush({ drops: true }) }))).toBe("changedLocally")
    // the older version of a commit amended here, or of the parent's: the review took what a push replaces
    expect(getReviewCommitState("taken", "amended", testRemoteStatus({ remoteTip: "taken", commitsAmended: 1, action: forcePush({ replaces: true }) }))).toBe("changedLocally")
    expect(getReviewCommitState("taken", "restacked", testRemoteStatus({ remoteTip: "taken", commitsRestacked: 1, action: forcePush({ replaces: true }) }))).toBe("changedLocally")
    expect(getReviewCommitState("taken", "local", testRemoteStatus({ exists: false, unpushedCommits: ["c"], commitsAhead: 1, action: CREATE }))).toBe("changedLocally")
  })

  it("takes the branch as it is for its own tip, and while nothing says otherwise", () => {
    // a Safe Push of the IDE took the local tip, which is not on the remote
    expect(getReviewCommitState("local", "local", testRemoteStatus({ exists: false, unpushedCommits: ["c"], commitsAhead: 1, action: CREATE }))).toBe("current")
    // the remote status is not loaded yet
    expect(getReviewCommitState("taken", "local", null)).toBe("current")
    expect(getReviewCommitState(null, "local", testRemoteStatus({ remoteTip: "newer" }))).toBe("current")
    // the sync builds the branch: whether its hashes differ is not known
    expect(getReviewCommitState("taken", undefined, testRemoteStatus({ remoteTip: "taken" }))).toBe("current")
    expect(getReviewCommitState("taken", undefined, testRemoteStatus({ remoteTip: "newer" }))).toBe("changedOnRemote")
  })

  it("is outdated only when the branch changed", () => {
    expect(isReviewCommitOutdated("changedOnRemote")).toBe(true)
    expect(isReviewCommitOutdated("changedLocally")).toBe(true)
    expect(isReviewCommitOutdated("rebased")).toBe(false)
    expect(isReviewCommitOutdated("current")).toBe(false)
  })
})

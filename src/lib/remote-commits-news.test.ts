import { describe, expect, it } from "vite-plus/test"
import type { News } from "@/lib/bindings"
import { describeRemoteCommitsNews, remoteCommitsNewsOf } from "@/lib/remote-commits-news"

describe("remoteCommitsNewsOf", () => {
  it("takes the commits of others the session fetched, and no other news", () => {
    const news: News[] = [
      { id: 1, event: { kind: "robotFailed", robotId: "r1", needsReview: null } },
      { id: 2, event: { kind: "remoteCommits", branch: "auth", tip: "d708d5b8", count: 2, authors: ["Lev Leontev", "Ann"] } },
    ]
    expect(remoteCommitsNewsOf(news)).toEqual([{ branchName: "auth", tip: "d708d5b8", count: 2, authors: ["Lev Leontev", "Ann"] }])
  })
})

describe("describeRemoteCommitsNews", () => {
  it("names who pushed how many commits to which branch, and that a push drops them", () => {
    expect(describeRemoteCommitsNews({ branchName: "bazel-dev-target-launch-v3", tip: "d708d5b8", count: 1, authors: ["Lev Leontev"] })).toEqual({
      title: "Lev Leontev pushed 1 commit to bazel-dev-target-launch-v3",
      description: "Your branch doesn't have it: a push drops it.",
    })
    expect(describeRemoteCommitsNews({ branchName: "auth", tip: "t", count: 3, authors: ["Lev Leontev", "Ann"] })).toEqual({
      title: "Lev Leontev and Ann pushed 3 commits to auth",
      description: "Your branch doesn't have them: a push drops them.",
    })
  })
})

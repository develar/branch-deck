import type { PullRequest } from "@/lib/bindings"

/** An open pull request of the branch `branchName`, #7 of `o/r`, by `me`, without checks or reviewers */
export function openPullRequest(branchName: string, patch: Partial<PullRequest>): PullRequest {
  return {
    number: 7,
    title: branchName,
    url: `https://github.com/o/r/pull/7`,
    state: "open",
    isDraft: false,
    reviewDecision: null,
    headRefName: `user/virtual/${branchName}`,
    headRefOid: `${branchName}-tip`,
    checks: null,
    updatedAt: 0,
    reviewers: [],
    author: "me",
    ...patch,
  }
}

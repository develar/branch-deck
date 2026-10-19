import type { ReviewRequest } from "@/lib/bindings"

/** A pull request of `acme/monorepo` that waits for the user's review, by alice, asked by bob; `waitingSince` grows with the number */
export function reviewRequest(number: number, patch: Partial<ReviewRequest> = {}): ReviewRequest {
  return {
    number,
    title: `Change ${number}`,
    url: `https://github.com/acme/monorepo/pull/${number}`,
    bodyText: "",
    author: "alice",
    requestedBy: "bob",
    createdAt: 1_000,
    updatedAt: 1_000,
    requestedAt: patch.waitingSince ?? 1_000 + number,
    waitingSince: 1_000 + number,
    backToYou: null,
    additions: 1,
    deletions: 0,
    changedFiles: 1,
    checksState: null,
    reviewDecision: "reviewRequired",
    reRequested: false,
    reviewedCommit: null,
    headRefName: `alice/feature-${number}`,
    headRefOid: "head",
    baseRefName: "master",
    baseRefOid: "base",
    isCrossRepository: false,
    baseIsDefault: true,
    ...patch,
  }
}

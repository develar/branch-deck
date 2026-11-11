import type { ActionState, ReviewRequest } from "@/lib/bindings"
import type { ReviewRequestActionContext } from "@/lib/review-request-actions"

/** A pull request of `acme/monorepo` that waits for the user's review, by alice, asked by bob; `waitingSince` grows with the number */
export function reviewRequest(number: number, patch: Partial<ReviewRequest> = {}): ReviewRequest {
  return {
    number,
    title: `Change ${number}`,
    url: `https://github.com/acme/monorepo/pull/${number}`,
    body: "",
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
    reviewedState: null,
    headRefName: `alice/feature-${number}`,
    headRefOid: "head",
    baseRefName: "master",
    baseRefOid: "base",
    isCrossRepository: false,
    baseIsDefault: true,
    ...patch,
  }
}

/** The commit of a pull request that the user reviewed in its window */
export const REVIEWED_COMMIT = "0123456789abcdef0123456789abcdef01234567"

/**
 * What the actions of a pull request are built from (`getReviewRequestActions`): a reviewed commit, the `actions` Rust
 * offers, no write in flight, handlers that do nothing
 */
export function reviewRequestActionContext(actions: readonly ActionState[], patch: Partial<ReviewRequestActionContext> = {}): ReviewRequestActionContext {
  return {
    reviewedCommit: { id: REVIEWED_COMMIT },
    actions,
    words: { operations: {}, named: {} },
    busy: undefined,
    onReview: () => {},
    onApprove: () => {},
    onSafePush: () => {},
    onRequestChanges: () => {},
    onOpenPullRequest: () => {},
    ...patch,
  }
}

import type { RemoteStatus } from "@/stores/branch-sync"

/**
 * How the commit a review status shows (the commit a Patronus robot took, the head of a pull request) relates to the
 * branch now. Every sync rebuilds the virtual branches on the newer baseline, so their hashes change while their
 * content doesn't: the remote branch and the remote status, which compares by content, decide, not the local hash.
 * - `current`: the branch is at the commit, or nothing known says otherwise (the remote status is not loaded yet);
 * - `rebased`: the remote branch is at the commit and the branch has the same commits, rebuilt on a newer baseline;
 * - `changedOnRemote`: the remote branch moved on since, and the branch has nothing the remote lacks;
 * - `changedLocally`: the branch has work the remote lacks (or lacks commits the remote has, older versions of amended
 *   or restacked commits included): a push is due.
 */
export type ReviewCommitState = "current" | "rebased" | "changedOnRemote" | "changedLocally"

/** `localTip`: the commit of the local virtual branch, undefined while the sync builds it; `remote`: null until loaded */
export function getReviewCommitState(commit: string | null, localTip: string | undefined, remote: RemoteStatus | null): ReviewCommitState {
  if (commit === null || commit === localTip || remote === null) {
    return "current"
  }
  // by content: a branch only rebased onto a newer baseline is up to date, the remote has its commits
  if (remote.action.kind !== "upToDate") {
    return "changedLocally"
  }
  if (commit !== remote.remoteTip) {
    return "changedOnRemote"
  }
  // while the sync builds the branch, whether its hashes differ is not known yet
  return localTip === undefined || localTip === "" ? "current" : "rebased"
}

/** The branch changed since the review took its commit: a `warning` dot */
export function isReviewCommitOutdated(state: ReviewCommitState): boolean {
  return state === "changedOnRemote" || state === "changedLocally"
}

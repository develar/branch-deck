import type { PullRequest, PullRequestView } from "@/lib/bindings"

/**
 * The open pull request as Rust reads it (`RepositoryView.openPullRequests`, `domain_policy::pull_request::base_of`,
 * pinned by `crates/domain-policy/src/pull_request_test.rs`), for the stores of page tests that mock the view: a
 * fixture under the prefix `user` and the target `master`, the page decides none of it. No candidates
 */
export function openPullRequestOf(pullRequest: PullRequest | undefined): PullRequestView | undefined {
  if (pullRequest === undefined || pullRequest.state !== "open") {
    return undefined
  }
  const base = pullRequest.baseRefName ?? ""
  const namespace = "user/virtual/"
  return {
    number: pullRequest.number,
    base:
      base === "" || base === "master"
        ? { kind: "target" }
        : base.startsWith(namespace) && base.length > namespace.length
          ? { kind: "virtual", from: base.slice(namespace.length), retarget: "master" }
          : { kind: "other", base },
    draft: pullRequest.isDraft,
    candidates: [],
  }
}

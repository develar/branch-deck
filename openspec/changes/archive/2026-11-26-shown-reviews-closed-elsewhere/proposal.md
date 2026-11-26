# Proposal

## Why

A shown review (show of ship/show/ask) closed or merged on GitHub outside Branch Deck left both of its branches,
`{prefix}/review/<branch>/head` and `/base`, on the remote forever: the listing searched only the open shown reviews,
and the only delete was the close of Branch Deck. A review reopened after its branches were deleted had no base, which
GitHub does not restore.

## What Changes

- The listing of the pull requests asks the user's shown reviews closed in the last 7 days too, a slim search of their
  own in the same query (`closed`, `@include(if: $withClosed)`), at the open of the session and then once an hour
  (`domain_policy::refresh::closed_shown_due`), with the tips their pull requests ended at and what the remote still has
  of their branches (`headRef`, `baseRef`).
- An hour after its close, while no open shown review of the branch is listed, the session tells
  `ShownReviewClosedElsewhere` (`domain_policy::shown::closed_elsewhere`); the page asks
  `Intent::DeleteClosedShownReview`, which deletes the branches the remote still has at those tips in one atomic push
  leased on them (`sync_core::review_head::delete_review_at`): a branch gone or moved since, a newer review of the
  branch, stays.
- `brd close-review <branch>` of a review closed on GitHub deletes its branches the same way (`state: closed`,
  `deleted`).
- An open of a review whose base the remote lacks pushes its branches again (`ShownReview::base_missing`).

Capabilities: `branch/shown-reviews`, `cli/agent-cli`.

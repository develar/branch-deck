# The groups of a code-owner check are of the tip it asked about

## Why

A review window remembered the groups of a code-owner check at the head its own rows held when the answer was applied
(`rememberReviewGroups`), and the session's view gives the facts to their stores before the rows. The first view of the
window's session, which brings an answer the check gave before it (the window opened while the check was held, its
subscription answered after the check), remembered them at no head: the window never marked the groups
(branch-review.spec.ts:475 in a full run at a load of 48, "Design1" for "Design3"). A view that brings a new head with
its answer remembered the groups of the new head at the old one.

The other flake of the same run, review-requests.spec.ts:1519 ("0 of its 3" for "1 of its 3"), is no fault of the
window: Approve counts the file on screen once the plan of the new head is read, and the test read the description of
Approve once while the plan was still read.

## What Changes

- `rememberReviewGroups` takes the tip the answer is about (`ReadinessEntry.tip`, the tip the session asked Patronus
  about), and `applyReadinessFacts` remembers the groups before the readiness store tells its subscribers.
- Approve gets the scenario of a new head whose plan is still read; its test holds the plan across the new head, and
  the description of Approve is read by an assertion that waits for it.
- Capabilities `branch/branch-review`, `repository/review-requests`.

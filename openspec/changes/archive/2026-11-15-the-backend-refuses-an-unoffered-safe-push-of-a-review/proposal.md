# The backend refuses a Safe Push of a review the session offers not

## Why

The review of 595c16e9 found that the engine never checked the Safe Push of a reviewed pull request
(`Intent::SubmitReview { then_land: true }`, `crates/operations/src/review.rs`): its plan was the intent's, and only the
page held to `review_actions` by not offering the item. A window of a state before (Patronus had not answered, the
pull request went into another base) or another client could approve the pull request and comment `/patronus merge`.
A rule of the domain holds in Rust, for whoever asks.

## What Changes

- `prepare` and `execute` of `SubmitReview` with `then_land` read the Safe Push of its pull request as the menus have it
  (`review_actions_of` over the view of Reviews) and refuse it before the review with the code of its blocker
  (`operations::review::safe_push_refusal`): `plan_changed` while Patronus has not answered, while the target is read or
  on a cached frame; `not_on_space` for no target or no Patronus; `pull_request_other_base`; `review_refused` for an own
  one; `no_pull_request` for one the session lists not. The run checks once admitted, before the fingerprint.
- `TestRepositoryBuilder.invoke` runs a command as another client does, without a page.
- Capability `repository/review-requests`.

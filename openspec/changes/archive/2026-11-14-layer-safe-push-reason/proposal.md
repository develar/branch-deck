# Proposal

## Why

A layer of a stack into the branch of the one below showed its Safe Push disabled with "#302 merges into
user-name/virtual/auth: Patronus would merge it there, not into master". Patronus does not merge it there: it refuses a
base no protected group names before the start (docs/design/change-review.md, decision 31).

## What Changes

- `ActionBlocker::OtherBase` says what Patronus does: "Patronus starts a Safe Push only from a pull request into
  master: #302 goes into user-name/virtual/auth"; its Rust rule says why, beside `approval_landing`.
- A table row of `review_actions` and `approval_landing` for a layer and its bottom; an e2e of the disabled reason.
- Capability `repository/review-requests`: "Safe Push of a reviewed pull request: Rust decides it on the branch Patronus
  pushes into" with the new words and the scenario of a layer.

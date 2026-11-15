# Approve and Safe Push… of a reviewed pull request stands in its place while Patronus answers

## Why

The row's menu of a pull request to review had no Approve and Safe Push… until Patronus answered: Rust decided a Safe
Push only once Patronus said it merges the repository. A menu opened before the answer lacked the item. Since an open
menu holds still (79e5279a9), the next open showed it, so the menu changed shape under the user. Request Review… of a
branch has stood from the first view since then, disabled with the wait for GitHub.

## What Changes

- `domain_policy::actions::Target::Pending`: where git names a repository Patronus knows (`LandingRemote::patronus`)
  and Patronus has not answered, the Safe Push of a pull request to review is disabled with
  `ActionBlocker::Pending { fact: Patronus }` ("Waits for the answer of Patronus"). Where git names no repository
  Patronus knows, it stays absent until Patronus says it merges there. An approval counts for no Safe Push before the
  answer.
- The row's menu and the review window keep the one builder (`getReviewRequestActions`). The action catalog has the
  blocker as an example.
- Capabilities `repository/review-requests`, `repository/reviews`.

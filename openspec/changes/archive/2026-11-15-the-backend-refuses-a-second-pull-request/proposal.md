# The backend refuses a second pull request of a branch

## Why

The audit after the review of 595c16e9: the menus offer Open Pull Request in the place of Create Pull Request… of a
branch with a live pull request, but the plan of a creation (`operations::pull_request::create_planned`) never looked
at the listing: a window of a state before or another client pushed the branch and then got GitHub's 422.

## What Changes

- `Reason::PullRequestOpen { number }`: a creation of a branch with an open pull request has it as a blocker, one with
  a pull request merged into the target `PullRequestMerged`; the run refuses it before the push (`plan_changed` for an
  open one: the view was older than the listing). One merged into a parent's branch, and a closed one, create.
- Its words in Rust and on the page, the reason catalog and the bindings.
- Capability `branch/pull-requests`.

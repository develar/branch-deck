# A profile chosen for a start by a pull request comment is refused, never dropped

## Why

The audit after the review of 595c16e9: the menus disable Safe Push With Profile where the open pull request starts
the robot by its `/patronus` comment (`ActionBlocker::CommentRoute`), but the plan of a start dropped a profile the
user chose there without a word (`decide_robot`: a comment takes no profile) and started the default checks. A window
of a state before or another client would run other checks than the user chose.

## What Changes

- `Reason::ProfileOnComment { number }`: the plan of a start with `ProfileChoice::Id` from a pull request comment has
  it as a blocker; its run refuses it with `patronus_refused`. The default checks and the profile of the last robot
  are no choice and start as before.
- Its words in Rust (`refusal_words`) and on the page (`src/presenters/reasons.ts`), the catalogs and the bindings; the
  scenario `pull-request-comment-takes-no-profile` answers the blocker.
- Capability `branch/patronus`.

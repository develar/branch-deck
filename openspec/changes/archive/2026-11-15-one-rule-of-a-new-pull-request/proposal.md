# One rule says when a pull request refuses a new one

## Why

The review of 939a5757 (bd-update-229-230-review): the rule lived in four places that disagreed: the menu of a branch
(`state != Closed`: no Create beside any merged pull request), the dialog of a branch in TS (`open || merged && landed`),
the dialog of a whole stack in TS (`state !== "closed"`) and the engine (`merged into the target`). A pull request merged
into a parent's branch hid Create in the menu while the dialog and the engine allowed it; one merged into the target
with a follow-up after it was allowed by the dialog and refused by the engine.

## What Changes

- `domain_policy::pull_request::creation_blocker(listed, landed)`: an open pull request (`PullRequestOpen`), a merged
  one that landed the subject as its next step says (`PullRequestMerged`); a merged one that did not land it and a closed
  one let a new one repeat it.
- The menus of a branch and of a whole stack offer Create only without it; the dialogs read whether Rust offers Create
  (`createOffered`), no rule of their own; the engine passes what landed the subject (`repository_model::state::landed_now`).
- Capability `branch/pull-requests`: the requirement of the backend becomes the requirement of the one rule.

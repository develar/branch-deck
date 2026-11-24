# Proposal

## Why

The monorepo works by ship/show/ask, and Branch Deck had ship (`brd safe-push`) and ask (the pull request of the
branch) but not show: land now, be reviewed after. People did it by hand: they found the landed ids, made a review on
Space and added the reviewers, and neither the row nor Your Turn saw it (#254).

## What Changes

- `brd safe-push <branch> --request-review` opens a shown review before the robot starts: a draft GitHub pull request
  from `{prefix}/review/<branch>/head` into its fork point `{prefix}/review/<branch>/base`, never master, so it can
  never land; its reviewers (`--reviewer`, else one available member of each owner group of the changed paths, from
  the CodeOwners service) are recorded in its body.
- Once the robot merged, the review opens (the app by the news of the session, `brd request-review --after-landing`):
  its head on what landed, ready for review, its reviewers asked; changes requested by one of them are the author's
  move in Your Turn, the row marks the review beside its landing.
- Once every reviewer it asked approved (their latest reviews, never GitHub's `reviewDecision`, no bot), it closes
  (the app by the news, `brd close-review`) and its branches are deleted.
- A reviewer tells a shown review by its base: no Safe Push of it is offered, Approve and merge is refused.

Capabilities: `branch/shown-reviews` (new), `cli/agent-cli`.

# Proposal

## Why

The app showed a review after landing (show of ship/show/ask, #254) in the row and in Your Turn, opened, extended and
closed it by the news of the session, but started none: only `brd safe-push --request-review` did, so a user of the app
had to leave it to land a branch and be reviewed after. Nor could the app ask one more reviewer of an open review, end
one nobody will approve, or tell a reviewer that the pull request they review is in master already: its row looked like
any request, only the review window said so. A Safe Push with a review after landing of a branch whose review is open
already pushed new branches of the review over its commits.

## What Changes

- **Safe Push and Request Review…** ("Patronus ›" of the branch menu, the command palette, and the popover of the Safe
  Push of the branch beside its start, `ActionId::SafePushRequestReview`): its popover is the Safe Push popover with a
  picker of the reviewers that starts with the code owners of the changes (`get_show_reviewers`, as `brd` chooses
  them: one available member of each owner group from the CodeOwners service, with the Space token of Patronus); the
  user removes one or adds any GitHub login of the repository; none refuses the start (`no_reviewer`). Rust offers it as
  the start refuses it.
- A Safe Push with a review after landing of a branch whose review is open already is refused (`show_review_open`): its
  fix lands by the plain Safe Push and goes into the review. A draft (its robot failed) is started again.
- **Request Review…** of a branch whose review is the one after landing asks more reviewers of it and records them in its
  body (`Intent::OpenShownReview { reviewers }`; of a draft only the record, its open asks them); **Close Review…**
  closes it after a confirmation that says what it closes without, approved or not (`allow_unapproved`).
- The row of the branch words each state of the review by whose move it is, the fix as `(auth) …`.
- A reviewer's row in Your Turn and Reviews and the review window carry the badge "Shown" (`ReviewRequest.shown`, by
  its base): in master already, the review gates no Safe Push.
- The test server answers the CodeOwners service of each test repository (`setCodeOwners`, the real shape), a review of
  someone else on its GitHub (`reviewOnGitHub`), the reviews after landing of a GitHub fixture (`shown`), and the tip of a
  branch only the remote has.

Capabilities: `branch/shown-reviews`, `repository/review-requests`.

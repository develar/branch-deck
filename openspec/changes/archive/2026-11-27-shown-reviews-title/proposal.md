# Proposal

## Why

A reviewer could not tell a review after landing from an ordinary pull request in the lists of GitHub: both are open
pull requests of the author with the title of the branch. A label was rejected: in the monorepo, creating one needs
write access, and triage access only applies a label that exists.

## What Changes

- Every opening of a review after landing (the draft of a mark, a Safe Push with `--request-review`, one of a branch
  that landed) titles it `[post-merge review] <the title of the branch>` (`domain_policy::shown::shown_title`,
  `operations::shown::shown_pull_request`, the one place every opening writes its pull request); a title that starts with
  the prefix already keeps it once.
- One opened before gets the prefix with the next record of its body (its open, its extend, a reviewer added or picked
  again): the `PATCH pulls/<n>` of the record carries the title too, and an open that writes anything records its body
  where the listing says the title lacks it (`ShownReview::prefix_missing`). Nothing finds a review by its title.
- Every reader of GitHub takes the title of a shown review without the prefix (`domain_policy::shown::listed_title`),
  before its issue key is split off: the page shows the title beside the badge, never the prefix. `FRAME_SCHEMA` 30.

Capabilities: `branch/shown-reviews`.

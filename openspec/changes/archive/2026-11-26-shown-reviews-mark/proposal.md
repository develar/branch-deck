# Proposal

## Why

The review after landing (show of ship/show/ask) could only start with a Safe Push of Branch Deck: a branch with its own
pull request was refused (`pull_request_open`), so an AI review in the pull request before landing and a human review
after it could not go together; a branch that a safe-merge started from its pull request landed had no review after
landing at all, and nothing marked a branch for one ahead of its landing, so it was lost.

## What Changes

- A Safe Push with a review after landing of a branch with an open pull request starts from its comment, refused as any
  start from it is (a draft, another base); `Reason::ShowWithPullRequest` is gone.
- `Intent::MarkShownReview` (Request Review After Landing…, `brd request-review <branch> --after-landing` before the
  landing) marks the review ahead of time: its branches at the tip of the branch, its draft opened asking nobody, its
  body recording the reviewers; refused as the Safe Push with the review is, without what refuses its robot, by the
  approvals first only without an open pull request. Of a draft it records the reviewers again; of a branch that landed
  it opens the review now. Cancel Review After Landing… closes the draft.
- Every Safe Push that lands a marked branch (its own, a child's, the whole stack's) pushes the head of its review to the
  commit it lands; its Safe Push and Request Review… with other reviewers records them.
- `domain_policy::shown::landed_head` decides what landed the branch since the draft opened (`ShownReview::created_at`):
  a merged Safe Push of it (the robots of a branch with a draft are read whatever the baseline has of it), its pull
  request merged into the target once the baseline has it whole (`PullRequest::closed_at`), the baseline by content; a
  robot or pull request of an earlier branch of the name opens nothing. The body of the review names the merged pull
  request where it was reviewed before.
- The row of a marked branch says it ("review after landing marked"), the Safe Push popover what opens once it merges.

Capabilities: `branch/shown-reviews`, `cli/agent-cli`.

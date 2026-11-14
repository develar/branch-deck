# Proposal

## Why

Review Stack showed a stack of pull requests as one review, but approved only the layer shown: a stack of seven, two of
them "Nothing to read", was seven approvals, one window state each. The spec named "Approve N Pull Requests" as not
built.

## What Changes

- `Intent::ApproveStack { top, layers }` (`operations::review`): an approval of each layer on the head the window
  showed, the bottom first, holding the review of every layer; a layer approved on that head, or an own one, is no
  effect. GitHub's head of every layer is read first: the layer whose head moved and every layer above it are skipped
  with the reason by number, the layers below approved (a stack lands bottom up, each layer with what it is built on).
- `domain_policy::stack_approval`: the layers it approves and skips, whether it can run (`ActionId::ApproveStack`,
  `ActionBlocker::StackApproved`), and the checks of the heads at the write (`head_checks`).
- The session keeps the layers of the last fetch of each stack a window shows (`ReviewRequestsFact::fetched_stacks`)
  and derives `RepositoryView.reviewStacks` by `review-stack:<top>`; the patch, the mirror of Air, the bindings carry it.
- The page: the button before Approve (the primary on All Changes), its item in "⋯", ⇧A, and Approve N Pull Requests…
  in the menu of the header of the stack in Your Turn, which opens Review Stack focused on it; one toast of the layers
  approved, one error toast of the layers refused. Where Patronus merges its hover says what Patronus counts the
  approvals for, of the `approvalLanding` of each layer: the bottom for its own Safe Push, the layers above once each
  goes into master.
- Capability `repository/review-requests`: four requirements of Approve N Pull Requests: what it approves and skips,
  what it waits for, its surfaces, the refusal of a moved head and of the layers above it.

# Proposal

## Why

The review of the screenshots of Approve N Pull Requests found: All Changes of a stack showed a disabled Approve of no
layer beside Approve N Pull Requests; the refusal of a moved head said "since you reviewed it" of a head the window
fetched, with no way to read the new commits; the hover of a skip read awkwardly; and a layer showed GitHub's count of
its files over an empty list while they were read, a count the files then contradicted.

## What Changes

- The actions of a stack itself (`review-stack:<top>` of `RepositoryView.actions`, `stack_review_actions`): Approve N
  Pull Requests alone; `ReviewStackApproval` keeps the layers it approves and skips. All Changes offers those actions:
  no Approve, Request Changes or Safe Push of one pull request, Approve N Pull Requests the primary.
- The refusal: "…moved from … to … since the window fetched it: fetch the stack again to read the new commits", its
  toast with Fetch Again, which lists the requests again.
- The hover of a skip: "Skips #302: you already approved its commit".
- A layer counts its files once read (`rangeFileCount`).
- Capability `repository/review-requests`: the four requirements of the decisions of a stack and of Approve N.

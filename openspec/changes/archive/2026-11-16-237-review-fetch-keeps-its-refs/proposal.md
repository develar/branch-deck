# Proposal

## Why

A full `pnpm e2e` on the Studio failed "should keep the review when the request is withdrawn meanwhile, and its refs"
once in 1254: the prunes were `[101, 102]`, `[102]`, `[101, 102]` instead of one `[101, 102]`. The review window asks
for the fetch of its pull request and tells its demand (`review_windows`) apart, the demand only once its own
subscription answered: under load the fetch landed before the demand in 11 of 80 runs, and the listing that withdrew
#101 came as soon as 0.2 s after the demand. A listing between the two pruned the refs of the review on screen, and the
late demand pruned again to keep them, which were gone by then.

## What Changes

- A review fetch tells the session as it starts, before it writes a ref (`Sessions::review_fetching`): the prunes keep
  the refs of the pull requests the fetches of the review windows started or brought (`ReviewRequestsFact::fetched_numbers`)
  with those the windows show and those that wait, until the window closes; the layers of a stack go with its top.
- The review requests hear of the windows that closed before the prune of that change of the demand, so a closed
  window's fetch keeps nothing.

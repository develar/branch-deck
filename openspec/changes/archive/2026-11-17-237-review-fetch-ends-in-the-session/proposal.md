# Proposal

## Why

The review of 366c4a3e6 (a review fetch tells the session as it starts, `Sessions::review_fetching`) found the
bookkeeping it added leaking and the prunes it relies on unordered:

- `fetched` and `stack_fetched` inserted whatever a fetch brought, and only a change of the demand removed it: a window
  that showed #7 while its fetch ran and moved on (or closed) dropped #7 and pruned, then the end of the fetch put #7
  back, and every later prune kept its refs for the life of the session. So did a start that reached the session after
  its window had gone, and a fetch that failed or was stopped never removed what its start made.
- `prune_review_refs` was a task of its own per prune, not ordered among them: an older `keep` could run after a newer
  one, and a prune decided before a fetch started could still delete the refs the fetch writes.

The review of the first fix of those (9b2b34249) found that one change of the windows dropped the head of a newer fetch
of the same pull request for another window (a stack window closing while the window of one of its layers fetched it),
that a failed fetch of a layer dropped the head of the stack that has it, that every prune ran, so fast navigation
queued prunes a fetch waited behind, and that nothing stopped that wait. The review of that (06daf742d) found that a
start whose first set to keep was empty never pruned, so the refs of an earlier start stayed, and that a stack window
opened again was offered Approve N Pull Requests on the heads of its earlier fetch.

## What Changes

- The head a fetch brought (`ReviewRequestsFact::heads`, the layers of a stack in `stacks`) is kept apart from what the
  prunes keep (`kept`, by the pull request a fetch writes, with the layers of its stack): a fetch whose window closed
  while it ran keeps no refs but gives its window the head; a fetch that ends without a head (an error, a stop, its
  operation dropped: `operations::review_fetch::CountedFetch`) tells the session (`Sessions::review_fetch_ended`),
  which drops it while no window shows its pull request and no other fetch of it runs; the layers of a stack stay
  while another stack that is kept has them. The end of a fetch prunes only when the set the prunes keep changed.
- The prunes of a session run one after another in the order it decided them, a prune that has not started giving its
  place to a newer one (`ReviewPrunes`); the session answers the start of a fetch once every prune it decided before
  has ended, a wait that a stop of the fetch ends (canceled), as does a wait of 120 s; the actor never waits for git.
- The first prune of a session prunes whatever it keeps, and only a change of the review windows or a listing decides
  one, so the first prune of a start is its first listing's or its first review window's, never an empty one before the
  listing came. A stack goes once no window shows its top, the heads staying for the readiness.
- The one fetch whose refs outlive its window is one that brought its head while no demand had named its pull request
  since it started; the spec and git-operations.md say so precisely.

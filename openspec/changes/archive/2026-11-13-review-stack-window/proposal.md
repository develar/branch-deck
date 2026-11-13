# Proposal

## Why

Your Turn shows a chain of requests as one stack (`RequestRow::Stack`), but its review is still one window per pull
request: twelve opens and twelve fetches for one author's chain, and nothing shows the whole of it or where one layer
ends and the next begins. A reviewer wants to read the stack as one change and answer each layer on its own.

## What Changes

- **Review Stack** on the header of a stack in Your Turn: a button on hover, the item of its menu (right click, the
  context menu key, Shift+F10) and `D` open one review window of the stack (`kind: "pullRequestStack"`, keyed by its
  top); Enter still shows or hides its layers.
- The window: "Review Stack #979–#991"; the commit column has All Changes of the whole stack, a scope per layer ("#202
  Drop workTreeRoot", its own range from the head below) and the commits under their layers; ↑/↓ walk the layers; the
  overview of All Changes lists the layers, the overview of a layer is its pull request. The decisions (Approve,
  Request Changes, Safe Push) are those of the layer of the item shown, on the head of that layer; on All Changes they
  are disabled, "Choose a pull request of the stack".
- The layers are a rule of Rust: `domain_policy::review_request_chains::stack_layers` chains them from the top down to
  the bottom over every pull request a listing named (`ReviewRequestsFact::stack`): a layer the user reviewed stays, a
  merged one leaves (the layer above goes into the default branch), one put in between joins.
- The fetch is the intent `fetchReviewStack { top, bottom }` (`operations::review_fetch`), its core
  `sync_core::review_fetch::fetch_review_stack`: one `fetch_review_head` of the head of the top with the base of the
  bottom, nothing when both are local, its range cut into the layers at their heads (`ReviewStack`, each layer with its
  own `ReviewRange`). A layer not on that range (restacked: not rebased onto the layer below as it is) is a fetch of its
  own into `refs/branch-deck/review/<top>/layers/<n>/`, kept and pruned with the refs of the top; the window says it.
- "Approve N Pull Requests" stays an open question of the spec.

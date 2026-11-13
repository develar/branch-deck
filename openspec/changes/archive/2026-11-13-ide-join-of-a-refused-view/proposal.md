# Proposal

## Why

The shim keeps the one view of a frame, and a subscription of the repository the frame holds joins that view. One that
joined while the view still opened shared its answer: when Air refused the view (brd answered `no_branch_prefix`), the
joined subscription failed with that refusal, though the page asked it later, under another config. So a focus of the
main view that subscribed again before the user saved the prefix of the welcome card, and the subscription after Save,
could end on the welcome card again with the prefix saved. In the desktop app each subscription gets an answer of Rust
asked after it.

## What Changes

- Page (`src/platform/air/shim.ts`): a subscription that joined a view whose `subscribe` failed leaves it and
  subscribes the frame anew, so brd answers it under the config as it is then.
- Capability `repository/ide-embedding`: the requirement "Page: The subscriptions of the page share the one view of its
  frame" gets the scenario of a subscription while the view of the frame opens.

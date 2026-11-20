# Proposal

## Why

Create Pull Request… and Request Review… push first what the remote lacks, and the engine refuses them while that push
can't be made (`ActionPushView::blocker`, the push of the stack branch of a whole stack). The menus offered them enabled
all the same: a branch whose sync conflicted, or whose push a running Safe Push holds, opened the dialog and was refused
at Create. One rule in two places, each with its own facts (the review of fb8b6b4a5).

## What Changes

- The menus disable Create Pull Request… and Request Review… of a branch and of a whole stack by the blocker the engine
  refuses them by: of a branch `BranchPush::action_blocker`, of a stack branch `derive::stack_push_blocker`, now one
  function of the engine and the view. A push that drops remote commits stays the user's decision, said by the dialog.

Capabilities: `branch/pull-requests`, `branch/whole-stack`.

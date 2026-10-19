# Running Operations

**Status:** Implemented  
**Updated:** 2026-10-19  
**Tags:** [frontend, actions, ux, concurrency]

## Problem

An action of the app (Unstack, Move, Squash, an amend) is a backend command and then the sync that shows its result.
Nothing recorded that it runs: a menu item's `onSelect` started it and forgot it, and the only guard
(`getRewriteBlocker`) disabled the rewrites during the sync, which starts after the command returns. A triple click on
"Unstack from air" in the notice of the expanded row sent three `restack_branch`: the backend ran them one after the
other under the repository lock, and the second and third, reading the stack the first had changed, failed with
"tool-events is not stacked" in two sticky error toasts.

The busy state that did exist was kept in a different place per action, nine of them: `activeInline.processing` of the
inline forms, `applying` of the conflict fix, `useState` flags in the Safe Push popover, the move form, the squash
dialog, the commit message editor, and the push, Safe Push, robot, pull request and Update Project stores. Each
covered only its own surface; another surface of the same action (the menu, a key, the review window) neither saw it
nor waited for it. The rule "one busy indicator per operation" of [UI Theme](ui-theme.md) was written, but nothing
held it.

## Solution Overview

One registry of the operations that run (`src/stores/operations.ts`), and one way to start an action:
`runOperation(spec, run)`. While an operation runs, every surface of the app reads the same record:

- **join**: a second start of the same action of the same object (a double click, the same item in another surface)
  runs nothing and resolves with the first;
- **wait**: an action that conflicts with a running one doesn't start; its menu item is disabled with the running
  operation's busy state as the reason ("Unstacking tool-events…");
- **one indicator**: the surface that started it shows the spinner; a start from a menu, a key or a toast shows it on
  the "⋯" of the object's row;
- **the whole action**: a rewrite runs until the sync that shows its result ends (`runRewrite`), so a row never offers
  the action again while it still shows the state before it.

## Design

```mermaid
flowchart LR
    Menu[menu item, key, toast] -->|"onSelect({ surface: null })"| Action
    Button[MenuItemButton, form, dialog] -->|"onSelect({ surface })"| Action
    Action[restackBranch, moveCommits, …] --> Rewrite[runRewrite]
    Rewrite --> Run[runOperation]
    Run --> Registry[(operationsStore: running, claims)]
    Registry --> Item[operationItem: running / disabled]
    Registry --> Row["row: busy, ⋯ spinner"]
    Registry --> Surface[the surface's spinner]
```

### The record

An `Operation` is the action's `id` (the `id` of its menu item: `unstack`, `move-to`, `squash-fixups`), its `subject`
(`branch:auth`, the row whose menu starts it), the `objects` it works on (the branches whose rows are busy), its `kind`,
its busy state (`label`: "Unstacking tool-events"), its repository, and the `surface` that started it.

### Conflicts

`getOperationBlocker(running, key)` is the one rule; `runOperation` and every builder read it.

1. **The same action of the same subject** joins: the promise of the running one.
2. **A rewrite waits for any rewrite of the repository.** Every rewrite moves HEAD or a ref of Branch Deck under one
   repository lock; the backend would run them one after the other, the second on what the first left, as the triple
   Unstack did.
3. **A remote action waits for a remote action of its subject or of a branch it works on** (restart and cancel of one
   robot).
4. **A rewrite and a remote action run side by side**, as before the registry: a push during a rewrite waits for the
   sync itself and pushes what it leaves.

A sync is no operation: a rewrite started during one still runs (an Undo in a toast during the auto-sync on focus), the
backend lock orders them and the sync chain reads the result again. Only the menus disable the rewrites during a sync
(`getRewriteBlocker`, "Wait for the sync to finish"), as they did.

### Items

A builder marks its items with `operationItem(running, item, key)`: the running action's item is `running` and
disabled, an item that must wait is disabled with the busy state as its `description`, an item disabled for its own
reason keeps it. The rewrites of the branch menu go through `rewriteItem` (the sync blocker first), the commit menus
through the same call. A surface that renders a builder subscribes to the registry and passes its snapshot
(`useOperations`, `getBranchContextMenuItems(branch, setExpanded, operations)`): a store read inside a builder that the
React Compiler memoizes would show a stale menu ([Rendering Performance](rendering-performance.md)). Callers outside
React (the review window's bridge) pass `getOperations()`.

`MenuItem.onSelect` takes a `SelectContext`: `{ surface }` of the button that started it, `FROM_MENU` (`surface:
null`) for a menu, a key or a toast. The action passes it to its operation.

### The indicator

A surface that stays while its action runs claims its key (`useIndicatorClaim`, a count per key in the store) and shows
the spinner of the operation it started (`useSurfaceBusy`): `MenuItemButton` with a `surface` (the Unstack of the
notice, `unstack-alert:<branch>`), an inline form (`inline:<branch>`, its progress row in place of itself), the Safe
Push popover (its status line), the squash dialog, the commit message editor, the move form, Apply Fix. An operation
whose surface has no mounted claim (a menu, a key, a toast, a popover the click closed) shows its spinner on the "⋯" of
its subject's row (`useUnclaimedBusy`, `MoreActionsMenu busy`), and of the branch preview's header while it is open.
Every row the operation works on is busy (`useObjectBusy`): no right-click menu, "⋯" disabled, no drag, no push.

### Rewrites

`runRewrite(spec, failureTitle, command, applied)` (`src/lib/rewrite.ts`) is the one runner of a rewrite: the command,
`notifyError` on a failure, `applied` (reveal the moved commits, say whether anything changed), then the sync, then
the toast `applied` returns, with Undo. The toast comes after the sync: the user sees the result beside its Undo, and
an Undo clicked at once never meets its own running operation. An Undo is a rewrite of its own (`undoRegroup`,
`undoSquash`). A start that must wait says so in an info toast titled as its failure ("Cannot undo the move",
"Unstacking tool-events…").

Through it: Stack On and Unstack, rename, move (and ungroup, remove prefix), squash, squash fixups and squash into,
reword, the conflict fix, and their undos. The inline forms run theirs through `withRowProcessing` (amend, unapply,
add issue reference; delete archived as a remote action), whose function includes the sync, or `withRowProgress` around
an action that is an operation itself (rename, ungroup, squash fixups, close the stack's pull request).

### Remote actions

- **The robot**: its restart and its cancel (`runRobotAction`, subject `robot:<id>`); `getRobotActions` marks them
  with `operationItem`, so the slot, the details and the landing overview show the running one (a spinner in place of
  its icon) and the other waits.
- **The pull request**: the retarget of the next step (`retargetToTarget`, subject `pr:<n>`), the creation and
  "Continue on GitHub" (`submitPullRequest`, `continueOnGitHub`: a second submit joins; one that must wait throws
  why), "Request Review…" (`RequestReviewForm`, its button the surface).
- **The review of a pull request**: Approve, Request Changes and Approve and Safe Push (`runReviewWrite`, subject
  `review:<n>`); `getReviewBusy` is what the review list and the review window read (`ReviewWindowState.busy`).

The next step reads the operation on its row (`NextStepFacts.busy`): the slot's own action runs (`running`), the
others wait with its busy state.

### Not in the registry yet

Push and stack push (`pushingBranches`, `stackPushes`), Safe Push and Dry Run (`safePushStore.starting`) and Update
Project keep their own state; push already joins a second click itself.

## Key Decisions

1. **Decision**: one registry for every window's actions, not a flag per action.
   **Rationale**: the surfaces of an action are many (menu, button, key, toast, review window); only a shared record
   lets all of them say the same.
2. **Decision**: a second start of the same action joins the first instead of being refused.
   **Rationale**: a double click is no mistake to report; the caller of the second start gets the first one's result.
3. **Decision**: a rewrite lasts until its sync ends, and its toast comes then.
   **Rationale**: the row shows the state before the rewrite until the sync; offering the action again then is what
   ran it three times.
4. **Decision**: no `expect_head` for the regroups.
   **Rationale**: an unstack writes only `refs/branch-deck/stack-parents`, HEAD may stay; the registry closes the
   double start in the app, and a `bd` process meanwhile gets the exact error under the lock.
5. **Decision**: the identical error toast updates in place (`notifyError` passes an id of the title and the
   description; Base UI updates the open toast of that id).
   **Rationale**: a poll or two surfaces must not stack copies of one error; Activity still records each.

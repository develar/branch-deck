# Landing Work into Master

**Status:** Draft  
**Updated:** 2026-10-18  
**Tags:** [product, stacks, patronus, pull-requests]

## Problem

The user works on one local branch, the master of a large repository, with many commits of several features at
once. The work is done when it is in master. In the IntelliJ monorepo it gets there through Patronus Safe Push: a
robot rebases a source branch onto master, runs the checks and pushes it into master. Approvals count only in the
pull request whose `/patronus merge` comment started the robot ([Patronus Approval](../../spec/branch/patronus.spec.md#code-owner-review)).

Branch Deck cuts that work into virtual branches, optionally stacked, so it can land in pieces. The first model of
stacks came from stacked pull requests: the pull request of a child went into its parent's branch, and the Safe Push
form of a child led "Safe Push the root first, then the next", one robot per branch. Neither lands anything into
master. A pull request into a parent's branch has approvals Patronus doesn't count and can't start a robot. A stack of
22 branches landed root first takes 22 robots, one after the other. Side branches could not land with the main line
at all.

## Solution Overview

Every operation is judged by what it lands into master and in how many robots.

- **A piece is a branch and what it is built on.** The Safe Push of a branch lands it together with every branch it
  is built on that isn't in master yet, in one robot. Its children stay. After the merge, the next sync builds them
  on master. A branch that builds without its parent can be unstacked, and then it lands alone.
- **Every pull request goes into master.** The pull request of a virtual branch, stacked or not, goes into the
  baseline as the remote names it. It holds everything its Safe Push lands, its ancestors' commits too. So its
  approvals are the ones Patronus counts, and its Safe Push starts from its comment.
- **The whole stack is one more virtual branch.** A stack that forks (side branches off its main line) also builds
  as one branch of all its commits, `{prefix}/stack/<root>`. It has its own pull request into master and lands in
  one robot. Its Safe Push is in the stack's header, and its form opens under the header.
- **A push serves the landing.** Git pushes put on the remote what a Safe Push or a pull request needs. A stack
  pushes in one atomic push.

This document is a draft while the model is implemented. The specs are the contract of each part:
[Stacked Branches](../../spec/branch/stacked-branches.spec.md), [Patronus](../../spec/branch/patronus.spec.md),
[Next Step](../../spec/branch/next-step.spec.md), [Stack Island](../../spec/branch/stack-island.spec.md),
[Pull Requests](../../spec/branch/pull-requests.spec.md) and [Whole Stack](../../spec/branch/whole-stack.spec.md).

## Design

### Key Decisions

- **The target is always master.** Branch Deck never starts a robot into another branch and never opens a pull
  request into a virtual branch.
  - *Rationale:* Patronus merges a pull request into its base and counts only the approvals of the pull request that
    started the robot. A pull request into a parent's branch can never land and its approvals never count.
- **One robot per piece, not per branch.** A stacked branch lands root..branch in one robot.
  - *Rationale:* a robot takes hours. Landing root first means one robot per branch. Each landing also rebuilds the
    children on master and force-pushes them, which dismisses their approvals.

- **The branch alone is a second way, never the lead.** A stacked branch whose trial build applies without its parent
  can be unstacked and land alone; the form offers it below the landing.
  - *Rationale:* the trial build checks lines and names, not a compiler, and a stored stack parent doesn't say why the
    branch was stacked. Landing what it is built on is always right; landing it alone is right only when the user
    says so.

### A piece: a branch and what it is built on

- **Next step.** A stacked branch has no row of its own in the [Next Step](../../spec/branch/next-step.spec.md) table
  (row 4, "Safe Push ‹root›…", is removed). It takes the rows of its own pull request into master: its approvals, its
  draft, its wait. The reasons of its ancestors come through the rows that already hold them: a parent that doesn't
  build blocks it, a Safe Push that lands the branch holds it, and a robot that lands an ancestor is the reason of
  `canStartRobot` that disables its start.
- **The form.** `describeStackLanding` (`src/lib/patronus-stack.ts`) words what the one robot lands, first and plain:
  "Lands oauth with auth and login, which it is built on: 3 branches · 12 commits into master." Start is the only start
  of the form, and Enter starts it. The ancestors with work for the remote are pushed first, a step of the start. The
  branches built on it stay; the next sync after the merge builds them on master. The trial build of the branch alone
  (`get_stack_independence`) gives the second way: the unstack, a warning when the branch uses a name of its stack,
  or the move of the commits it needs. An old pull request into a virtual branch offers its retarget to master.
- **Where it opens.** Every surface of a branch (its slot, "⋯", S, the preview, the review window) opens the branch's
  own form below its own row. The header of a stack island opens the form of the last branch of its main line right
  under the header (`InlinePlacement` `stack-header`), also while the island is collapsed; its key, processing and
  holds stay the branch's.
- **The readiness.** A stacked branch is asked for its readiness now, for its own pull request. To keep the load on
  Patronus as it was, the readiness store asks only for the branches a surface shows (`showReviewReadiness`: a row on
  the screen, an island header, the preview, the review window), and for a hidden one whose answer waits for an
  approval, so its approval is still news.

### The stack branch

The whole stack of a stack that forks is one more virtual branch, `{prefix}/stack/<root>`, which every sync builds
beside the branches ([Whole Stack](../../spec/branch/whole-stack.spec.md), "Stack branch" in
[Git Operations](git-operations.md)):

- **What it holds.** The original commits of every branch of the stack, in the order of the checked out branch's
  history, on the base of the sync: a flat virtual branch of those commits. Not the main line first and the side
  branches after: a new commit on a side branch would rewrite all of it, and an order the local history doesn't have
  can conflict where nothing conflicts locally. Patronus rebases the source with `REBASE_AUTOSQUASH` anyway, so the
  main line gains nothing from keeping its ids.
- **Only for a stack that forks.** A straight chain has none: its leaf is the whole stack, and its Safe Push lands it.
- **When it is landable.** Only a `Built` stack branch has the stack's commits as they are. A member that is blocked,
  conflicts or failed, a commit that conflicts in the stack branch itself, or a ref another process moved leave the
  ref as it was (`Blocked`, `Conflict`, `Stale`), and nothing pushes it.
- **Free while nothing changed.** Its build is a function of the base and the original commits, remembered by a hash
  of them (the process, the object cache): an unchanged sync runs no git for it.
- **Its life.** A stack that stops forking loses its stack branch, unless the stack branch is on the remote: its pull
  request may be open, and the user decides on it.

## Alternatives Considered

- **Stacked pull requests into the parent's branch.** Each pull request shows only its own commits.
  - *Why not:* Patronus doesn't count their approvals and can't start from them, so every landing goes through the API
    without approvals, or waits for retargets after each parent lands.
- **Root first: the Safe Push of a child leads to the Safe Push of its root.** Each branch lands after the one it is
  built on, its own start a muted alternative.
  - *Why not:* one robot per branch, hours each, and each landing rebuilds the children on master and force-pushes
    them, which dismisses their approvals. The form of a child led away from the branch the user asked for.
- **The whole stack as the main line's branches first, then the side branches.** The main line's commits would keep
  the ids of its leaf's virtual branch.
  - *Why not:* a commit on a side branch moves every commit after it, and the order differs from the local history,
    so it can conflict where the user sees no conflict; Patronus rebases the source branch anyway.
- **Knowing an unchanged stack branch by the identities of its commits** (author, author time, subject).
  - *Why not:* an amend keeps all three, so the stack branch would keep the old content and land it. The hash of the
    commit ids covers the content too.

## References

- [Virtual Branches](virtual-branches.md)
- [Git Operations](git-operations.md)
- [Stacked Branches](../../spec/branch/stacked-branches.spec.md)
- [Patronus](../../spec/branch/patronus.spec.md)
- [Whole Stack](../../spec/branch/whole-stack.spec.md)

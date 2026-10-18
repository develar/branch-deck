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
[Next Step](../../spec/branch/next-step.spec.md), [Stack Island](../../spec/branch/stack-island.spec.md) and
[Pull Requests](../../spec/branch/pull-requests.spec.md).

## Design

### Key Decisions

- **The target is always master.** Branch Deck never starts a robot into another branch and never opens a pull
  request into a virtual branch.
  - *Rationale:* Patronus merges a pull request into its base and counts only the approvals of the pull request that
    started the robot. A pull request into a parent's branch can never land and its approvals never count.
- **One robot per piece, not per branch.** A stacked branch lands root..branch in one robot.
  - *Rationale:* a robot takes hours. Landing root first means one robot per branch. Each landing also rebuilds the
    children on master and force-pushes them, which dismisses their approvals.

## Alternatives Considered

- **Stacked pull requests into the parent's branch.** Each pull request shows only its own commits.
  - *Why not:* Patronus doesn't count their approvals and can't start from them, so every landing goes through the API
    without approvals, or waits for retargets after each parent lands.

## References

- [Virtual Branches](virtual-branches.md)
- [Git Operations](git-operations.md)
- [Stacked Branches](../../spec/branch/stacked-branches.spec.md)
- [Patronus](../../spec/branch/patronus.spec.md)

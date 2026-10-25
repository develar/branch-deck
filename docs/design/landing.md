# Landing Work into Master

**Status:** Implemented  
**Updated:** 2026-10-19  
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

Two words are used for what lands, never for anything else:

- **A piece**: a branch and the branches it is built on, root..branch; what the Safe Push of a branch lands, and what
  "From Root" of the review window shows.
- **The whole stack**: every branch of a stack, its side branches included, named by its root ("the whole air
  stack"); what the Safe Push of the stack branch lands. A straight chain's whole stack is its last branch's piece.

- **A piece is a branch and what it is built on.** The Safe Push of a branch lands it together with every branch it
  is built on that isn't in master yet, in one robot. Its children stay. After the merge, the next sync builds them
  on master. A branch that builds without its parent can be unstacked, and then it lands alone.
- **Every pull request goes into master.** The pull request of a virtual branch, stacked or not, goes into the
  baseline as the remote names it. It holds everything its Safe Push lands, its ancestors' commits too. So its
  approvals are the ones Patronus counts, and its Safe Push starts from its comment.
- **The whole stack is one more virtual branch.** A stack that forks (side branches off its main line) also builds
  as one branch of all its commits, `{prefix}/stack/<root>`. It has its own pull request into master and lands in
  one robot. Its Safe Push is in the stack's header, and its popover opens at the header.
- **A push serves the landing.** Git pushes put on the remote what a Safe Push or a pull request needs. A stack
  pushes in one atomic push. The first push of a branch is a step of its Safe Push and its Create Pull Request, which
  push it themselves: the row, the preview and the review window offer no push of a branch the remote lacks, only its
  menu does (`isFirstPushInLanding`). A push stays a move of its own where it changes what other people see: a remote
  branch that lags, the commits of a live pull request, a force push the landing refuses to make.

The specs are the contract of each part:
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
  can be unstacked and land alone; the popover of its Safe Push offers it below the landing.
  - *Rationale:* the trial build checks lines and names, not a compiler, and a stored stack parent doesn't say why the
    branch was stacked. Landing what it is built on is always right; landing it alone is right only when the user
    says so.

### A piece: a branch and what it is built on

- **The left edge.** The node of every branch row says where the branch is on its way into master, not its push:
  ready to land, the user's step first, a fix, landing now, a wait, in master (`getLandingNode`, over the facts of the
  next step, [Branch Row](../../spec/branch/branch-row.spec.md#tree)). Filled is the user's move on this row, hollow a
  wait or done; the push keeps the dot of the status text. A table without stacks keeps the column, a lane for the
  node alone.
- **The filter.** The state chips of the table take the rows by their nodes, in the words of the overview: Your Move,
  Landing, Waiting, In Master ([Branch Filter](../../spec/branch/branch-filter.spec.md)). They replaced To Push,
  Running and Attention: a push is no state, a running robot is Landing (a Dry Run waits), and what needed attention is
  a fix among the user's moves, which F2 visits first. The move is made when the user makes it: a branch is Landing
  from the start or the restart of its Safe Push (its target check, its push), not from the moment Patronus lists the
  robot, which takes the push and up to a minute more; it waits from the click of the creation of its pull request or
  of a review request, and the request counts once the pull request has it, before Patronus answers again
  ([Running Operations](operations.md#moves)). The chip of a collapsed stack and the states of an island
  header count the rows of the stack by their nodes alike ("· 1 to fix", "4 landing", "2 waiting").
- **The header.** One button says where the work is on its way into master: the node of the most urgent move, the
  number of the user's moves, a bar of the way ([Landing Overview](../../spec/repository/landing-overview.spec.md)). A
  stack is one move, as it lands in one robot; its overview runs the next step of a move at its row, lists the robots
  that land, what waits, what is in master until the update. It replaced the count of the commits to push and the
  robots indicator: a push is a step of a move, never a move of its own.
- **Next step.** A stacked branch has no row of its own in the [Next Step](../../spec/branch/next-step.spec.md) table
  (row 4, "Safe Push ‹root›…", is removed). It takes the rows of its own pull request into master: its approvals, its
  draft, its wait. The reasons of its ancestors come through the rows that already hold them: a parent that doesn't
  build blocks it, a Safe Push that lands the branch holds it, and a robot that lands an ancestor is the reason of
  `canStartRobot` that disables its start.
- **The start, not a form.** A start of a piece has nothing to ask in the usual case: the piece is the branch the
  user pressed and what it is built on. So "Safe Push" is an action (`requestSafePush`, `src/hooks/useSafePush.ts`),
  named by what it lands ("Safe Push · 3"), and one pure decision (`getSafePushDecision`,
  `src/lib/safe-push-decision.ts`) says whether it starts at once, refuses with the reason every surface says, or
  confirms first in a popover at the button: a stacked branch always confirms, since it lands more than its row; so do
  a profile other than the default checks, a running Safe Push it cancels, approvals Patronus judges only at the
  start. The popover's title is what the one robot lands, worded by `describeStackLanding`
  (`src/lib/patronus-stack.ts`): "Lands oauth with auth and login, which it is built on: 3 branches · 12 commits into
  master."; its start takes the focus, Enter starts. The ancestors with work for the remote are pushed first, a step
  of the start shown in the slot button ("Pushing 2 branches 45%"). The branches built on it stay; the next sync after
  the merge builds them on master. The trial build of the branch alone (`get_stack_independence`) gives the second
  way: the unstack, a warning when the branch uses a name of its stack, or the move of the commits it needs. An old
  pull request into a virtual branch is no start: the next step retargets it to master first.
- **Where it opens.** Every surface of a branch (its slot, "⋯", S, the preview, the review window) asks the branch's
  own start; its popover opens at its own row's slot. The header of a stack island asks the start of the whole stack
  (below); of a straight chain, which has no stack branch, the start of its last branch, its popover at the header's
  slot (`SafePushPlacement` `stack-header`), also while the island is collapsed; its holds and its running start stay
  the branch's.
- **bd.** `bd safe-push <branch>` lands the same piece: no refusal and no flag (the old `stack_not_landed` and
  `--land-stack` are gone, the flag is accepted and ignored), the ancestors with work for the remote pushed first, the
  robot started on the branch's virtual branch. `lands` names root..branch, in the start and in `--check`; only
  `--check` adds `alone`, the same trial build of the branch alone as the popover's second way, in the trial session of
  the dangling references, so a start runs no trial ([Agent CLI](../../spec/cli/agent-cli.spec.md#bd-safe-push)).
  bd closes no pull request: the app closes those of the ancestors a merged robot landed when it sees the merge, a
  robot bd started included, so after a bd landing they stay open while the app doesn't run, and the agent says so.
  What merged Safe Pushes landed, bd decides as the rows do (below): `bd status` names it (`landed_by`), `bd push`
  leaves a branch landed whole out (`skipped`), `bd safe-push` refuses it (`landed`), and both land only the rest of a
  piece or the whole stack.
- **The readiness.** A stacked branch is asked for its readiness now, for its own pull request. To keep the load on
  Patronus as it was, the readiness store asks only for the branches a surface shows (`showReviewReadiness`: a row on
  the screen, an island header, the preview, the review window), and for a hidden one whose answer waits for an
  approval, so its approval is still news.

### Every pull request goes into master

The base of every pull request of a virtual branch is the baseline as the remote names it: `getPullRequestBase` of
`src/lib/git/new-pull-request.ts` (no branch to ask: it is the same for all), `pull_request.base` of `bd push` and the
`--create-pull-request` of `bd safe-push`. A stacked branch's pull request holds the commits of the branches it is
built on, which `Create pull request` says under its title ("Includes the commits of auth and login, which oauth is
built on: 3 branches · 12 commits", `formatPullRequestIncludes`), and its text names them under the items of its own commits ("Built on auth, then login", `pull_request_text`). A child pushed
alone (`Push oauth Only`, `bd push --alone`, a parent a running Safe Push holds) carries them: nothing waits for a parent
to be on the remote.

Pull requests opened into a parent's branch before this model, or of a parent that landed, still exist. A base under
`{prefix}/virtual/` (`isVirtualBranchBase`, `strip_branch_namespace` in `bd`) is never a choice, so it is retargeted
to the baseline (`PATCH pulls/<n>`, `retarget_pull_request`):

- the app, right before the push of the branch (`retargetBeforePush`, each branch of a stack push), and from the next
  step, row 5 "Retarget #7 to master", for any virtual base, the current parent's included
  ([Next Step](../../spec/branch/next-step.spec.md));
- `bd push`, after the push and the lock, for the pushed branches; `bd safe-push` before its push, for the branch and
  the parents its push takes along, and a refused retarget of the branch's own pull request stops the start: Patronus
  would merge it into the old base. `--create-pull-request` takes the open pull request of the branch whatever its base
  ([Agent CLI](../../spec/cli/agent-cli.spec.md#bd-push)).

Any other base (a release branch) may be meant: the push leaves it, the next step only opens the pull request (row
5′), and the menu keeps "Retarget #7 to master". The stack branch `{prefix}/stack/<root>` (below) is not under
`virtual/`, so no retarget touches a pull request of it or into it.

A retarget widens the diff of the pull request to what its Safe Push lands, so it may ask more code owners, and GitHub
may dismiss approvals. The push description says it before ("Retargets #8 to master: GitHub may dismiss its
approvals."), the push toast and Activity name each retarget after ("Retargeted #8 to master" / "login · it went into
auth"), and `bd` lists them as `retargeted` with `approved_by`, for the agent to tell the user.

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
- **A parked member.** A branch the user parked ([Parked Branches](../../spec/branch/parked-branches.spec.md)) lands
  nothing until it is unparked, and neither does a branch built on it, which carries its commits. The stack branch
  holds every member, so a stack with a parked one lands in pieces: its build stays, nothing pushes it.
- **An unapplied branch.** A branch the user unapplied ([Unapply and Reapply](../../spec/branch/unapply.spec.md)) is out
  of HEAD: it is in no stack and lands nothing; a push keeps a copy on the remote under its own name.
- **When it is landable.** Only a `Built` stack branch has the stack's commits as they are. A member that is blocked,
  conflicts or failed, a commit that conflicts in the stack branch itself, or a ref another process moved leave the
  ref as it was (`Blocked`, `Conflict`, `Stale`), and nothing pushes it.
- **Free while nothing changed.** Its build is a function of the base and the original commits, remembered by a hash
  of them (the process, the object cache): an unchanged sync runs no git for it.
- **Its life.** A stack that stops forking loses its stack branch, unless the stack branch is on the remote: its pull
  request may be open, and the user decides on it.
- **Its Safe Push.** One robot from the stack branch lands the whole stack; only the stack branch is pushed, leased,
  and the branches of the stack stay as they are on the remote. `bd safe-push <branch> --whole-stack` does it for any
  branch of the stack ([Agent CLI](../../spec/cli/agent-cli.spec.md#bd-safe-push)); a straight chain starts from its
  leaf. A stack branch that is not `Built` refuses it (`conflict` when its own commit conflicts, `stack_not_built`
  otherwise): the stack then lands in pieces. Push with Children of the root (the app's and `bd push <root>
  --with-children`) takes the stack branch along once it is on the remote or its pull request is open, and only when
  the sync built it: the pull request of the whole stack follows its branches. A stack branch on no remote and in no
  pull request is no branch to publish by itself.
- **Its pull request.** The stack branch has its own pull request into master, by its root
  (`PullRequestsResult::stack_pull_requests`), never a branch's and never retargeted. Its approvals are the ones the
  robot of the whole stack counts, so the readiness of the whole stack asks Patronus about the stack branch
  (`ReviewReadinessBranch::source`) with that pull request, and its start comments on it. Its text is written from what
  the sync read (`stack_pull_request.rs`): the stack branch's own description, else the root's title and a section
  per branch, its description or its commits.
- **The robots wait for each other.** A robot of the stack branch (`PatronusRobot::stack_root`) belongs to no
  branch, not even by its revision. It lands every branch of the stack, so a branch's Safe Push and push wait for it;
  a robot of a branch lands a part of the stack, so the whole stack's Safe Push waits for it (the `landingMember` of
  the repository view, `repository_model::patronus_view`). A robot started elsewhere on a temporary ref is matched by its tip, as a branch's is.

### The whole stack in the app

The whole stack is an object of its own in the app, beside its branches, named "the whole air stack" as `bd` names it
([Whole Stack](../../spec/branch/whole-stack.spec.md#the-whole-stack-in-the-app)):

- **Its robot** (`RobotSubject` of `src/lib/patronus.ts`: a branch, or the stack of a root) is no branch's. It is
  polled, told and listed by the whole stack ("Whole air stack merged into master"), it holds every branch of the
  stack while it runs, and its rows say "lands with the whole air stack", after its merge "Landed with the whole air
  stack" until the update. Its restart works on the stack branch and
  pushes it first while its remote lacks work (`pushWholeStack`, only a built one); its failure prompt lists the commits
  of every branch, matched by identity (the stack branch has no mapping notes).
- **Its pull request** comes from "Create Pull Request of the Stack…" (the dialog of the stack branch, its text from
  `stack_pull_request_text`, the code owners Patronus names for the stack branch asked to review, the stack branch
  pushed alone on Create) and opens from "Open Pull Request #12".
- **One list of its actions** (`getWholeStackMenuItems`, `src/hooks/useWholeStackActions.ts`) renders in the "⋯" of the
  island and under "Whole Stack ›" in the root's menu, so a small stack without an island has them too, and the root's
  preview: its Safe Push and Dry Run, the pull request, Push with Children, "Review Whole Stack" (the review window of
  the stack branch) and the actions of its robot.
- **Its Safe Push** is one more set of facts for the start of a branch (`getSafePushDecision`), its subject the key
  `stack:<root>`: the stack branch, the robot of the whole stack, its pull request, its readiness
  (`ReviewReadinessBranch::source`). The header of its island shows its next step ("Safe Push · 7", the same table as a
  branch's, [Next Step](../../spec/branch/next-step.spec.md#the-whole-stack)); its slot, its "⋯" and S on the header
  open its popover at the header, "Whole Stack ›" of the root's menu at the root's row for a small stack, and so does
  "Land the whole stack: Safe Push Whole Stack" in the popover of a piece (the root, any branch of a stack that forks):
  the piece and the whole stack are the two ways, side by side. Its readiness is asked while the header or the root's
  row shows, so the menus of a small stack know its missing approvals before a click. The popover
  says what one robot lands, the main line first and then the side branches, which only the whole stack lands ("Lands 7
  branches · 14 commits into master in one Safe Push: air, bazel, … side-fix and side-test."), and that only the stack
  branch is pushed; Enter pushes it alone and starts the robot from it, or comments on its pull request. A stack branch
  the sync didn't build disables it with the reason, and the stack lands in pieces: the Safe Push of the last branch of
  its main line is offered in its place.
- **Its approvals** count only in its own pull request, as Patronus counts only those of the pull request whose comment
  started the robot. The island's header says so ("Approvals count in #12, the pull request of the whole stack"),
  apart from the pull requests of the branches; a branch without a pull request of its own points to it ("In #12")
  while its approvals are missing; the popover of a piece says that the approvals of #12 don't count for it; and an
  approval in it is news with "Safe Push Whole Stack".
- **Closing by content.** After a merge, the open pull requests of the branches the robot landed (the ancestors of a
  piece, every branch of the whole stack) are closed, but only those that hold nothing the branch lacks: the head is the
  branch's tip, or the remote branch has no commit the branch lacks. A head compared by hash alone left a pull request
  of a branch the sync had only rebuilt on a newer baseline open, though all of it landed. Which branches it landed is
  one rule, in Rust for the app and `bd` (`repository_model::patronus_view`, `landedByRobot` of the repository view): a piece's ancestors while the robot ran on a commit its branch has (its tip, the
  same commits rebuilt, or one the branch built new commits on since: the copies below are the same objects, so the
  ancestors are as it took them), the whole stack's branches while it ran on the stack branch as it is; after a change
  below its commit, by content (below): the merge toast waits for that answer (the view the session derives after the
  answer that merged it), so the closing and the rows agree, and a branch with commits added after the landed ones keeps
  its pull request, which still shows work that did not land.
- **Landed rows until the update.** The merge toast is gone in seconds, and the update that archives the branches may
  come hours later. Until then the rows of the branches a merged robot landed say who landed them ("Landed with
  oauth", "Landed with the whole air stack"; the branch it ran on keeps "Merged"), and offer no Safe Push, no Dry Run,
  no restart of a robot, no new pull request and no push: their commits are in master, a second robot or a push would
  land nothing (`getLandedBy`, the repository view's `landedBy`, row 1 of the next step, a `landed` push hold, also of the robot's own branch). It is a
  state of the robots and the branches, not of the merge event, so a robot first seen merged (after a restart, a
  landing by `bd`) says it too. A branch built on
  a landed one lands without it ("Safe Push" where it said "Safe Push · 2"), and pushes without it: a landed branch is
  only left out of Push with Children and of a Push Up to Here, unlike a running robot, which holds every push of its
  stack. A landed commit offers no Push Up to Here on any surface, as a commit the remote has (`isLandedCommit` in
  `offersPushUpTo`: no push arrow and no item in the menu of the commit). Once every branch of
  a stack landed, its stack branch is held too: it carries only commits that are in master, so Push with Children, the
  island's push, `pushWholeStack` and `bd push <root> --with-children` leave it and say who landed the stack ("Landed in
  master with deploy and side-test: nothing of it is left to push", `getStackBranchHoldReason`), and a failed robot of
  the whole stack restarts no more (`checkStackRobotRestart`).
- **Landed by content.** A change below the commit the robot took (a new commit or an amend of a branch it is built
  on, a reword; for the whole stack any change of a branch of it, which rebuilds the stack branch) makes new copies, so
  the hashes no longer tell. Then the content does: the commits the robot tested against each branch as the table has
  it (`robot_landings_of_grouped`, `crates/sync-core/src/robot_landing.rs`, asked by the Patronus view that the session
  derives for the app and `bd` for itself, [Session facts](git-operations.md#session-facts)), the same
  comparison as the robot commit map of a failure prompt (the same commit, a patch-equivalent copy, or a copy with the
  same changes on a newer base). A branch whose oldest commits the robot tested landed, and commits added after them
  are the only work left of it ("Landed with oauth · 1 commit after it"): its push and its Safe Push send those. An
  amend of a landed commit, or a commit inserted below one, leaves the branch not landed: what landed is not what it
  is, and only the fetch tells ("Landed in origin/master"). The app asks when the robots, the remote facts or what
  a sync read changed, never for a sync of the same inputs, so an unchanged table costs nothing. The same content decides what a running robot holds
  (`robot_spans`): only the branches whose commits its commit has, so a parent a restack placed below its branch after
  the start pushes and lands on its own, and the pieces stay as small as the stacks cut them.
- **What is left of the whole stack.** Its stack branch carries every commit of the stack, the landed ones too, and
  Patronus finds those in the target. So after a piece landed its main line, the Safe Push of the whole stack lands the
  rest and says only the rest ("Lands 1 branch · 1 commit into master in one Safe Push: session; auth, login and oauth
  landed with oauth", `getWholeStackRest`); once every branch of it landed, nothing is left, and it says who landed them
  ("Landed in master with oauth and the whole auth stack").
- **A pull request that no longer matches.** The pull request of the whole stack shows the stack as it was. A Safe
  Push of a piece that lands the root, or a stack that no longer forks, leaves it showing a stack that is no more. It
  is never closed by itself, and its stack branch stays: the user may still want its review, or its approvals for
  another landing. The merge toast, the root's menu and preview and the island say "#12 no longer matches the air
  stack: auth landed", and "Close Pull Request #12…" asks in the root's row. The landing that made it so is known only
  while the app saw the merge (in memory); after a restart the reason is the shape of the stack, and once the root is
  no longer a branch (after the update that archives it) nothing in the app shows it.

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
- **Closing the pull request of the whole stack once a piece landed its root.** It holds commits that did not land,
  and its approvals may still matter to the user.
  - *Why not:* closing a pull request someone reviews is not the app's decision; it says why it no longer matches and
    asks.
- **Knowing an unchanged stack branch by the identities of its commits** (author, author time, subject).
  - *Why not:* an amend keeps all three, so the stack branch would keep the old content and land it. The hash of the
    commit ids covers the content too.

## References

- [Virtual Branches](virtual-branches.md)
- [Git Operations](git-operations.md)
- [Stacked Branches](../../spec/branch/stacked-branches.spec.md)
- [Pull Requests](../../spec/branch/pull-requests.spec.md)
- [Patronus](../../spec/branch/patronus.spec.md)
- [Whole Stack](../../spec/branch/whole-stack.spec.md)
- [Agent CLI](../../spec/cli/agent-cli.spec.md)

# Stacks of GitHub

**Status:** Draft
**Updated:** 2026-10-30
**Tags:** [product, stacks, pull-requests, github]

## Problem

A stacked branch's pull request goes into master and holds the commits of the branches it is built on
([Landing](landing.md#every-pull-request-goes-into-master)). The reason is Patronus: it merges a pull request into its
base and counts only the approvals of the pull request that started the robot, so a pull request into a parent's
branch can never land. On a repository without Patronus (the `pull-request` and `push` routes,
[Landing Routes](../../spec/branch/landing-routes.spec.md)) the same rule held for the same reason on plain GitHub: a
merge lands only into its base, branch protection counts the pull request merged into master, CI often filters on the
base. The price is the review: the pull request of `oauth` shows the commits of `auth` and `login` too.

GitHub now has native stacked pull requests (public preview,
[About stacked pull requests](https://docs.github.com/en/pull-requests/get-started/about-stacked-prs), REST
[Stacks](https://docs.github.com/en/rest/pulls/stacks?apiVersion=2026-03-10)). A stack is a linear chain of pull
requests, each into the branch of the one below, the bottom into the base of the stack. Each layer shows only its own
diff, and each is judged by the rules of the base of the stack: required reviews, required checks and CODEOWNERS of
master. A merge of a layer merges it and every layer below in one operation. That is a piece landed in one move, the
unit of [Landing](landing.md), with the narrow review the old stacked pull requests had.

## Solution Overview

- On the routes without Patronus, a stack that does not fork is a GitHub stack: the pull request of each branch goes
  into its parent's branch, the root's into the target, and the pull requests form one stack of GitHub. Patronus keeps
  every pull request into master. A stack that forks keeps the model of [Landing](landing.md) whole: GitHub stacks are
  linear, and a root reviewed in the main line, in each side branch and in the stack branch would have approvals that
  carry over nowhere.
- The route decides, with no setting of its own; a repository where GitHub has no stacks keeps the pull requests into
  master.
- Membership decides the base, not a probe: a pull request expects its parent's branch only while GitHub lists it in
  a stack whose base is the target, directly above its parent's pull request. Nothing retargets a member; an existing
  pull request into master keeps its base until the user takes it into the stack.
- Create Pull Request of a stacked branch creates the missing pull requests of the branches it is built on and the
  stack, as `gh stack submit` does, and says so first.
- The merge of a piece on the `pull-request` route is the asynchronous merge of its top layer.

## What GitHub answers

Measured on a private repository of github.com, 2026-10-30, REST `X-GitHub-Api-Version: 2026-03-10`.

- **Availability.** `GET /repos/{o}/{r}/stacks?per_page=1` answers 200 `[]` on a repository with no stack; no
  setting enables stacks. A repository the token cannot see answers 404 too, so a 404 alone does not tell a missing
  feature.
- **Create.** `POST /repos/{o}/{r}/stacks {"pull_requests": [bottom, …, top]}` answers 201 with the stack (its
  `number` comes from the numbers of the pull requests: stack and pull request numbers never overlap). It takes at
  least two pull requests ("2 items required; only 1 was supplied", 422): a chain of one branch above master is a
  plain pull request. A pull request already in a stack is refused ("Pull request #10 is already part of a stack").
  The bottom need not go into the default branch: a stack whose bottom goes into `user/virtual/zc` was created, with
  `base.ref` that branch. So the base of the stack must be checked, not assumed.
- **Add.** `POST …/stacks/{n}/add {"pull_requests": [new, …]}` appends on top (200); the same pull request again is
  refused as above.
- **Unstack.** `POST …/stacks/{n}/unstack` removes the unmerged pull requests: 204 when none is left (the stack is
  dissolved), 200 with the merged ones left in a closed stack (`open: false`).
- **Retarget.** `PATCH pulls/{n} {"base": …}` of a member is refused, 422 "Cannot change the base branch because the
  pull request is part of a stack." So the writes go in this order: the push, the unstack, the retargets, the create
  or add. After the unstack a retarget works.
- **Close.** A member can be closed (`PATCH state=closed`, 200); it stays in its stack.
- **Read.** The pull request of REST has `stack {id, number, base {ref, sha}, size, position}`, `null` outside a stack.
  GraphQL `PullRequest` has `stack {number size baseRefName entries}` and `stackEntry {position}`, so the listing of
  the pull requests reads them on its own node, no request of its own. `mergeStateStatus` of each layer is its own
  (`CLEAN`, `UNKNOWN` while GitHub computes it after a push).
- **Force push.** A push of every layer with new commits (an amend of the root, the layers rebuilt on it, as a sync
  does) keeps the stack, its order and its bases. A layer whose parent's branch moved before the layer was pushed shows
  the parent's old commit as its own until its push.
- **Merge.** `PUT pulls/{n}/merge-async {"sha", "merge_method"}` answers 202 `pending` with a `uuid`; `GET
  pulls/{n}/merge-async/{uuid}` gives `merged` (with the commit), `enqueued` (a merge queue: final, the queue merges
  later) or `failed` (a message); a 409 names a merge already pending; the result is kept 24 hours. The legacy merge
  can't merge a stack. The merge of the middle layer by rebase took under five seconds and put the commits of both
  layers into main with new ids, author and author time kept: the rows of the update pair them as after any rebase
  merge.
- **After a partial merge.** The merged layers are `MERGED` and keep their old base. The layer above goes into the base
  of the stack, and GitHub rebases it on the server: its branch gets a new commit on the merged ones, author and author
  time kept, the same patch (`rev-list --cherry-mark` marks every commit `=`). The merged branches stay (the repository
  did not delete head branches).
- **A push into master** (the `push` route) of the top layer's copy marks only the bottom layer merged, the one whose
  base is master. The layer above stays open into its parent's branch, its head in master.

Not measured: approvals across a force push of a layer that keeps its diff and across GitHub's rebase (a ruleset of a
private repository needs a paid plan, an approval needs a second account), `mergeStateStatus` under the rules of
master, GitHub Enterprise Server.

## Design

### The chain

A GitHub stack is the straight chain of a stack that does not fork: root → leaf by declared parent, starting above the
ancestors that already landed (`domain_policy::landed::landed_state`). Each layer's build must take its declared
parent, its effective base the parent's tip. A blocked, parked or unapplied member, or one built on master, ends the
chain; the branches above it go into master as before, and the row says why. A chain of one branch is a plain pull
request into master.

### The expected base

The base a pull request of a branch should have: the parent's branch while GitHub lists the pull request in a stack
whose base is the target, directly above the parent's pull request; else the target. `PullRequestBase` gains the state
of a pull request into a virtual branch outside such a stack: its merge would land nothing, so it is refused and its
next step links the stack or retargets it to master. Every reader of the base (the next step, the Safe Push decision,
the merge decision, the push plan, the page) reads the expected base from the view.

### Availability

A fact of the repository, `github_stacks`, in the pattern of the rules of the target: asked when a chain needs a pull
request, kept for the repository. 200 is available; a 404 of a repository whose pull requests GitHub lists is
unavailable; 422, 5xx and no answer are unknown, and an unknown fact waits. Settings → Landing says where the pull
requests of stacked branches go and why.

### Creation and upkeep

One rule lists the action of each branch of the chain: create its pull request, keep a member, offer the retarget of
an old pull request into master, stop at a pull request into another base or in another stack. The effects run in the
order GitHub requires: the push of the chain (one atomic push), the unstack, the retargets, the create of the missing
pull requests bottom-up, then the create or the add of the stack. A stack write that fails after the pull requests
exist leaves them as they are, a state with "Link Stack" as its next step.

A layer's text is its own commits and names the stack ("Part of the stack auth → login → oauth").

A push of a layer whose parent landed waits for Update Project: before the update the layer is still built on the
parent's old copies, and its push would replace GitHub's rebase with them; the patch-equivalence of the remote status
would not tell, since GitHub's copies carry the same patches.

### Landing

On the `pull-request` route the merge of a piece is the asynchronous merge of its top layer, after the decision read
every layer (its merge state, its approvals, its checks, its head) and named the one that holds it back. The merge has
states of its own (pending, merged, enqueued, failed); the merged write and Update Project wait for `merged`.

After a merge GitHub marks the merged layers merged, so the closing of landed pull requests leaves them to it. On the
`push` route the layers above the bottom stay open after the push, so the closing by content closes them with the
landing comment.

### The client

The stack endpoints and the asynchronous merge exist only in REST version `2026-03-10` (`STACKS_API_VERSION`); a
request names it through a client of that version (`GitHubClient::with_api_version`), every other request keeps the
default. The writes are `create_stack`, `add_to_stack`, `unstack`, `merge_async` and `merge_async_status`
(`remote_facts::pull_request_writes`), their answers `PullRequestStack` and `AsyncMerge` of `domain_types`. A 409 of
`merge-async` is the merge of an earlier request, pending, by its id: the answer of a merge that was lost is taken up,
not sent again. The `merge_action` is GitHub's default: the merge queue of a base that has one, else a direct merge.

The fake GitHub of the tests holds the stacks (`FakeStack`) with GitHub's validation (two layers at least, one stack
per pull request, each base the head below, no retarget of a member) and runs an asynchronous merge at once: the layers
up to the merged one through the `Merger`, bottom first, the layer above into the base of the stack. It does not
rebase that layer as GitHub does, and its stacks never close by themselves.

## Alternatives Considered

- **A link to the diff of the branch's own commits in the body of a pull request into master.**
  - *Why not:* a narrow view, not a narrow review: the approvals and the code owners still judge the whole diff, and the
    link goes stale with every push.
- **Pull requests into the parent's branch without a GitHub stack.**
  - *Why not:* a merge lands into the parent's branch, and the rules of that branch, not master's, judge it
    ([Landing](landing.md#alternatives-considered)).
- **A GitHub stack of the main line of a stack that forks.**
  - *Why not:* the side branches and the stack branch would review the root again, in pull requests whose approvals
    don't count for each other.

## References

- [Landing Work into Master](landing.md)
- [Stacked Branches](../../spec/branch/stacked-branches.spec.md)
- [Pull Requests](../../spec/branch/pull-requests.spec.md)
- [Landing Routes](../../spec/branch/landing-routes.spec.md)

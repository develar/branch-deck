# Stacks of GitHub

**Status:** Implemented
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
- Membership decides the base, not a probe: a pull request into a virtual branch is a layer while GitHub lists it in a
  stack whose base is the target. Nothing retargets a member; an existing pull request into master keeps its base.
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
branches that landed whole (`domain_policy::github_stack::chain_of`, over `landed_states`). Each layer's build must
take its declared parent, its effective base the parent's tip: a blocked or parked member, or one built on another
base, ends the chain, and the branches above it go into master as before. A chain of one branch is a plain pull request
into master. The session derives it into the view (`RepositoryView.githubLayers`, `repository_model::derive::
github_layers_of`): by branch, its chain, the branch below it, and the branches below it without an open pull request.
`bd` derives the same over its one-shot facts (`github_layers_over`). None on the route of Patronus.

### The base of a layer

`PullRequestBase::Layer { parent }`: an open pull request into a virtual branch that GitHub lists in a stack whose base
is the target (`domain_policy::pull_request::base_of`). GitHub's membership decides, never a fact of the session that
may change its answer, so nothing retargets a member in bulk, and GitHub refuses the retarget of one anyway. A pull
request into the branch below it in its chain that GitHub lists in no stack yet, between its creation and the link, or
after a link GitHub refused, is a layer too for the push and the view (`layer_base_of` with the parent of
`githubLayers`): a push links the stack instead of retargeting it into the target. Its merge is refused until the
stack is linked (`LayerNotLinked`): it would merge into the branch below it. No robot starts from a layer
(`starts_from`): Patronus would merge it into the branch below it.

### Availability

The listing of the pull requests asks the stack of each pull request on its own node (`stack`, `stackEntry`). A
schema of GitHub that refuses those fields (`undefinedField`) is asked again without them and remembered for the
process; the listing then says the repository has no stacks of GitHub (`PullRequestsResult.github_stacks`). The REST
probe of the draft was dropped: no setting enables stacks on github.com, a 404 of `stacks` does not tell a missing
feature from a repository the token cannot see, and the listing already answers once per refresh.

### Creation

Create Pull Request of a layer (`operations::pull_request::create_planned`) pushes the chain below it first (the push
of an action takes the ancestors with work), then opens the pull requests of the layers below it that have none,
bottom first, each into the branch below it, the bottom into the target (`CreateLayerPullRequest`, the text of its own
commits against that branch, `StartHost::layer_text`), then its own into its parent's branch (`CreatePullRequest {
base }`, the text the dialog read of its own commits against that branch), then links the stack (`LinkStack`). The
dialog says what it opens ("A layer of the GitHub stack auth → login → oauth: it shows only the commits of oauth, into
login. Also creates the pull requests of auth and login."). "Continue on GitHub" opens GitHub's form of its own pull
request into its parent's branch; the next push links the stack.

### The link of a stack

`domain_policy::github_stack::stack_writes` brings the stack of GitHub of a chain in line with it, by the open pull
requests of its branches, in the order GitHub requires: nothing while it is in line (the layers left above a partial
merge included); the new layers on top, retargeted first when they go into another virtual branch; else the unstack of
what is there, the retargets and a new stack. The layers end at a branch without a pull request and at a pull request
into the target or another base above the bottom: one of before the stacks of GitHub, which the user takes in. The run
(`LinkStack`) decides the writes again over the open pull requests the plan read and the ones it opened, and GitHub's
refusal of one fails the effect, the pull requests left as they are.

A push of a layer links its stack after the push while it is out of line (`operations::push::linking_stacks`). A layer
GitHub moved onto the target after the layers below it merged refuses the push while the branch is still built on them
(`rebased_by_github`, `LayerRebased`: Update Project first): its push would replace GitHub's rebase with the old
copies, and the patch-equivalence of the remote status would not tell, since GitHub's copies carry the same patches.

### Landing

On the `pull-request` route the merge of a piece is the asynchronous merge of its top layer (`PlannedEffect::
MergeStack`, `land_direct::merge_of` with the layers below, `layers_below`): the first layer below that a merge can't
take holds it by its own number; no auto-merge, which GitHub offers no stack. The run asks the result every 2 s, at
most 2 minutes: merged, the layer and the layers below go into the session as merged, and Update Project follows as
after any merge; enqueued, the merge queue of the base merges it later; failed, in GitHub's words.

After a merge GitHub marks the merged layers merged, so the closing of landed pull requests leaves them to it. On the
`push` route GitHub marks only the bottom merged after the push of the copy into master: the layers above it go into
the branches below them, so the close of what the push landed takes them with the landing comment (`Landing::Push`
leaves to GitHub only a pull request into the target).

### Link GitHub Stack

An old pull request into master of a layer keeps its base: no push retargets it. "Link GitHub Stack" (the menu of the
layer, `Intent::LinkStack`) takes it in: the link with `join` retargets each layer into master above the bottom into
the branch below it, then makes the stack. A layer into the branch below it that GitHub lists in no stack has the link
as its next step (row 5): its merge would land nothing (`LayerNotLinked`).

### bd and Settings

`bd push` drafts the pull request of a layer into the branch below it, with only its own commits, and names each stack
of the drafted branches with the `gh stack link` of their remote branches (`github_stacks`). Settings → Landing says
where the pull requests of a stack go by the route: into master on Safe Push, a stack of GitHub off it.

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

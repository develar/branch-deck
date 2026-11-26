# Proposal

## Why

Update Project rebases the local master, and the next sync rebuilds every virtual branch on the newer baseline. The
remote status compares content, so a branch only rebuilt since its push was up to date: Push was disabled ("The remote
branch is up to date"), and a Safe Push started without a push, relying on Patronus to rebase it. The head of its pull
request and the builds of that pull request stayed on the older master, which may not even compile; nothing in the app
moved them.

## What Changes

- The remote status tells such a branch: `RemoteStatusUpdate::older_base`, up to date by content, no fast-forward, the
  oldest commits of the remote branch and of the branch on different parents, the branch's the baseline tip. One more
  option of the walk of the commits ahead (`--boundary`), no new process; the cache version of a status goes up.
- Its row stays quiet (no mark, no chip, no next step, no count). Its push button, its push item and `P` are Push
  Rebased Branch: a force push with a lease, without a question, the branches it is built on that are on an older
  baseline too first (`BranchPush::rebased`, `Push::rebased`); no retarget of its pull request.
- A Safe Push, a Dry Run and the restart of a failed, canceled or failing robot push such a subject first, unless a
  running Safe Push holds it or a branch it is built on; the branches it is built on stay as they are. The rules of the
  approvals apply to that push as to any other.
- Create Pull Request… pushes such a subject first; Request Review… doesn't.

Capabilities: `repository/outgoing`, `branch/branch-row`, `branch/patronus`, `branch/pull-requests`. The agent CLI is
the change `rebased-push-cli`.

# Proposal

## Why

`brd update` and the app still stop on a conflict without `landed_as` when GitHub squash-merged a pull request and the
squash carries an edit of the review of its own (its lines are no sum of the local commits), or is authored under a
noreply address, which the pairing never reads: the upstream commits read are those of the local authors
(branch-deck#284). GitHub knows the exact fact: the commit it merged the pull request as (`mergeCommit`), and the head
it merged.

## What Changes

- After a conflict, after the candidates by changed lines, the local commits that nothing took and that a merged pull
  request pushed (the commits of its head that the upstream lacks, by id, identity or mapping note) are one candidate of
  its merge commit (`CandidateBy::PullRequest`), never a proof; its merge commit must be in the upstream since the merge
  base, untaken, and no twin of a local commit (a rebase merge is left to the keys).
- The app's listing keeps every merged pull request with its merge commit beside the one of each branch, out of the view
  and the frame; the update check of a sync and Update Project pair with it. A cached check that asked is reused only
  while the answer is the same, and a listing with other merged ones syncs again when the last check asked them.
- `--landed` of such a candidate drops at once only when the upstream has the change of its unit as it is, else it asks
  the user; `landed_as` says `by: pull_request` with its number. The session state is version 6.
- Patronus closes its pull requests without a merge commit: in a Patronus repository none of this applies.

Capabilities: `repository/landed-commits`, `cli/update`, `branch/pull-requests`.

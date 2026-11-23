# Proposal

## Why

On 2026-11-23 a `brd lease pick` conflicted with a commit that another session had just landed on master. In the same
call the update of the lease onto master resolved the conflict by words, and the pick again landed the result at once:
the requirement "An update that applies at once (the words resolved the conflict) is picked again at once" said so.
Nothing built or tested that tree, and the word merge was wrong as Rust: it dropped a call in `land`
(`crates/update-core/src/pick.rs`), and master stayed red for 30 minutes. A merge of words of code is a guess that no
build has seen; the update itself says so (`verify.paths`: what to build and test). The gate of `pick_requires_check`
stopped such a tree, but only in a repository that sets it, and this one does not.

## What Changes

- An update after a conflict of `brd lease pick` that applies at once because its words resolved a conflict of git
  (`update.resolved`) lands nothing, whether or not a check is required: `state: conflict` (exit 1) with the `update`
  (its `resolved` and `verify.paths` list the files), and `conflict.next` names the files and says to build and run
  the tests in the lease, then `brd lease pick` again.
- Only an update that resolved no content is picked again in the same call: the commits that conflicted dropped as
  present (`dropped`), or no conflict left in the replay. With `pick_requires_check` that pick still waits for a
  record of the new tree.
- No switch: a merge of words never lands unseen.

Capabilities: `cli/lease`.

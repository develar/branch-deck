# Proposal

## Why

The Shape of a review counted the lines of the engine of the diff views, which aligns some blocks otherwise than git:
on a test file of one commit it deleted the closing brace of the class and added it again (+98 −1 where git counts
+97 −0), and where the empty line after the final line end of the old text meets a blank line of the new one it
counts one line fewer than the file grew by. So the Shape said "+205 −7" of a change that `git diff --stat` says is
+204 −6, and a reviewer who compares the two stops trusting either.

## What Changes

- The commit diffs keep git's counts of each file beside the engine's (`FileChange::git_additions`): `--numstat` in
  the `log --raw` the plan runs anyway and in the `diff-tree` of a carried scope, git's default algorithm whatever
  `diff.algorithm` says. No process more; the `commit_diff` store goes to version 5.
- The Shape counts git's lines; a file that several commits change counts its pair as the engine does, since git's
  count of the range would take a process of its own. The tree and the header of a card keep the engine's counts
  under the whitespace policy.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/review-tour`: The Map counts the lines as git does.
- `branch/review-plan`: Modules: the lines of a module are git's counts.

## Impact

- `crates/trial-check/src/diff_scan.rs`, `crates/object-cache/src/commit_diff.rs` (store version 5),
  `crates/review-plan/src/modules.rs`, `crates/review-plan/src/pair.rs`, `git_ops::commit_files::split_numstat`
  public for the parse.

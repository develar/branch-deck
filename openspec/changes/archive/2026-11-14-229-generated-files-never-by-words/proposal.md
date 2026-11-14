# Proposal

## Why

`brd update` replayed a local commit over an upstream that changed the same generated file, a table whose first line
says that a tool wrote it (#229). The words resolved the conflict with no round, and the merge kept an upstream line
that refers to a name the local commit had renamed: text-clean, but the build could not load the file. A second file
of the same generator had no conflict and silently missed what the local commit adds to every such file. A merge of
words cannot see such a conflict, and the right resolution of a generated file is never a merge of its text: it is the
output of its generator on the merged inputs.

## What Changes

- The resolve by words takes a scope (`word_resolve::WordScope`): the update, its check inside every sync and the
  rounds of `brd update` (`word_replay`) leave a generated file a conflict (`NotResolved::Generated`), by its name, its
  attributes (`linguist-generated`, `-diff`, read in the `check-attr` of the merge attribute) or the header a tool wrote
  into a side; the copies of a sync resolve every text as before, an open trade-off
  ([develar/branch-deck#232](https://github.com/develar/branch-deck/issues/232)): there such a conflict would recur on
  every sync of two branches that change one generated file.
- A round names why each file is generated (`generated_by` of the file, kept in the state of the session), and its
  `hint` says to take a side, then run the generator and commit its output as a `fixup!`.
- An update of `brd update` that applies (at once or at the finish of a session) lists in `verify.regenerate` the
  generated files that the upstream changed since the merge base and that the update changed again (the local commits
  as they replayed, `upstream..new head`, or a round that kept its commit and took `onto`), also without a conflict
  (`git_ops::generated::merged_generated_files`); a commit dropped as landed changes nothing there.
- `generated-files` becomes the rule alone (`generated_by::generated_by` from a name, the attributes and the texts),
  with no git: the reads of the attributes move to `git_ops::generated`, which the update needs.
- Capabilities `cli/update` (the round, `verify.regenerate`) and `repository/update-project` (the check).

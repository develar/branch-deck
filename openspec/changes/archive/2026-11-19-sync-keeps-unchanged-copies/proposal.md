# Proposal

## Why

On 2026-11-19 a Safe Push of a2ui-v1 in idea-1 started no robot: "a2ui-v1 changed after the push". While its push
sent (79 s), a Fold of air-structure-rust folded a fixup whose target lay below the commits of a2ui-v1 and air-cli;
`replay` gave those commits new trees and a new committer, and the fold carried no mapping note to them, as it carries
none to a commit whose trees changed. The sync after the fold wrote every copy of both branches again: the same trees,
messages and authors, only a newer committer date, so new ids. The check before the start saw the branch off the
pushed commit and skipped the start, as it must. Any rewrite below a branch that leaves what the branch holds (a fold,
a squash, a drop or a reword of another branch's commit, a rebase outside Branch Deck without `notes.rewriteRef`)
pushed the same commits again under new ids, and refused the Safe Push that raced it.

## What Changes

- A sync keeps the commit of a virtual branch on the same parent whose tree, message and author are those its copy
  gets, with or without a mapping note, and writes the note anew; so does a mapping note whose source trees are not the
  commit's any more. The check costs the merge a copy costs anyway, no process more.
- The build of a stack branch keeps the copies it holds the same way, read in the one `log` of the existing branches.

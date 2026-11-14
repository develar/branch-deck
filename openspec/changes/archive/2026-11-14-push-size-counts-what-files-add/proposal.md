# Proposal

## Why

The push size check counted every version of a file the commits of a push change, and each changed file whole. A
branch of 59 commits was refused as "50.0 MiB in 773 files", naming one 700 KiB design doc three times as "the largest";
git would have sent a thin pack of 1.4 MiB, since it sends the versions of a file as deltas of each other and of the
remote's version. The limit is for build output committed by mistake, new files, which stays caught as it was.

## What Changes

- `git_ops::push_contents`: each path the commits change once, by its largest version less the remote's version of the
  path (the `<src>` of its first diff), dropped when it adds nothing; a blob the remote has (the `<src>` of a diff no
  earlier commit wrote) counts nothing, so a moved or reverted file adds nothing. Still one `log --raw` (now with
  `--format=tformat:@`, a mark per commit) and one `cat-file --batch-check`.
- Capability `repository/outgoing`: the requirement of the files of a push says what a file counts.

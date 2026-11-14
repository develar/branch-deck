# Proposal

## Why

The commit of the files of a worktree (`worktree_commit`: the scans of a host slot before and after a run, the commit
of the working tree of `brd-host run`) must be what `git add --all` stages. Its files become blobs by
`hash-object -w --stdin-paths`, which reads no index: an edited file whose blob in the index holds CRLF, under
`text=auto`, `git add` stages with its CRLF (the safer autocrlf of git), but `hash-object` normalized it to LF, so the
host got other bytes than `git add` would give. The root of #230 outside the update.

## What Changes

- A tracked file that holds a CR, whose blob in the index is text with CRLF, under attributes that leave git the safer
  autocrlf (`crlf::crlf_kept_paths`), is hashed as it is (`hash-object -w --no-filters --stdin-paths`): one `check-attr`
  and one `cat-file --batch` of the index blobs more, none when no tracked file holds a CR.
- Capability `cli/lease`: a scenario of the commit of the files of a worktree.

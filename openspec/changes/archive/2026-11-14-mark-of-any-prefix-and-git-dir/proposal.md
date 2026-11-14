# Proposal

## Why

The review of b580e1ef and fb389778 found two holes of the mark of a sync. Its reader found the git directory of a
worktree only for a main worktree and a linked one, so the mark a sync wrote in a submodule or in a checkout of a
separate git directory was never read, and such a repository waited for the user's first sync each time. And the file of
a prefix was named by the hex of its bytes: a prefix longer than about 120 bytes gave a name no file system takes, and
was never marked.

## What Changes

- The reader takes the git directory the `.git` of the worktree names (`git_ops::lease_slot::git_dir_of`): `.git`
  itself, or what the file `gitdir: <dir>` names, as `git rev-parse --git-dir` does, where the sync writes the mark.
- The file of a prefix is named by the SHA-256 of the prefix in hex and holds the prefix, which the read checks; a file
  of the build before, named by the hex of the prefix, is read and moved by the next mark.
- Capability `repository/onboarding`: the requirement of the mark of a sync.

# Proposal

## Why

The probe of the preview (#230) answers for a hashed file while the stamps of its attribute sources stay. It watched
the global file only at its default place: under a `core.attributesFile` of the user's own, an edit of that file left
the probe answering what the old attributes gave, and the system file of the git binary was not watched at all. And
where a tracked `.gitattributes` is deleted only in the working tree, git reads its index blob, while `hash-object`,
which reads no index, reads none: the preview called clean a file git shows as changed.

## What Changes

- `core.attributesFile` is read with the one `git config` of the operation (`ConfigSnapshot`), placed as git places it
  (a relative path from the root of the worktree, `~/` from the home), and watched with the system file of the git
  binary (`git var GIT_ATTR_SYSTEM`, asked once per process): the check inside a sync and the preview of `brd update`
  put them on their executor (`with_attribute_files`); a value the preview can't place (`~user/`, `%(prefix)/`, a key
  without a value) keeps no answer of the probe for a later check.
- A `.gitattributes` of the working tree is a source only as a regular file, as git reads it (it follows no link
  there).
- The same `cat-file --batch-check` of the index entries reads `:<dir>/.gitattributes` of each directory of a changed
  path: a file under one that the index has and the working tree lacks blocks.
- Tests: the nested `.gitattributes` in the key, attributes rewritten within the racy margin with the same stamp, the
  global and the system file, the kept probe of an unchanged sync, a CRLF split by the chunks of the read of the
  commit of a worktree.

# Proposal

## Why

`brd update` asked the user about a blocked path (`ask_user: true`, `blocked_paths`) for a file nobody had changed: it
was byte for byte its blob in HEAD and `git status` showed it clean, but the blob had been committed with CRLF line
ends and `.gitattributes` gives the path `text=auto eol=lf` (#230). The cheap check of the local changes
(`preview_blocking_paths`, the check of every sync and the early `blocked_paths` of a resolve session) compares the
file's `hash-object --stdin-paths`, which applies the clean filters, with the old blob: the filter normalizes CRLF to
LF, so the id differs. git itself keeps such a blob as it is (the safer autocrlf of `text=auto`), and the finish went
through.

## What Changes

- A file whose filtered id differs from the old blob, but whose length is the blob's size (`%(objectsize)` of the same
  `cat-file --batch-check` of the index entries), is compared as it is too: the id of its bytes without the filters
  (`commit_utils::file_blob_id`, what `hash-object --no-filters` gives, in process, the file read in chunks). Equal to
  the old blob, it blocks nothing where git keeps such bytes (`crlf::crlf_kept_paths`: the safer autocrlf of
  `text=auto`, `-text` or neither `text` nor `eol`, and no `ident`, `filter` or `working-tree-encoding`; one
  `check-attr` of such files only); where git converts them (`text`, an `eol` alone), git shows the file changed, and
  so does the preview. The probe of the preview keeps that id beside the filtered one.
- Capability `repository/update-project`: a file that differs from its blob only by the clean filter does not block.

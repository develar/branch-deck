# Proposal

## Why

Under Ignore Whitespace and Blank Lines the review window of idea-1 `jna-one-copy` marked the counts of
PluginClassLoader.kt, "+63 −0" with "1 line hidden by Whitespace" in its hover, but the row of its step, its commit,
said "+196 −2" with no mark, where the main window says git's "+196 −3": the sum under the policy differed from git's
and nothing said why. A directory, a group of a concern or of a kind summed the lines of its files the same way, unmarked.

## What Changes

- Each group of the tree (a directory, a step and its commit, a concern, a section of a large step, a group of a kind)
  sums the lines the whitespace policy hides in its files whose diff is read (`FileTreeDirectory.hiddenLines`), as it
  sums their lines; its counts carry the dotted underline of a file, their hover and the description of its row the
  words "1 line hidden by Whitespace" of the sum. A row "also in" a group counts none of the lines of its file, as it
  counts none of its lines; a group whose read files hide nothing has no mark.
- The template `review_whitespace`: a branch of two commits with bodies, the first a blank line apart from a real
  change; an E2E test switches the policy in its review window and reads the row of each step.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-files`: Reading the diffs: the counts of a group mark the lines its files hide.

## Impact

- `src/lib/file-tree.ts`, `src/components/files/FileTree.tsx`; the template of `crates/test-utils/src/repo_template.rs`
  and the mock server's list of templates.

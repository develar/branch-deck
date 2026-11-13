# Proposal

## Why

A real check on a GitHub stack of seven layers found three defects of the reading chips. At a 1000 pt window the
label chips of a layer row were cut mid-word ("refac", "refa") while the claim chip beside them, "Labelled refactoring
· 4 changes to read", said in full the count the reading chip at the end already said ("4 changes to read in 1 file").
The header of the stack said "1 with nothing to read · 6 to read · 14 changes", which reads as if 6 and 14 were of one
unit. And the column of the Review Stack window showed every layer alike ("1 commit"): a layer with nothing to read,
which the window passes, looked like the others, so nothing said why.

## What Changes

- A label the diff proves wrong is that label's own chip in the warning tone, first of the labels, its hover why and
  how much is left to read; no second chip, no repeated count. A word of the title the diff proves wrong is one compact
  warning chip, "refactor: in the title", its count in its hover. The same in the header of the review window.
- A row of Your Turn and Reviews gives way in one order: the title first, down to its first words, then the meta, then
  the rest of the title, then the labels, each cut with an ellipsis; what it leaves to read and its age never shrink.
  Before, the meta gave way before the title, and the labels were clipped mid-word with it.
- The sum of a stack counts each thing of its own unit: "1 of 7 with nothing to read · 14 changes to read", "Nothing to
  read" when no layer has anything. Rust's `StackReading` carries the count of the layers (`layers`).
- The scope of each layer in the column of the Review Stack window says what it leaves to read as a compact chip
  ("Nothing to read" in success, "4 to read"), its hover the whole verdict; a layer with nothing to read is muted.

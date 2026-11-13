# Proposal

## Why

A reviewer gets twelve overnight pull requests of one author, a chain, and wants to drop the refactorings first. What
a pull request is cannot be told by its title or its labels: those are the author's word. What it leaves to read can
be proven: the marks of the diffs already pass formatting, renamed names, imports and moves, file by file
(`DiffReading`). Summed over a range, by the role of each file, that proof says "Nothing to read" of a refactoring and
"3 changes to read" of one that changed a body, and it flags a label the diff contradicts. No LLM, no setting, nothing
written to GitHub.

## What Changes

- **`RangeReading`** (`domain_types::range_reading`): the `DiffReading` of every file of a range summed by its role
  (code, tests, prose, build, resources), the files no proof can be made of with why, the kinds of proof that passed a
  change; `review_plan::reading` proves it from the full marks of `file-diffs` (`FileDiffs::collect`, then the answers
  of the index), the same count as the badge of each file in the window. The plan carries it (`ReviewPlan.reading`);
  its renames wait for the index before it answered, and such a plan is not final.
- **The verdict** (`domain_policy::range_reading`): nothing to read, tests only, docs only, or what is left, with the
  files not proven; **the author's word** (`domain_policy::author_claim`): a label or a conventional type of a closed
  list (refactor, tests, docs), checked against the reading in one case: the diff leaves changes to read outside the
  role the word names. A fix or a feature is never checked.
- **The window**: the header of a pull request says what it leaves to read, its labels and the chip of a word the diff
  proves wrong ("Labelled refactoring · 3 changes to read"); Review Stack reads every layer in one read
  (`get_range_readings`: one `diff-tree --stdin` of the lists of the layers, one `cat-file --batch`, then the index
  layer by layer), opens on the first layer to read, folds a layer with nothing to read with its proofs and its
  Approve, ↑/↓ pass it, and the overview sums the stack ("2 with nothing to read · 2 to read · 2 changes").
- **`brd review-plan`**: schema 35, `reading`.

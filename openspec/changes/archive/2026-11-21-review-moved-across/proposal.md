# Proposal

## Why

A function moved to another file is two units of the review plan, a removal and an addition, each read whole; a
function extracted into a new file is a removal beside a new file, one moved out of a file the commits delete an
addition beside a deleted file. The plan knows nothing ties them, so neither does an agent that reads `brd
review-plan`, nor the window that will one day read the two as one (step A2 of the plan of 2026-11-21, after
JetBrains/patch-splitter). The smallest honest step is a word: the plan says which removal and which addition are the
same declaration moved, before anything folds it.

## What Changes

- `ReviewUnit::moved_to` of a unit `removed` and `ReviewUnit::moved_from` of a unit `added` (`MovedPlace`: the `path`
  and the `line` of the name at the other place), where exactly one declaration of one side alone before the commits
  and one after them, in two files, have the same content of their tokens (`whitespace::Sides::code`, as
  `Sides::same_code` compares a pair that moved inside its file). The other place is a unit `added` or `removed`, or
  an outermost declaration of a new or a deleted file, whose whole unit says nothing. A code of two places on a side,
  a move with a changed line and a move inside one file say nothing.
- Pure: no git process and no query of the index (`accio moves`' `moved` is not asked): the tokens the fingerprints
  read already, and of a new or deleted file only where one of its outermost declarations has the name of such a unit.
- The units, their order, fingerprints, concerns, steps and the window are those without the word.
- `brd review-plan`: `moved_from`, `moved_to`, `"schema": 39`.

Capabilities: `branch/review-units`, `branch/review-plan`, `review/range-reading`.

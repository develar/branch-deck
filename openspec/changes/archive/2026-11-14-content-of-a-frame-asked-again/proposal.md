# Proposal

## Why

The review of the view of the robots as they are found four gaps. A git error of the content of a robot's commit was
logged at debug: the derivation answered, the part was `Fresh` without the facts only the content tells, and nothing
asked again. The hold of a branch a merged robot landed read the latest answer of the robot, which can be of the branch
as it was: a branch with a commit since stayed held as landed in the view (its readers judge it again, the view
itself did not). The cache of the view of the robots as they are by the version of the memo had no test. And the frame
of the session kept the whole memo: answers of states the table left long ago, and answers of the rules of an earlier
binary, which a derivation of a new binary never asked again because the memo had them.

## What Changes

- `learn_content` returns the error of the first call of the content that failed, the answers of the other kept;
  `patronus_view` and `facts::derive_patronus` fail with it and the view of what the memo has (`ContentFailed`). The
  session's derivation fails (`Stale` with why, `Unavailable` without an earlier view), and the next answer of Patronus
  derives it again; meanwhile the session makes the view of the robots as they are by what the memo has. A one-shot
  command of `brd` and the checks of a push read that view, the failure in the log.
- The hold of a view is decided of the branch as the table has it: a landing holds only when what landed the branch
  covers its commits as they are (`BranchTable::landed_whole`), as `derive::hold_now` reads it.
- A test of the cache of `PatronusFact::publish`: a publish of the same inputs after the memo kept an answer makes the
  view again.
- The frame keeps of the memo the answers of the questions of the listed robots over its table (`content_keys`) and the
  latest answer of each listed robot (`ContentMemo::frame_of`). A memo seeded from a frame marks its answers
  (`MemoAnswers::from_frame`): the views read them, and the first derivation that needs one asks git again, so the
  rules of the binary that reads the frame answer from its first derivation on. Chosen over a version of the rules in
  the frame: a version has to be raised by hand with every change of the rules of the content, and forgetting it keeps
  the old answers; the frame's answers asked again cost one derivation's git once per start, and only for robots whose
  content the hashes don't tell. The format of the frame is the same (`FRAME_SCHEMA` 16).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/patronus`: a content git does not tell fails the derivation; the frame keeps the answers of its table, asked
  again by the first derivation; the hold of a landing is of the branch as the table has it.

## Impact

- `crates/repository-model/src/{patronus_view,facts,derive,runtime,one_shot,push_guard,warm_start}.rs`.
- `docs/design/{landing,git-operations,rust-domain-authority}.md`.

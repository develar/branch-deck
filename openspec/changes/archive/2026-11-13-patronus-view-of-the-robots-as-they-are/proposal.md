# Proposal

## Why

The view of the robots the session showed was the one its last derivation answered, of the robots and the table it
was derived of. A Safe Push started after it (`brd safe-push`, `brd land`) held nothing until a new view was derived,
though the robots listed it already; a robot that ended since still refused a start in the step; a branch added since
had no robot that lands it. Only the content of the robots' commits needs git, yet the whole view waited for it.

## What Changes

- One function makes every view (`patronus_view::view_of`): over the table, the remote branches and the robots, by
  the hashes and the answers of the content the memo keeps, no git. The derivation asks git what the memo lacks
  (`learn_content`), then calls it; the session calls it again whenever the robots, the table or the remote branches
  change and are not what the derived view is of (`FactStore::publish_patronus`), `Stale` until the derivation answers;
  `brd` (one-shot sessions, the robots a push checks) calls the same.
- The memo keys what the content told by the robot and the commits it covered, and keeps the latest answer of each
  merged robot, what the hashes told included: a view of a state not asked yet reads it, judged against the branches as
  they are (`LandedByView::knows`, `is_whole_of`). The frame of the session keeps the memo (`FRAME_SCHEMA` 16).
- The view builds the rules' input of the table once instead of once per branch: a rebuild over 141 rows and 46
  robots takes 0.9 units of the derive budget (about 2 ms in a debug build, 9.5 before), every derivation too.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `branch/patronus`: the view is of the robots, the table and the remote branches as they are; the memo keys what the
  content told by robot and keeps its latest answer and is kept in the frame.
- `branch/next-step`: only what the content told can be of another state than the branches have.

## Impact

- `crates/repository-model/src/{patronus_view,facts,runtime,one_shot,warm_start,derive}.rs`.
- `docs/design/{landing,git-operations,rust-domain-authority}.md`.

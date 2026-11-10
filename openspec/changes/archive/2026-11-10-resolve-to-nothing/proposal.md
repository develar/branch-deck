# Proposal

## Why

A property test of the merge asked each side merged again with the result to resolve, and failed at 20,000 cases on
left `}`, base `}\n`, right `a\na\n`: the words resolve it to `a\na`, and the right side merged again with that
result is one conflict of its last line. `git merge-file` conflicts on both merges, and on every such merge again that
1M random merges gave: the side and the result each align with the base on their own, so the expectation was wrong,
not the merge. At 40,000 cases the property then found a resolve the composition refused: left `}`, base `}\n`, right
`` (the last line removed on one side, its final line end on the other) resolves to the empty text, which
`compose_merge` called inexact, as it calls any resolve that leaves nothing at the end, although no line was before
it whose end it would decide. The resolve by words left such a conflict to git.

## What Changes

- The property no longer asks a merge again to resolve, and says why; two tests keep the shrunk case, one of them
  against `git merge-file`.
- `compose_merge`: a resolve by words that leaves nothing at the end is inexact only when a line is before it; without
  one, the result is the empty text, exact. `RESOLVE_VERSION` 3: such a conflict now resolves.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cli/agent-cli`: `brd sync`: A conflict the words resolve is committed, also when they leave nothing of the file.

## Impact

- `crates/text-diff/src/apply.rs`, `crates/git-ops/src/word_resolve.rs` (`RESOLVE_VERSION`), the tests of the merge
  and the resolve; `docs/design/text-diff.md` (Merge, Result).

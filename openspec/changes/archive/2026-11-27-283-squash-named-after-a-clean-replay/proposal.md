# Proposal

## Why

The candidates by changed lines (#265) name a pull request that GitHub squashed under another identity only after a
conflict. When the replay of the squashed commits onto the squash is clean, they go empty: `brd update` drops them as
`proof: empty`, and the landed report says `squashed` without the baseline commit, though their changed lines sum to it
(branch-deck#283).

## What Changes

- After a replay, the local commits it left empty without a key, a proof or a candidate land as the one upstream commit
  of their author that nothing else names whose changed lines the smallest set of them sums to
  (`landed_by_lines::squashes_of_gone`); a set that another upstream commit's set shares a commit with names neither.
  A fixup of a commit just named takes its squash.
- The preview and the update read no paths again: the check keeps the sets of paths of its pairing
  (`UpdatePlan::landed_paths`). One `diff-tree -p -U0` only with gone commits and an upstream commit whose paths they
  cover; a session finish reads the paths first (one `diff-tree --name-only`) and leaves out the commits the user
  dropped before the search.
- `dropped` names the squash as the `upstream` of those commits, still with `proof: empty`.

Capabilities: `repository/landed-commits`, `cli/update`.

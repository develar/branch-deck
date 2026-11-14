# Proposal

## Why

A pull request that only renames a function (`return sumOf(entries)` to `return totalOf(entries)`) showed each call
under the heading "balance — body changed" where no index proves the rename. The header of the same review already
said it truthfully, "4 changes to read in 1 file · 1 file not proven" (`Unproven::RenamesUnanswered`), but "body
changed" reads as a change of behaviour: the only change in that body is a renamed name whose proof is unanswered.

## What Changes

- `ReviewUnit::renamed` (`RenamedUses`: the renames, old and new, and `wait`, `checking` or `notProven`): of a unit
  `behaviour` whose every changed line is a run `Renamed` of a name that is not local, which the index has not proven,
  read from the marks the reading of the plan proves (`review_plan::renamed_uses`) and read again with the answer of
  the index. A rename the index confirmed, a run that is quiet, a local name and a body with any other change leave the
  unit as it was.
- The heading words it: "balance — only renamed sumOf → totalOf, not proven", "… checking" while the index is asked;
  the badge of the card of a name "Renamed, not proven" or "Renamed, checking".
- `brd review-plan`: the `renamed` of a unit, `"schema": 37`.
- Capability `branch/review-units`: the unit says so, the heading words it, `brd` prints it; `branch/review-plan`: the
  version 37.

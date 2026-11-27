# Proposal

## Why

Issue #281: the split of `crates/search/src/lib.rs` of accio into modules (`mod x; pub use x::*;` in the root,
`use super::*;` atop each module) read as removals beside new files. Of 62, 35 and 75 removals of its three commits
56, 35 and 71 said where they went, of the 167 of the range 137, and no place the code left was nothing to read. A
function and the module of its name were one pair, the lines of an `impl` belonged to no declaration, a block that
removed the code and added `mod x; pub use x::*;` belonged to the module, a test of `mod tests` was no place of a move,
two places of one code were no move, and 22 declarations made `pub(crate)` by the commit before the split were no
move of the range. The agent that reviewed it looked for the kind `moved`, the reorder inside one file.

## What Changes

- A module pairs only with a module: `fn files` and `mod files;` are a removal and an addition (units and API).
- A member of an `impl` of Rust moves with the code of the header of its `impl`; the `impl` blocks are read of the
  tokens, and the lines of one whose members all go, of a module whose members all go and of the members of a removed
  type go with them.
- A block that adds the skeleton of a move (`mod x;` of a file of the change, `pub use x::*;`, imports) where it removes
  the moved code is the move alone, held by its unit.
- The declarations of a new or a deleted file that stand in modules alone are places of a move; a deleted file holds
  a module of no code of its own by its members.
- Two places of one code pair by what they are members of; the places left pair by their code but the modifiers of
  their reach and say so (`visibility`), "moved from scope.rs, visibility changed".
- A line of no declaration that is an item of its own (a call of a macro) goes with the move where a new file holds it.
- The skill tells `moved` (a reorder inside a file) from `moved_to`/`moved_from` (a move between files). `"schema": 44`.

A new file of moved code alone stays read once, at its new place: the decision of the user. The API of a re-export
(`pub use x::*`) is another change.

Capabilities: `branch/review-units`, `branch/review-plan`, `branch/review-concerns`, `review/range-reading`,
`cli/agent-cli`.

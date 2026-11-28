# Proposal

## Why

A Rust file split into modules that the crate root re-exports (`mod scope; pub use scope::*;`) keeps every path of its
API, yet the API surface of a review listed each moved public declaration as removed from the root and added to its
module, and the verdict said `breaks` (develar/branch-deck#281): the key of a Rust declaration is the module path of its
file, and the plan did not follow the re-export. A glob `pub use` had no reach in the summary, so a `pub(crate) use x::*`
could not be told from one that passes the names on to other crates.

## What Changes

- psi-syntax: `Import::exported` is the reach a file passes an imported name on at (`Option<Reach>`), a glob `use` too.
- `trial_check::…::declaration_checks::changed_declarations` keys a public Rust declaration at the path the crate passes
  it on at (`RustReexports`): through each `pub use` of its module, whole (`pub use x::*`, `self::`, `crate::`) or by
  its name (an alias gives the alias), read from the files the commits change, carried from commit to commit of the
  range. A move behind such a re-export keeps the forms: no entry, no use breaks. A `pub(crate) use` passes nothing on.

Capabilities: `branch/review-plan`.

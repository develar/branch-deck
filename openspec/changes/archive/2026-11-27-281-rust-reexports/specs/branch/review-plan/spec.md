## ADDED Requirements

### Requirement: API Surface: A Rust declaration is seen at the path its crate passes it on at

A public Rust declaration MUST be keyed at the path its crate passes it on at: through each `pub use` of its module,
whole (`pub use x::*`, `self::`, `crate::`) or by its name (an alias gives the alias), as far as such re-exports go. A
declaration moved into a module that the crate root re-exports MUST keep its forms: no entry, no use left outside
breaks. A `pub(crate) use` passes nothing on to another crate.

#### Scenario: A move into a module the root re-exports
- **WHEN** a commit moves `pub fn open` from `src/lib.rs` into `src/files.rs`, which the root declares with
  `mod files; pub use files::*;`, `pub use files::open;`, `pub use self::files::*;` or `pub use crate::files::open;`
- **THEN** the `api` has no entry and the verdict is `noUseBreaks`
- **AND** under `pub use files::open as start;` it is `.open` removed and `.start` added, and under `pub(crate) use
  files::*;` or without a `use` `.open` removed and `files.open` added
  [@test] crates/review-plan/src/api_test.rs (`a_rust_declaration_moved_into_a_module_the_root_re_exports_keeps_its_path`)

### Requirement: API Surface: The re-exports are those of the files the commits change

The re-exports MUST be read from the files the commits change, and those of a commit MUST hold for the commits after
it in the range that leave its file as it is. A re-export in a file no commit of the range changes is not known.

#### Scenario: A later commit of the range
- **WHEN** after the move a commit of the range changes only `src/files.rs`, the body of `open`, then its header
- **THEN** the body is no entry, and the header one change of `open` in `src/files.rs`
  [@test] crates/review-plan/src/api_test.rs (`a_re_export_holds_for_the_later_commits_of_the_range`)

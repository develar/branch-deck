//! Which archived virtual branches the baseline integrated, and the archive step of a sync
//! (docs/design/integration-detection.md): a branch that leaves the grouping moves into `<prefix>/archived/<date>/`,
//! is detected as merged, rebased or squashed, and is deleted once integrated and older than the retention period. The
//! results are notes on the branch tips, so an unchanged branch runs no detection again. Below sync-core, which runs it
//! beside the builds of a sync.
//!
//! - `detector`: the archive step and the detection of the archived branches, by the cached result of each tip
//! - `archive`: the archive and the delete of branches, free names under the archive namespace
//! - `common`: the listing of the branches under the prefix with their detection notes
//! - `cache`: the detection result in a note on the tip (`NOTES_REF`), and the stack base notes (`STACK_NOTES_REF`)
//! - `rebase`: rebased and cherry-picked commits by one cherry-mark walk, and the recheck of a `NotIntegrated` result
//! - `merge`: a branch merged into the baseline, and the commits its merge brought in
//! - `squash`: a squash merge, by a merge of the tip into the baseline that keeps the baseline's tree
//! - `strategy`: which of the three a detection runs, by the environment

pub mod archive;
pub mod cache;
pub mod common;
pub mod detector;
pub mod merge;
pub mod rebase;
pub mod squash;
pub mod strategy;

// No re-exports - import modules directly

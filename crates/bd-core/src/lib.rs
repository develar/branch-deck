//! What every `bd` command builds on, below the commands, so that an edit of one command doesn't rebuild the others:
//! - `context`: the environment of a run and the repository it resolves (`RepoContext`), the caches and the config
//! - `error`: `CliError`, its `ErrorCode` and exit code, and the JSON every failed command prints
//! - `output`: the schema version, short ids, the JSON writer
//! - `text_input`: a text argument or stdin
//! - `version`: the version of Branch Deck (`build.rs`) and `bd version`
//! - `bd_command`: how to run the bd of this binary, and a word quoted for the user's shell
//! - `sync_command`: `bd sync`, the repository lock, and the summaries of a sync the other commands print
//! - `stack_summary`: the whole stack of a branch as the outputs print it
//! - `branch_view`: the grouping of the commits ahead of the baseline, the placement and the stack problems
//! - `review_summary`: the code-owner review of a branch as the outputs print it
//! - `suggest_plan`: the plan of `bd suggest`, read by `bd check --plan` and `bd pick`

pub mod bd_command;
pub mod branch_view;
pub mod context;
pub mod error;
pub mod output;
pub mod review_summary;
pub mod stack_summary;
pub mod suggest_plan;
pub mod sync_command;
pub mod text_input;
pub mod version;

#[cfg(test)]
mod text_input_test;

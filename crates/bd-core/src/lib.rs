//! What every `bd` command builds on, below the commands, so that an edit of one command doesn't rebuild the others:
//! - `context`: the environment of a run and the repository it resolves (`RepoContext`), the caches and the config
//! - `error`: `CliError` with its `ErrorCode` (`domain_types::error`), and the JSON every failed command prints
//! - `output`: the schema version, short ids, the JSON writer
//! - `text_input`: a text argument or stdin
//! - `version`: the version of Branch Deck (`build.rs`) and `bd version`
//! - `bd_command`: how to run the bd of this binary, and a word quoted for the user's shell
//! - `sync_command`: `bd sync`, the repository lock, and the summaries of a sync the other commands print
//! - `stack_summary`: the whole stack of a branch as the outputs print it
//! - `branch_view`: the grouping of the commits ahead of the baseline, the placement and the stack problems
//! - `check_summary`: the summaries of a check that `bd check` and `bd safe-push` print, the dangling references and
//!   the refusal of a start they fail
//! - `services`: the remote services of a command (`patronus_service`, `github_service`) and the host of the reads of
//!   the facts over them (`BdServices`, for `remote_facts::service_sources::ServiceFactSources`)
//! - `review_summary`: the code-owner review of a branch as the outputs print it
//! - `next_step_summary`: the next step of a branch or a whole stack as the outputs print it
//! - `operation_record`: the record of a run in the operations registry of its repository and its entry in the journal:
//!   of an operation of the remote once its plan holds, of a rewrite, an update and a pick before any lock
//! - `suggest_plan`: the plan of `bd suggest`, read by `bd check --plan` and `bd pick`

pub mod bd_command;
pub mod branch_view;
pub mod check_summary;
pub mod context;
pub mod error;
pub mod next_step_summary;
pub mod operation_record;
pub mod output;
pub mod review_summary;
pub mod services;
pub mod stack_summary;
pub mod suggest_plan;
pub mod sync_command;
pub mod text_input;
pub mod version;

#[cfg(test)]
mod check_summary_test;
#[cfg(test)]
mod context_test;
#[cfg(test)]
mod text_input_test;

//! `brd need`: declare that a branch lands only after the branches it names (`refs/branch-deck/needs`), so that a
//! Safe Push, a dry run and a land of it refuse to start before them (openspec/specs/branch/declared-needs/spec.md); recorded in
//! the operations registry of the repository before the lock, as every rewrite of `bd` (`bd_core::operation_record`),
//! the lock taken by the core after it read the store

use crate::store_command::store_write;
use bd_core::context::RepoContext;
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use domain_types::rewrite::RewriteIntent;
use domain_types::rewrite::TableWitness;
use history_rewrite::needs::{NeedError, NeedOutcome, need_core, unneed_core};
use history_rewrite::repository_lock::RewriteOptions;
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, clap::Args)]
pub struct NeedArgs {
  /// The branch (group name) that lands last
  pub branch: String,
  /// What lands before it: a branch of this repository, or `<companion>:<branch>`
  #[arg(required = true, num_args = 1..)]
  pub needs: Vec<String>,
  /// Remove the named needs instead of declaring them
  #[arg(long)]
  pub remove: bool,
}

/// `brd need`
#[derive(Debug, Serialize, JsonSchema)]
pub struct NeedOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub branch: String,
  /// What lands before the branch afterwards, sorted
  pub needs: Vec<String>,
  /// What landed before it until now, sorted
  pub previous_needs: Vec<String>,
  /// Whether this call wrote the store
  pub changed: bool,
}

/// `brd need`, and with `--remove` the removal of the named needs: every name in one write; one that is not declared
/// refuses them all
pub fn need(context: &RepoContext, args: &NeedArgs) -> Result<NeedOutput, CliError> {
  let (branch_name, needs) = (args.branch.clone(), args.needs.clone());
  let intent = |witness| {
    if args.remove {
      RewriteIntent::Unneed { branch_name, needs, witness }
    } else {
      RewriteIntent::Need { branch_name, needs, witness }
    }
  };
  let needs: Vec<&str> = args.needs.iter().map(String::as_str).collect();
  let core = if args.remove { unneed_core } else { need_core };
  locked(context, intent, |options| core(&context.git, &context.repository, &args.branch, &needs, options))
}

/// `write`, the rewrite `intent`, recorded (`store_write`), and its output
fn locked(
  context: &RepoContext,
  intent: impl FnOnce(TableWitness) -> RewriteIntent,
  write: impl FnOnce(&RewriteOptions) -> Result<NeedOutcome, NeedError>,
) -> Result<NeedOutput, CliError> {
  let outcome = store_write(context, intent, write)?;
  Ok(NeedOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    changed: outcome.needs != outcome.previous_needs,
    branch: outcome.branch,
    needs: outcome.needs,
    previous_needs: outcome.previous_needs,
  })
}

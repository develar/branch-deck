//! `bd operation forget <op>` (spec/cli/agent-cli.spec.md, "Operations of other processes"): a start whose answer was
//! lost, which holds what it held in the operations registry of the repository until a read of Patronus tells whether
//! its robot started (`operations::reconcile`), leaves the registry, so the subject can start again; Patronus may still
//! run the robot it sent. The id is the one `busy` names (`error.holder.op`). The same core as Forget This Start of the
//! app (`operations::forget::forget_start`)

use bd_core::context::RepoContext;
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use domain_types::operation::OperationId;
use operations::forget::forget_start;
use operations::registry_file::ProcessRegistry;
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, clap::Args)]
pub struct OperationArgs {
  #[command(subcommand)]
  pub command: OperationCommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum OperationCommand {
  /// Forget a start whose answer was lost: its record leaves the operations registry, and what it held is free; only
  /// when the user asks. Patronus may still run the robot it sent: see `bd robot`
  Forget(ForgetArgs),
}

#[derive(Debug, clap::Args)]
pub struct ForgetArgs {
  /// The id of the operation, as `busy` names it (`error.holder.op`)
  pub id: String,
}

/// `bd operation forget`
#[derive(Debug, Serialize, JsonSchema)]
pub struct OperationForgetOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  /// The operation forgotten
  pub forgotten: String,
  /// The branch it started, `stack:<root>` a whole stack
  #[serde(skip_serializing_if = "Option::is_none")]
  pub subject: Option<String>,
  /// The ref its start was sent on, as Patronus would list its robot
  #[serde(skip_serializing_if = "Option::is_none")]
  pub source_ref: Option<String>,
}

/// Forgets the start `args.id` of the repository of `context`
///
/// # Errors
/// `invalid_arguments` for an id the registry lacks or of an operation whose process still runs it; `locked` when
/// another process keeps the lock of the registry
pub fn forget(context: &RepoContext, args: &ForgetArgs) -> Result<OperationForgetOutput, CliError> {
  let forgotten = forget_start(&ProcessRegistry::of(&context.git_common_dir), &OperationId(args.id.clone()))?;
  Ok(OperationForgetOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    forgotten: forgotten.id.0.clone(),
    subject: forgotten.intent.subject(),
    source_ref: forgotten.pending_start().map(|start| start.source_ref.clone()),
  })
}

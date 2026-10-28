use crate::install::InstallOutput;
use bd_core::error::CliError;
use bd_core::error::ErrorOutput;
use bd_core::output::SchemaVersion;
use bd_core::sync_command::SyncOutput;
use bd_core::version::VersionOutput;
use bd_lease::host_client_command::{HostApplyOutput, HostPackOutput, HostSnapshotOutput};
use bd_lease::host_command::{
  LeaseHostChangesOutput, LeaseHostCheckoutOutput, LeaseHostFillOutput, LeaseHostGitOutput, LeaseHostGiveBackOutput, LeaseHostHaveOutput, LeaseHostIngestOutput,
  LeaseHostLendOutput, LeaseHostLimitOutput, LeaseHostReclaimOutput, LeaseHostShowOutput, LeaseHostShrinkOutput, LeaseHostSnapshotOutput, LeaseHostStatusOutput,
};
use bd_lease::lease_command::{LeaseAdoptOutput, LeaseCleanOutput, LeaseGetOutput, LeaseReturnOutput, LeaseStatusOutput};
use bd_lease::workspace_command::WorkspaceOutput;
use bd_remote::land_command::LandOutput;
use bd_remote::operation_command::OperationForgetOutput;
use bd_remote::push_command::PushOutput;
use bd_remote::request_review_command::RequestReviewOutput;
use bd_remote::robot_command::RobotOutput;
use bd_remote::robot_stats_command::RobotStatsOutput;
use bd_remote::safe_push_command::SafePushOutput;
use bd_remote::status::{StatusBrief, StatusOutput};
use bd_rewrite::check_command::CheckOutput;
use bd_rewrite::commits::{CommitsBrief, CommitsOutput};
use bd_rewrite::describe::DescribeOutput;
use bd_rewrite::fold_command::FoldOutput;
use bd_rewrite::move_hunk_command::MoveHunkOutput;
use bd_rewrite::park_command::ParkOutput;
use bd_rewrite::rewrite::RewriteOutput;
use bd_rewrite::suggest::SuggestOutput;
use bd_update::pick_command::{LeasePickOutput, PickOutput};
use bd_update::unapply_command::{ReapplyOutput, UnapplyOutput};
use bd_update::undo_command::UndoOutput;
use bd_update::update_command::{UpdateAbortOutput, UpdateOutput, UpdateStatusOutput};
use domain_types::error::ErrorCode;
use schemars::{Schema, schema_for};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// The outputs of each command, by their names in `bd schema --all`: `rewrite` is the output of group, rename, restack,
/// unstack and reword, `commits_brief` and `status_brief` the outputs of `--brief`, `update_status` and `update_abort`
/// those of `bd update --status` and `--abort`, `lease_get` … `lease_clean` those of `bd lease <command>`,
/// `lease_host_lend` … `lease_host_status` those of `bd lease host <command>`. `error` is
/// what any command prints when it fails
pub const COMMAND_OUTPUTS: &[(&str, &[&str])] = &[
  ("check", &["check"]),
  ("commits", &["commits", "commits_brief"]),
  ("describe", &["describe"]),
  ("fold", &["fold"]),
  ("group", &["rewrite"]),
  ("host", &["host_snapshot", "host_pack", "host_apply"]),
  ("install", &["install"]),
  (
    "lease",
    &[
      "lease_get",
      "lease_pick",
      "lease_return",
      "lease_status",
      "lease_adopt",
      "lease_clean",
      "lease_host_lend",
      "lease_host_checkout",
      "lease_host_give_back",
      "lease_host_fill",
      "lease_host_reclaim",
      "lease_host_shrink",
      "lease_host_limit",
      "lease_host_status",
      "lease_host_have",
      "lease_host_ingest",
      "lease_host_show",
      "lease_host_git",
      "lease_host_snapshot",
      "lease_host_changes",
    ],
  ),
  ("move-hunk", &["move_hunk"]),
  ("operation", &["operation_forget"]),
  ("land", &["land"]),
  ("park", &["park"]),
  ("pick", &["pick"]),
  ("push", &["push"]),
  ("reapply", &["reapply"]),
  ("rename", &["rewrite"]),
  ("request-review", &["request_review"]),
  ("restack", &["rewrite"]),
  ("reword", &["rewrite"]),
  ("robot", &["robot"]),
  ("robot-stats", &["robot_stats"]),
  ("safe-push", &["safe_push"]),
  ("status", &["status", "status_brief"]),
  ("suggest", &["suggest"]),
  ("sync", &["sync"]),
  ("unapply", &["unapply"]),
  ("undo", &["undo"]),
  ("unpark", &["park"]),
  ("unstack", &["rewrite"]),
  ("update", &["update", "update_status", "update_abort"]),
  ("version", &["version"]),
  ("workspace", &["workspace"]),
];

/// The commands without a JSON output of their own: `bd schema` prints these schemas or the index, `bd help` text
pub const WITHOUT_OUTPUT: &[&str] = &["help", "schema"];

#[derive(Debug, clap::Args)]
pub struct SchemaArgs {
  /// Commands (`status`, `safe-push`) or outputs (`rewrite`, `status_brief`, `error`) to print the schema of; none: the
  /// index of the commands and their outputs
  pub names: Vec<String>,
  /// The schema of every output, about 350 KB
  #[arg(long, conflicts_with = "names")]
  pub all: bool,
}

/// `bd schema`: the index of the commands and the names of their outputs, a few KB, rather than every schema
#[derive(Debug, Serialize)]
pub struct SchemaIndex {
  pub schema: SchemaVersion,
  /// Per command the outputs it prints; `bd schema <command>` prints their schemas, `bd schema error` that of the
  /// error any command prints
  pub commands: BTreeMap<&'static str, &'static [&'static str]>,
}

/// `bd schema <name>...` and `bd schema --all`: the JSON Schema of the outputs asked for, by output name
#[derive(Debug, Serialize)]
pub struct SchemaOutput {
  pub schema: SchemaVersion,
  pub outputs: BTreeMap<String, Schema>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum SchemaAnswer {
  Index(SchemaIndex),
  Outputs(SchemaOutput),
}

pub fn schema(args: &SchemaArgs) -> Result<SchemaAnswer, CliError> {
  if !args.all && args.names.is_empty() {
    return Ok(SchemaAnswer::Index(SchemaIndex {
      schema: SchemaVersion,
      commands: COMMAND_OUTPUTS.iter().copied().collect(),
    }));
  }
  let mut outputs = every_output();
  if !args.all {
    let mut wanted = Vec::new();
    for name in &args.names {
      let keys = COMMAND_OUTPUTS
        .iter()
        .find(|(command, _)| command == name)
        .map(|(_, keys)| keys.to_vec())
        .or_else(|| outputs.contains_key(name.as_str()).then(|| vec![name.as_str()]))
        .ok_or_else(|| {
          let commands: Vec<&str> = COMMAND_OUTPUTS.iter().map(|(command, _)| *command).collect();
          let names: Vec<&str> = outputs.keys().map(String::as_str).collect();
          CliError::new(
            ErrorCode::InvalidArguments,
            format!("No command or output {name:?}. Commands: {}. Outputs: {}", commands.join(", "), names.join(", ")),
          )
        })?;
      wanted.extend(keys.into_iter().map(str::to_string));
    }
    outputs.retain(|key, _| wanted.contains(key));
  }
  Ok(SchemaAnswer::Outputs(SchemaOutput { schema: SchemaVersion, outputs }))
}

/// The JSON Schema of every output, by output name, each with the `operation_id` that `bd` prints after its fields
pub fn every_output() -> BTreeMap<String, Schema> {
  let outputs = BTreeMap::from([
    ("check".to_string(), schema_for!(CheckOutput)),
    ("commits".to_string(), schema_for!(CommitsOutput)),
    ("commits_brief".to_string(), schema_for!(CommitsBrief)),
    ("describe".to_string(), schema_for!(DescribeOutput)),
    ("error".to_string(), schema_for!(ErrorOutput)),
    ("fold".to_string(), schema_for!(FoldOutput)),
    ("host_apply".to_string(), schema_for!(HostApplyOutput)),
    ("host_pack".to_string(), schema_for!(HostPackOutput)),
    ("host_snapshot".to_string(), schema_for!(HostSnapshotOutput)),
    ("install".to_string(), schema_for!(InstallOutput)),
    ("lease_adopt".to_string(), schema_for!(LeaseAdoptOutput)),
    ("lease_clean".to_string(), schema_for!(LeaseCleanOutput)),
    ("lease_get".to_string(), schema_for!(LeaseGetOutput)),
    ("lease_host_checkout".to_string(), schema_for!(LeaseHostCheckoutOutput)),
    ("lease_host_fill".to_string(), schema_for!(LeaseHostFillOutput)),
    ("lease_host_give_back".to_string(), schema_for!(LeaseHostGiveBackOutput)),
    ("lease_host_lend".to_string(), schema_for!(LeaseHostLendOutput)),
    ("lease_host_limit".to_string(), schema_for!(LeaseHostLimitOutput)),
    ("lease_host_reclaim".to_string(), schema_for!(LeaseHostReclaimOutput)),
    ("lease_host_shrink".to_string(), schema_for!(LeaseHostShrinkOutput)),
    ("lease_host_status".to_string(), schema_for!(LeaseHostStatusOutput)),
    ("lease_host_have".to_string(), schema_for!(LeaseHostHaveOutput)),
    ("lease_host_ingest".to_string(), schema_for!(LeaseHostIngestOutput)),
    ("lease_host_show".to_string(), schema_for!(LeaseHostShowOutput)),
    ("lease_host_git".to_string(), schema_for!(LeaseHostGitOutput)),
    ("lease_host_snapshot".to_string(), schema_for!(LeaseHostSnapshotOutput)),
    ("lease_host_changes".to_string(), schema_for!(LeaseHostChangesOutput)),
    ("lease_pick".to_string(), schema_for!(LeasePickOutput)),
    ("lease_return".to_string(), schema_for!(LeaseReturnOutput)),
    ("lease_status".to_string(), schema_for!(LeaseStatusOutput)),
    ("move_hunk".to_string(), schema_for!(MoveHunkOutput)),
    ("land".to_string(), schema_for!(LandOutput)),
    ("park".to_string(), schema_for!(ParkOutput)),
    ("operation_forget".to_string(), schema_for!(OperationForgetOutput)),
    ("pick".to_string(), schema_for!(PickOutput)),
    ("push".to_string(), schema_for!(PushOutput)),
    ("reapply".to_string(), schema_for!(ReapplyOutput)),
    ("rewrite".to_string(), schema_for!(RewriteOutput)),
    ("robot_stats".to_string(), schema_for!(RobotStatsOutput)),
    ("request_review".to_string(), schema_for!(RequestReviewOutput)),
    ("robot".to_string(), schema_for!(RobotOutput)),
    ("safe_push".to_string(), schema_for!(SafePushOutput)),
    ("status".to_string(), schema_for!(StatusOutput)),
    ("status_brief".to_string(), schema_for!(StatusBrief)),
    ("suggest".to_string(), schema_for!(SuggestOutput)),
    ("sync".to_string(), schema_for!(SyncOutput)),
    ("unapply".to_string(), schema_for!(UnapplyOutput)),
    ("undo".to_string(), schema_for!(UndoOutput)),
    ("update".to_string(), schema_for!(UpdateOutput)),
    ("update_abort".to_string(), schema_for!(UpdateAbortOutput)),
    ("update_status".to_string(), schema_for!(UpdateStatusOutput)),
    ("version".to_string(), schema_for!(VersionOutput)),
    ("workspace".to_string(), schema_for!(WorkspaceOutput)),
  ]);
  outputs.into_iter().map(|(name, schema)| (name, with_operation_id(schema))).collect()
}

/// `schema`, of an output object, with the `operation_id` property every output carries (`bd-cli::print`), required
fn with_operation_id(mut schema: Schema) -> Schema {
  if let Some(object) = schema.as_object_mut() {
    if let Some(Value::Object(properties)) = object.get_mut("properties") {
      properties.insert(
        "operation_id".to_string(),
        json!({
          "type": "string",
          "description": "The operation of the run (a ULID): the `operation_id` of its log lines (`bd -v`), of its record in the operations registry and of its entry in the journal of the operations of the repository"
        }),
      );
    }
    match object.get_mut("required") {
      Some(Value::Array(required)) => required.push(json!("operation_id")),
      _ => drop(object.insert("required".to_string(), json!(["operation_id"]))),
    }
  }
  schema
}

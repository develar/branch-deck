use crate::install::InstallOutput;
use bd_core::error::ErrorOutput;
use bd_core::output::SchemaVersion;
use bd_core::sync_command::SyncOutput;
use bd_core::version::VersionOutput;
use bd_lease::lease_command::{LeaseAdoptOutput, LeaseCleanOutput, LeaseGetOutput, LeaseReturnOutput, LeaseStatusOutput};
use bd_lease::workspace_command::WorkspaceOutput;
use bd_remote::push_command::PushOutput;
use bd_remote::request_review_command::RequestReviewOutput;
use bd_remote::robot_command::RobotOutput;
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
use bd_update::pick_command::PickOutput;
use bd_update::unapply_command::{ReapplyOutput, UnapplyOutput};
use bd_update::undo_command::UndoOutput;
use bd_update::update_command::{UpdateAbortOutput, UpdateOutput, UpdateStatusOutput};
use schemars::{Schema, schema_for};
use serde::Serialize;
use std::collections::BTreeMap;

/// `bd schema`: the JSON Schema of every output, by command; `rewrite` is the output of group, rename, restack,
/// unstack and reword, `commits_brief` and `status_brief` the outputs of `--brief`, `update_status` and `update_abort`
/// those of `bd update --status` and `--abort`, `lease_get`, `lease_return`, `lease_status`, `lease_adopt` and
/// `lease_clean` those of `bd lease`, and `error` is what any command prints when it fails
#[derive(Debug, Serialize)]
pub struct SchemaOutput {
  pub schema: SchemaVersion,
  pub outputs: BTreeMap<String, Schema>,
}

pub fn schema_output() -> SchemaOutput {
  SchemaOutput {
    schema: SchemaVersion,
    outputs: BTreeMap::from([
      ("check".to_string(), schema_for!(CheckOutput)),
      ("commits".to_string(), schema_for!(CommitsOutput)),
      ("commits_brief".to_string(), schema_for!(CommitsBrief)),
      ("describe".to_string(), schema_for!(DescribeOutput)),
      ("error".to_string(), schema_for!(ErrorOutput)),
      ("fold".to_string(), schema_for!(FoldOutput)),
      ("install".to_string(), schema_for!(InstallOutput)),
      ("lease_adopt".to_string(), schema_for!(LeaseAdoptOutput)),
      ("lease_clean".to_string(), schema_for!(LeaseCleanOutput)),
      ("lease_get".to_string(), schema_for!(LeaseGetOutput)),
      ("lease_return".to_string(), schema_for!(LeaseReturnOutput)),
      ("lease_status".to_string(), schema_for!(LeaseStatusOutput)),
      ("move_hunk".to_string(), schema_for!(MoveHunkOutput)),
      ("park".to_string(), schema_for!(ParkOutput)),
      ("pick".to_string(), schema_for!(PickOutput)),
      ("push".to_string(), schema_for!(PushOutput)),
      ("reapply".to_string(), schema_for!(ReapplyOutput)),
      ("rewrite".to_string(), schema_for!(RewriteOutput)),
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
    ]),
  }
}

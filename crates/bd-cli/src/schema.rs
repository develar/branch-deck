use crate::check_command::CheckOutput;
use crate::commits::{CommitsBrief, CommitsOutput};
use crate::describe::DescribeOutput;
use crate::error::ErrorOutput;
use crate::fold_command::FoldOutput;
use crate::install::InstallOutput;
use crate::output::SchemaVersion;
use crate::push_command::PushOutput;
use crate::rewrite::{RewriteOutput, UndoOutput};
use crate::safe_push_command::SafePushOutput;
use crate::status::{StatusBrief, StatusOutput};
use crate::suggest::SuggestOutput;
use crate::sync_command::SyncOutput;
use crate::update_command::{UpdateAbortOutput, UpdateOutput, UpdateStatusOutput};
use crate::version::VersionOutput;
use schemars::{Schema, schema_for};
use serde::Serialize;
use std::collections::BTreeMap;

/// `bd schema`: the JSON Schema of every output, by command; `rewrite` is the output of group, rename, restack and
/// unstack, `commits_brief` and `status_brief` the outputs of `--brief`, `update_status` and `update_abort` those of
/// `bd update --status` and `--abort`, and `error` is what any command prints when it fails
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
      ("push".to_string(), schema_for!(PushOutput)),
      ("rewrite".to_string(), schema_for!(RewriteOutput)),
      ("safe_push".to_string(), schema_for!(SafePushOutput)),
      ("status".to_string(), schema_for!(StatusOutput)),
      ("status_brief".to_string(), schema_for!(StatusBrief)),
      ("suggest".to_string(), schema_for!(SuggestOutput)),
      ("sync".to_string(), schema_for!(SyncOutput)),
      ("undo".to_string(), schema_for!(UndoOutput)),
      ("update".to_string(), schema_for!(UpdateOutput)),
      ("update_abort".to_string(), schema_for!(UpdateAbortOutput)),
      ("update_status".to_string(), schema_for!(UpdateStatusOutput)),
      ("version".to_string(), schema_for!(VersionOutput)),
    ]),
  }
}

use crate::commits::CommitsOutput;
use crate::describe::DescribeOutput;
use crate::error::ErrorOutput;
use crate::output::SchemaVersion;
use crate::rewrite::{RewriteOutput, UndoOutput};
use crate::status::StatusOutput;
use crate::sync_command::SyncOutput;
use schemars::{Schema, schema_for};
use serde::Serialize;
use std::collections::BTreeMap;

/// `bd schema`: the JSON Schema of every output, by command; `rewrite` is the output of group, rename, restack and
/// unstack, and `error` is what any command prints when it fails
#[derive(Debug, Serialize)]
pub struct SchemaOutput {
  pub schema: SchemaVersion,
  pub outputs: BTreeMap<String, Schema>,
}

pub fn schema_output() -> SchemaOutput {
  SchemaOutput {
    schema: SchemaVersion,
    outputs: BTreeMap::from([
      ("commits".to_string(), schema_for!(CommitsOutput)),
      ("describe".to_string(), schema_for!(DescribeOutput)),
      ("error".to_string(), schema_for!(ErrorOutput)),
      ("rewrite".to_string(), schema_for!(RewriteOutput)),
      ("status".to_string(), schema_for!(StatusOutput)),
      ("sync".to_string(), schema_for!(SyncOutput)),
      ("undo".to_string(), schema_for!(UndoOutput)),
    ]),
  }
}

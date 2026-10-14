use crate::commits::CommitsOutput;
use crate::error::ErrorOutput;
use crate::output::SchemaVersion;
use crate::status::StatusOutput;
use schemars::{Schema, schema_for};
use serde::Serialize;
use std::collections::BTreeMap;

/// `bd schema`: the JSON Schema of every output, by command; `error` is what any command prints when it fails
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
      ("error".to_string(), schema_for!(ErrorOutput)),
      ("status".to_string(), schema_for!(StatusOutput)),
    ]),
  }
}

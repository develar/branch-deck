use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Serialize, Serializer};
use std::borrow::Cow;
use std::io::Write;

/// The version of the output contract
pub const SCHEMA_VERSION: u32 = 1;

/// The `"schema": 1` field every output object starts with
#[derive(Debug, Clone, Copy, Default)]
pub struct SchemaVersion;

impl Serialize for SchemaVersion {
  fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_u32(SCHEMA_VERSION)
  }
}

impl JsonSchema for SchemaVersion {
  fn inline_schema() -> bool {
    true
  }

  fn schema_name() -> Cow<'static, str> {
    "SchemaVersion".into()
  }

  fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "integer", "const": SCHEMA_VERSION })
  }
}

/// Write `value` as one line of JSON
pub fn write_json<T: Serialize>(out: &mut dyn Write, value: &T) -> std::io::Result<()> {
  serde_json::to_writer(&mut *out, value)?;
  out.write_all(b"\n")
}

/// Abbreviates commit ids to one length for a whole output: git's abbreviation length for the repository, longer when
/// two ids of the output share that prefix
pub struct ShortIds {
  len: Option<usize>,
}

impl ShortIds {
  /// Full ids
  pub fn full() -> Self {
    Self { len: None }
  }

  /// `min_len` is the length of `git rev-parse --short`; `ids` are all the ids the output names
  pub fn new<'a>(min_len: usize, ids: impl IntoIterator<Item = &'a str>) -> Self {
    let mut sorted: Vec<&str> = ids.into_iter().collect();
    sorted.sort_unstable();
    sorted.dedup();
    let shared = sorted
      .windows(2)
      .map(|pair| pair[0].bytes().zip(pair[1].bytes()).take_while(|(a, b)| a == b).count())
      .max()
      .unwrap_or(0);
    Self {
      len: Some(min_len.max(shared + 1)),
    }
  }

  pub fn get<'a>(&self, id: &'a str) -> &'a str {
    match self.len {
      Some(len) if len < id.len() => &id[..len],
      _ => id,
    }
  }
}

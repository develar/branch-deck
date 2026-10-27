use domain_types::commit::Commit;
use domain_types::conflict::WordResolutionSummary;
use git_ops::revs::short_id;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Serialize, Serializer};
use std::borrow::Cow;
use std::io::Write;
use sync_utils::operation_id::OperationId;

/// The version of the output contract
pub const SCHEMA_VERSION: u32 = 2;

/// The `"schema": 2` field every output object starts with
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

/// An output object as `bd` prints it: its own fields, then the `operation_id` of the run that printed it, the id of the
/// root span of its log lines and of its entry in the journal of the operations (`bd-cli::print`)
#[derive(Serialize)]
pub struct WithOperation<'a, T> {
  #[serde(flatten)]
  pub output: &'a T,
  pub operation_id: OperationId,
}

/// The conflict of git that the copy of a commit resolved by words (`branchdeck.resolveByWords`): the sync committed the
/// result of the engine where git stopped, and the branch builds
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ResolvedByWords {
  /// The conflicts of git that the words resolved
  pub conflicts: u32,
  pub file_count: u32,
  /// The resolved files with their conflicts, from the sync that resolved them; omitted for a copy a sync reused, whose
  /// mapping note keeps the counts only
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub files: Vec<ResolvedFileEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ResolvedFileEntry {
  pub path: String,
  pub conflicts: u32,
}

impl ResolvedByWords {
  /// What a sync tells of a copy
  pub fn of_summary(summary: &WordResolutionSummary) -> Self {
    Self {
      conflicts: summary.conflicts,
      file_count: summary.file_count,
      files: summary
        .files
        .iter()
        .map(|file| ResolvedFileEntry {
          path: file.path.clone(),
          conflicts: file.conflicts,
        })
        .collect(),
    }
  }

  /// What the mapping note of a commit records of its copy
  pub fn of_commit(commit: &Commit) -> Option<Self> {
    commit.mapped_resolution.map(|resolved| Self {
      conflicts: resolved.conflicts,
      file_count: resolved.files,
      files: Vec::new(),
    })
  }
}

/// Write `value` as one line of JSON, every non-ASCII character escaped as `\uXXXX` (a UTF-16 pair above the BMP): the
/// bytes are ASCII, so a reader that decodes stdout with a legacy code page (Windows PowerShell 5.1 uses the OEM one)
/// still gets every path and subject right
pub fn write_json<T: Serialize>(out: &mut dyn Write, value: &T) -> std::io::Result<()> {
  let mut serializer = serde_json::Serializer::with_formatter(&mut *out, AsciiFormatter);
  value.serialize(&mut serializer).map_err(std::io::Error::other)?;
  out.write_all(b"\n")
}

/// The compact formatter of `serde_json`, with `\uXXXX` for every non-ASCII character
struct AsciiFormatter;

impl serde_json::ser::Formatter for AsciiFormatter {
  fn write_string_fragment<W: ?Sized + Write>(&mut self, writer: &mut W, fragment: &str) -> std::io::Result<()> {
    let mut units = [0u16; 2];
    for c in fragment.chars() {
      if c.is_ascii() {
        writer.write_all(&[c as u8])?;
      } else {
        for unit in c.encode_utf16(&mut units) {
          write!(writer, "\\u{unit:04x}")?;
        }
      }
    }
    Ok(())
  }
}

/// Abbreviates commit ids as `short_id` does, longer for a whole output when two of its ids share that prefix
pub struct ShortIds {
  /// `None` keeps full ids
  min_len: Option<usize>,
}

impl ShortIds {
  /// Full ids
  pub const fn full() -> Self {
    Self { min_len: None }
  }

  /// `ids` are all the ids the output names
  pub fn new<'a>(ids: impl IntoIterator<Item = &'a str>) -> Self {
    let mut sorted: Vec<&str> = ids.into_iter().collect();
    sorted.sort_unstable();
    sorted.dedup();
    let shared = sorted
      .windows(2)
      .map(|pair| pair[0].bytes().zip(pair[1].bytes()).take_while(|(a, b)| a == b).count())
      .max()
      .unwrap_or(0);
    Self { min_len: Some(shared + 1) }
  }

  pub fn get<'a>(&self, id: &'a str) -> &'a str {
    match self.min_len {
      None => id,
      Some(min_len) => {
        let short = short_id(id);
        if short.len() >= min_len { short } else { id.get(..min_len).unwrap_or(id) }
      }
    }
  }
}

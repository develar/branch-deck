//! The end-of-line conversion git gives a file it adds (`convert.c`, `convert_attrs` and `crlf_to_git`), and its safer
//! autocrlf: a file whose blob in the index holds CRLF line ends, and is no binary, is added with its bytes as they are
//! when its conversion is `auto`, though `hash-object`, which reads no index, normalizes it. So a file that holds such a
//! blob as it is has no change, and an edit of it keeps its CRLF in what `git add` stages. `ident`, a `filter` driver
//! and a `working-tree-encoding` change the bytes besides the line ends, which this module does not model.

use crate::attributes::read_attributes;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use tracing::instrument;

/// The attributes that decide how git converts the bytes of a file it adds; `crlf` is the old name of `text`, which git
/// reads where `text` is unspecified
const CONVERSION_ATTRIBUTES: [&str; 6] = ["text", "crlf", "eol", "ident", "filter", "working-tree-encoding"];

/// What git does to the line ends of a file it adds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnds {
  /// Nothing: `-text` (`-crlf`), its bytes as they are
  AsTheyAre,
  /// `auto` (`text=auto`, `crlf=auto`, or neither `text`, `crlf` nor `eol`, which leaves it to `core.autocrlf`, auto or
  /// nothing): CRLF to LF, but for a file whose blob in the index holds CRLF (the safer autocrlf)
  Auto,
  /// CRLF to LF for every file: `text`, `text=input`, `crlf`, `crlf=input`, or an `eol` alone
  Converted,
}

/// How git converts the bytes of a file it adds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Conversion {
  pub line_ends: LineEnds,
  /// `ident`, a `filter` driver or a `working-tree-encoding` changes its bytes too
  pub other: bool,
}

impl Conversion {
  /// Whether git keeps the bytes of the file as they are when its blob in the index holds them: no conversion of the
  /// line ends but the safer autocrlf, and none besides them
  pub const fn keeps_a_crlf_blob(self) -> bool {
    !self.other && !matches!(self.line_ends, LineEnds::Converted)
  }
}

/// The conversion of each of `paths` (`convert_attrs` of git), by one `check-attr` of the attributes as git finds them
/// for the working tree; none without paths
#[instrument(skip(git, paths))]
pub fn conversions<'a>(git: &GitCommandExecutor, repository: &str, paths: impl IntoIterator<Item = &'a str>) -> Result<HashMap<String, Conversion>> {
  let mut values: HashMap<String, HashMap<String, String>> = HashMap::new();
  for attribute in read_attributes(git, repository, None, &CONVERSION_ATTRIBUTES, paths)? {
    values.entry(attribute.path).or_default().insert(attribute.attribute, attribute.value);
  }
  Ok(values.into_iter().map(|(path, values)| (path, conversion(&values))).collect())
}

/// The action of `text`, else of `crlf`, then of `eol` (`git_path_check_crlf`, `convert_attrs`)
fn conversion(values: &HashMap<String, String>) -> Conversion {
  let value = |name: &str| values.get(name).map_or("unspecified", String::as_str);
  let action = |value: &str| match value {
    "set" | "input" => Some(LineEnds::Converted),
    "unset" => Some(LineEnds::AsTheyAre),
    "auto" => Some(LineEnds::Auto),
    _ => None,
  };
  // `eol` sets no conversion of a binary file, keeps `auto` auto, and makes text of a file neither attribute decides;
  // without it `core.autocrlf` decides, auto or nothing, which keep a blob with CRLF alike
  let line_ends = action(value("text"))
    .or_else(|| action(value("crlf")))
    .unwrap_or_else(|| if matches!(value("eol"), "lf" | "crlf") { LineEnds::Converted } else { LineEnds::Auto });
  let off = |name: &str| matches!(value(name), "unspecified" | "unset");
  Conversion {
    line_ends,
    other: !(off("ident") && off("filter") && off("working-tree-encoding")),
  }
}

/// Whether git reads `content`, a blob of the index, as text with CRLF line ends (`has_crlf_in_index`): one CRLF at
/// least, and no binary by git's statistics (a NUL, a lone CR, or more than one non-printable byte in 128 printable
/// ones)
pub fn is_crlf_text(content: &[u8]) -> bool {
  let (mut crlf, mut lone_cr, mut nul, mut printable, mut non_printable) = (0_u64, 0_u64, 0_u64, 0_u64, 0_u64);
  let mut index = 0;
  while index < content.len() {
    match content[index] {
      b'\r' if content.get(index + 1) == Some(&b'\n') => {
        crlf += 1;
        index += 1;
      }
      b'\r' => lone_cr += 1,
      b'\n' => {}
      127 => non_printable += 1,
      // BS, HT, ESC and FF
      8 | 9 | 27 | 12 => printable += 1,
      0 => {
        nul += 1;
        non_printable += 1;
      }
      byte if byte < 32 => non_printable += 1,
      _ => printable += 1,
    }
    index += 1;
  }
  // an EOF (^Z) at the end is no non-printable byte
  if content.last() == Some(&26) {
    non_printable = non_printable.saturating_sub(1);
  }
  let binary = lone_cr > 0 || nul > 0 || (printable >> 7) < non_printable;
  crlf > 0 && !binary
}

#[cfg(test)]
#[path = "crlf_test.rs"]
mod crlf_test;

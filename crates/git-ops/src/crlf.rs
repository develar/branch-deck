//! The safer autocrlf of git (`convert.c`, `crlf_to_git`): a file whose blob in the index holds CRLF line ends, and is
//! no binary, is added with its bytes as they are when its end-of-line conversion is `auto`, though `hash-object`, which
//! reads no index, normalizes it. So a file that holds such a blob as it is has no change, and an edit of it keeps its
//! CRLF in what `git add` stages. Only where no other attribute changes the bytes: `ident`, a `filter` driver and a
//! `working-tree-encoding` do, and `text` or an `eol` without `text=auto` converts every file.

use crate::attributes::read_attributes;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{HashMap, HashSet};
use tracing::instrument;

/// The attributes that decide how git converts the bytes of a file it adds
const CONVERSION_ATTRIBUTES: [&str; 5] = ["text", "eol", "ident", "filter", "working-tree-encoding"];

/// The paths of `paths` whose bytes git keeps as they are when their blob holds CRLF (the module header): `text=auto`,
/// `-text`, or neither `text` nor `eol` (which leaves the conversion to `core.autocrlf`, `auto` or none), and no `ident`,
/// `filter` or `working-tree-encoding`. One `check-attr` of the attributes as git finds them for the working tree; none
/// without paths
#[instrument(skip(git, paths))]
pub fn crlf_kept_paths<'a>(git: &GitCommandExecutor, repository: &str, paths: impl IntoIterator<Item = &'a str>) -> Result<HashSet<String>> {
  let mut values: HashMap<String, HashMap<String, String>> = HashMap::new();
  for attribute in read_attributes(git, repository, None, &CONVERSION_ATTRIBUTES, paths)? {
    values.entry(attribute.path).or_default().insert(attribute.attribute, attribute.value);
  }
  Ok(
    values
      .into_iter()
      .filter(|(_, values)| {
        let value = |name: &str| values.get(name).map_or("unspecified", String::as_str);
        let off = |name: &str| matches!(value(name), "unspecified" | "unset");
        let kept_eol = match value("text") {
          "auto" | "unset" => true,
          "unspecified" => value("eol") == "unspecified",
          _ => false,
        };
        kept_eol && off("ident") && off("filter") && off("working-tree-encoding")
      })
      .map(|(path, _)| path)
      .collect(),
  )
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

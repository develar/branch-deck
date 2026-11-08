//! The texts of the UI a review changes (spec/branch/review-plan.spec.md, "UI Texts"): the keys the reviewed commits add
//! to, remove from or change in the message bundles, with their values before and after, which tell a reviewer what the
//! user of the change reads.
//!
//! - **A message bundle** is a `*.properties` file in a `messages` directory, or whose name without the extension ends
//!   with `Bundle` or `Messages` (`ThreadGuiBundle.properties`, `ErrorMessages.properties`), at its path before or after
//!   the commits: a resource bundle of Java or Kotlin. Another `*.properties` (`gradle.properties`, the keys of a
//!   registry) holds no text of the UI.
//! - **Its entries** are read as `java.util.Properties::load` reads them: one per logical line, a natural line that
//!   ends with an odd number of `\` going on in the next, whose leading whitespace is dropped; a line whose first
//!   character other than whitespace is `#` or `!` is a comment, a blank line nothing; the key runs up to the first `=`,
//!   `:` or whitespace not escaped, then whitespace, one `=` or `:` and whitespace again, and the value to the end of the
//!   logical line; `\t`, `\n`, `\r`, `\f` and `\uXXXX` (a surrogate pair too) are unescaped, another escaped character
//!   is itself. A key given twice keeps its last value.
//! - **A text** is a key the commits add (only after them), remove (only before) or change (its value differs), read
//!   before the first commit that changes the bundle and after the last: a comment, a blank line, a moved entry or a
//!   value broken over other lines is none. Its line is that of its entry after the commits (before them for a
//!   removal), from 1; its module that of its bundle (`Modules::of_file`). A side without its text (binary, too large)
//!   gives its bundle no texts, never every key removed.
//! - **The order**: the bundles in tree order, then by line, then by key.
//! - **The text** of a bundle before and after the commits comes with the text of the hunks (`bundle_blobs`): from the
//!   memory of the diffs, else in the batch of the summaries.

use crate::files::ChangedFile;
use crate::modules::Modules;
use serde::Serialize;
use std::collections::HashMap;

/// What the commits do to a text of the UI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum UiTextChange {
  /// A key only after the commits
  Added,
  /// A key only before the commits
  Removed,
  /// A key whose value differs
  Changed,
}

/// A text of the UI the commits add, remove or change: a key of a message bundle
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UiText {
  /// The bundle, at its last path
  pub path: String,
  pub key: String,
  pub change: UiTextChange,
  /// Its value before the commits, unescaped: of a removal and a change
  pub before: Option<String>,
  /// Its value after the commits, unescaped: of an addition and a change
  pub after: Option<String>,
  /// The line its entry starts on, from 1: after the commits, before them for a removal
  pub line: u32,
  /// The module of its bundle, by its place in `ReviewPlan::modules`
  pub module: Option<u32>,
}

/// The whitespace of a properties file
const WHITESPACE: [char; 3] = [' ', '\t', '\u{c}'];

/// Whether `path` is a message bundle: a `*.properties` file in a `messages` directory, or whose stem ends with `Bundle`
/// or `Messages`
fn is_message_bundle(path: &str) -> bool {
  let (directory, name) = path.rsplit_once('/').unwrap_or(("", path));
  let Some(stem) = name.strip_suffix(".properties") else { return false };
  stem.ends_with("Bundle") || stem.ends_with("Messages") || directory.split('/').any(|segment| segment == "messages")
}

/// A bundle at its path before or after the commits
fn is_bundle(file: &ChangedFile<'_>) -> bool {
  is_message_bundle(file.path) || is_message_bundle(file.first_path)
}

/// The blobs of every message bundle before the first commit that changes it and after the last
pub(crate) fn bundle_blobs(files: &[ChangedFile<'_>]) -> Vec<String> {
  files
    .iter()
    .filter(|file| is_bundle(file))
    .flat_map(|file| [file.before, file.after])
    .flatten()
    .map(str::to_string)
    .collect()
}

/// An entry of a properties text: its value, unescaped, and the line it starts on, from 1
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Entry {
  pub value: String,
  pub line: u32,
}

/// The entries of a properties text by key, as `java.util.Properties::load` reads them; a key given twice keeps its
/// last entry
pub(crate) fn entries(text: &str) -> HashMap<String, Entry> {
  let lines: Vec<&str> = text.split('\n').map(|line| line.strip_suffix('\r').unwrap_or(line)).collect();
  let mut entries = HashMap::new();
  let mut index = 0;
  while index < lines.len() {
    let start = index;
    let mut line = lines[index].trim_start_matches(WHITESPACE);
    index += 1;
    if line.is_empty() || line.starts_with(['#', '!']) {
      continue;
    }
    let mut logical = String::new();
    loop {
      let escapes = line.len() - line.trim_end_matches('\\').len();
      if escapes % 2 == 0 {
        logical.push_str(line);
        break;
      }
      // the `\` that joins the next natural line; the last line of the text joins none
      logical.push_str(&line[..line.len() - 1]);
      let Some(next) = lines.get(index) else { break };
      line = next.trim_start_matches(WHITESPACE);
      index += 1;
    }
    let (key, value) = key_and_value(&logical);
    let line = u32::try_from(start + 1).unwrap_or(u32::MAX);
    entries.insert(key, Entry { value, line });
  }
  entries
}

/// The key and the value of a logical line, unescaped: the key up to the first `=`, `:` or whitespace not escaped,
/// then whitespace, one `=` or `:` and whitespace again
fn key_and_value(logical: &str) -> (String, String) {
  let mut end = logical.len();
  let mut chars = logical.char_indices();
  while let Some((at, char)) = chars.next() {
    if char == '\\' {
      chars.next();
    } else if char == '=' || char == ':' || WHITESPACE.contains(&char) {
      end = at;
      break;
    }
  }
  let rest = logical[end..].trim_start_matches(WHITESPACE);
  let value = rest.strip_prefix(['=', ':']).map_or(rest, |value| value.trim_start_matches(WHITESPACE));
  (unescape(&logical[..end]), unescape(value))
}

/// The four hex digits `text` starts with, as a UTF-16 code unit
fn code_unit(text: &str) -> Option<u16> {
  let digits = text.get(..4)?;
  if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
    return None;
  }
  u16::from_str_radix(digits, 16).ok()
}

/// `text` with its escapes read: `\t`, `\n`, `\r`, `\f`, `\uXXXX` (a surrogate pair of two), another escaped character
/// itself; a `\u` without four hex digits stays as written
fn unescape(text: &str) -> String {
  let mut unescaped = String::with_capacity(text.len());
  let mut rest = text;
  while let Some(at) = rest.find('\\') {
    unescaped.push_str(&rest[..at]);
    rest = &rest[at + 1..];
    let Some(escaped) = rest.chars().next() else { break };
    rest = &rest[escaped.len_utf8()..];
    match escaped {
      't' => unescaped.push('\t'),
      'n' => unescaped.push('\n'),
      'r' => unescaped.push('\r'),
      'f' => unescaped.push('\u{c}'),
      'u' => {
        let Some(unit) = code_unit(rest) else {
          unescaped.push_str("\\u");
          continue;
        };
        rest = &rest[4..];
        let low = rest
          .strip_prefix("\\u")
          .and_then(code_unit)
          .filter(|low| (0xD800..0xDC00).contains(&unit) && (0xDC00..0xE000).contains(low));
        if low.is_some() {
          rest = &rest[6..];
        }
        let units = std::iter::once(unit).chain(low);
        unescaped.extend(char::decode_utf16(units).map(|char| char.unwrap_or(char::REPLACEMENT_CHARACTER)));
      }
      other => unescaped.push(other),
    }
  }
  unescaped.push_str(rest);
  unescaped
}

/// The entries of the side `blob` of a bundle: none where it is not, `None` without its text
fn side(blob: Option<&str>, texts: &HashMap<String, String>) -> Option<HashMap<String, Entry>> {
  blob.map_or_else(|| Some(HashMap::new()), |blob| texts.get(blob).map(|text| entries(text)))
}

/// The texts of the UI the commits add, remove or change in the message bundles of `files` (`changed_files`), by the
/// texts of their blobs (`bundle_blobs`) and their `modules`: the bundles in tree order, then by line and key
pub(crate) fn ui_texts(files: &[ChangedFile<'_>], texts: &HashMap<String, String>, modules: &Modules) -> Vec<UiText> {
  let mut found = Vec::new();
  for (index, file) in files.iter().enumerate().filter(|(_, file)| is_bundle(file)) {
    let (Some(before), Some(after)) = (side(file.before, texts), side(file.after, texts)) else {
      continue;
    };
    let text = |key: &str, change: UiTextChange, before: Option<&Entry>, after: Option<&Entry>, line: u32| UiText {
      path: file.path.to_string(),
      key: key.to_string(),
      change,
      before: before.map(|entry| entry.value.clone()),
      after: after.map(|entry| entry.value.clone()),
      line,
      module: modules.of_file.get(index).copied(),
    };
    let mut changed: Vec<UiText> = after
      .iter()
      .filter_map(|(key, entry)| match before.get(key) {
        None => Some(text(key, UiTextChange::Added, None, Some(entry), entry.line)),
        Some(old) if old.value != entry.value => Some(text(key, UiTextChange::Changed, Some(old), Some(entry), entry.line)),
        Some(_) => None,
      })
      .chain(
        before
          .iter()
          .filter(|(key, _)| !after.contains_key(*key))
          .map(|(key, entry)| text(key, UiTextChange::Removed, Some(entry), None, entry.line)),
      )
      .collect();
    changed.sort_by(|a, b| a.line.cmp(&b.line).then_with(|| a.key.cmp(&b.key)));
    found.extend(changed);
  }
  found
}

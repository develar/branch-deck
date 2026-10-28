//! The body of a commit message as blocks (`sync_utils::commit_body::body_blocks`): what the commit preview renders
//! (`commit_body_blocks`, `src/components/commit/CommitMessage.tsx`) and what the body of a pull request is written from
//! (`remote_facts::pull_request_text`), one grammar for both.

use serde::{Deserialize, Serialize};

/// A block of a commit body
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CommitBodyBlock {
  /// Prose, a line per line it reads as: hard-wrapped lines joined by a space; a line after a hard break (two spaces,
  /// `\`), a quote line and a table row each a line of their own, a heading and a rule a paragraph of their own
  Paragraph { lines: Vec<String> },
  /// A bullet (`-`, `*`, `+`, `•`) or numbered (`1.`, `1)`) list: the items without their markers, their wrapped and
  /// indented lines joined, nested items flattened
  List {
    ordered: bool,
    /// The number of the first item of an ordered list
    start: u32,
    items: Vec<String>,
  },
  /// Code or other text whose line breaks and spacing matter (fenced, or indented by four spaces or a tab), verbatim
  /// without the common indent and the fences
  Preformatted { text: String },
  /// The `Token: value` lines that end the message (`Co-Authored-By:`, `Signed-off-by:`, `Fixes:`…), an indented line
  /// joined to its trailer
  Trailers { lines: Vec<String> },
}

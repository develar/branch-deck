//! The body of a commit message as Rust reads it (`sync_utils::commit_body::commit_body`), the one grammar of a body:
//! what the page renders (`commit_body`, `src/components/commit/CommitBodyView.tsx`) and what the body of a
//! pull request is written from (`remote_facts::pull_request_text`).

use serde::{Deserialize, Serialize};

/// The body of a commit message, the lines after its subject line
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CommitBody {
  /// The blocks in the order of the body, without the trailers
  pub blocks: Vec<CommitBodyBlock>,
  /// The `Token: value` lines that end the message (`Co-Authored-By:`, `Signed-off-by:`, `Fixes:`…), an indented line
  /// joined to its trailer
  pub trailers: Vec<String>,
  /// Whether `blocks[0]` is a paragraph, before any label or heading: the summary of the body
  pub lead: bool,
  /// The parts of the body in its order, each block in one: the lead alone; a labelled section (a label or a heading
  /// and its blocks up to the next one; a run-in `Look closely: text` its paragraph alone); a run of the other blocks,
  /// two where the fold cuts it. Each says whether a
  /// reader folds it, so Rust alone says where the fold is
  pub parts: Vec<CommitBodyPart>,
}

/// A part of a commit body (`CommitBody::parts`): the blocks a reader shows or folds together
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CommitBodyPart {
  /// Its first block in `CommitBody::blocks`: its label or heading when it has one
  pub start: u32,
  /// The block after its last one
  pub end: u32,
  /// Whether a reader folds it: a part past the fold, which comes after the first part and the whole parts that read in
  /// about 12 lines with it, and a verification section wherever it is; never a Look closely section, wherever it is.
  /// Neither of the two costs a line
  pub folded: bool,
}

/// A block of a commit body
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CommitBodyBlock {
  /// Prose, a line per line it reads as: hard-wrapped lines joined by a space; a line after a hard break (two spaces,
  /// `\`), a quote line and a table row each a line of their own
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
  /// A Markdown heading (`## Text`): its level, one to six, and its text without the marks
  Heading { level: u8, text: String },
  /// The label of the blocks after it, up to the next label or heading (a run-in Look closely its paragraph alone): a
  /// paragraph that is only `Label:`, a `Label:` line right before a list, or right before a paragraph or code at the
  /// start of its block, or the 1–3 words that start a `Label: text` paragraph, whose text is then the paragraph after
  /// it. The text without its colon, as the author wrote it
  Label {
    text: String,
    kind: LabelKind,
    /// Whether the author wrote the label at the start of its paragraph (`Why: the text`), the paragraph after it the
    /// rest; false for a label on a line of its own
    #[serde(rename = "runIn")]
    run_in: bool,
  },
  /// A thematic break (`---`, `***`, `___`), a block of the section it is in: it ends no section
  Rule,
}

/// What a label says its section is, by its words: a leading "The", the case and what follows a comma aside, so "The
/// fix", "Fix" and "Fix, in two places" are one
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum LabelKind {
  /// Any other label: "The cause", "What changed", "Why"
  Plain,
  /// How the author checked the change: Tests, Evidence, Gates, Verified, Verification, Results, Proof, Left out
  /// (also "Test", "Gates on the final tree"); a review folds it always
  Verification,
  /// "Look closely": where the author asks the reviewer to decide
  LookClosely,
}

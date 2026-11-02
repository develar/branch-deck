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
  /// The code of each line of its blocks that holds some (`sync_utils::code_spans`): the page sets these spans in the
  /// monospace font, so a card, a preview and the story of a review set one name alike
  pub code: Vec<LineCode>,
}

/// The spans of code of one line of a block of a body: a line of a paragraph, an item of a list, a label or a heading
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct LineCode {
  /// Its block in `CommitBody::blocks`
  pub block: u32,
  /// The line of a paragraph, the item of a list; 0 for a heading or a label
  pub item: u32,
  pub spans: Vec<CodeSpan>,
}

/// A span of code in a line of prose (`sync_utils::code_spans`): UTF-16 offsets, the end excluded
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CodeSpan {
  pub start: u32,
  pub end: u32,
  /// The author wrote it in backticks, which the span holds: a chip; else the grammar found it, only the code font
  pub explicit: bool,
  /// The grammar joined tokens one space apart into it (`bt.cmd --lane`): a chip too, the words read as one command
  pub joined: bool,
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
  /// What it is: the lead, a section its first block heads, or a run of other blocks
  pub kind: CommitBodyPartKind,
  /// The kind of the label that heads a section: a Look closely is a callout; `Plain` for a heading, the lead and a run
  pub tone: LabelKind,
  /// Whether a reader folds it: a part past the fold, which comes after the first part and the whole parts that read in
  /// about 12 lines with it, and a verification section wherever it is; never a Look closely section, wherever it is.
  /// Neither of the two costs a line
  pub folded: bool,
}

/// What a part of a commit body is (`CommitBodyPart::kind`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum CommitBodyPartKind {
  /// The paragraph the body opens with, before any label or heading: its summary, alone
  Lead,
  /// A label or a heading (its first block) and its blocks up to the next one; a run-in Look closely its paragraph
  Section,
  /// A run of other blocks
  Run,
}

/// A block of a commit body
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

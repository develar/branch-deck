//! What a range leaves to read (openspec/specs/review/range-reading/spec.md): the facts `review_plan::reading` proves
//! from the marks of the diffs of every file of `base..head` (`RangeReading`), what `domain_policy::range_reading`
//! decides of them (`ReadingSummary`) and what `domain_policy::author_claim` reads of the author's word
//! (`AuthorClaim`, `ClaimConflict`). Facts and verdicts only; the page words them.

use serde::{Deserialize, Serialize};

/// What a changed file is to its module, by its path (`review_plan::reading::reading_role`): the order is the order of
/// `RangeReading::roles`
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum FileRole {
  Code,
  Tests,
  /// Markdown, `reStructuredText`, `AsciiDoc`, text: the docs
  Prose,
  /// A build file: a module file, a file of the build model, a manifest of dependencies
  Build,
  /// Any other file of no language
  Resources,
}

/// What is left to read of the changed files of one role
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct RoleReading {
  pub role: FileRole,
  /// Its changed files
  pub files: u32,
  /// Its files with a change to read
  pub files_to_read: u32,
  /// Its blocks of changed lines (`DiffReading::changes`)
  pub changes: u32,
  /// Those with a start, past the blocks of the imports alone (`DiffReading::stops(true)`): what the badge of each
  /// file of a review with its plan counts, so the sum here is the sum of those badges
  pub to_read: u32,
}

/// Why a file cannot be proven to need no reading: its count, if any, is all of its changes
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum Unproven {
  /// Binary on a side
  Binary,
  /// A side over the size the diffs read
  TooLarge,
  /// A submodule: commits, no text
  Submodule,
  /// Git cannot read a side (a partial or shallow clone)
  Missing,
  /// A text of code or resources with a change to read and no grammar: no run of formatting, comments or renames can
  /// be proven in it
  NoGrammar,
  /// A rename that is not local waits for the index of the repository (`RenameProof::Asked`): its places count until
  /// the answer, which may pass them
  RenamesPending,
  /// The index could not prove a rename that is not local (`RenameProof::Unavailable`, `Incomplete`): its places count
  RenamesUnanswered,
}

/// A changed file that cannot be proven, with why
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UnprovenFile {
  pub path: String,
  pub role: FileRole,
  pub why: Unproven,
}

/// A kind of proof that passed a change of the range: what made the rest quiet. The order is the order of
/// `RangeReading::proofs`
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum QuietProof {
  /// A quiet run of the kind `Formatting`: line breaks, indents, spaces between tokens
  Formatting,
  /// A quiet run of the kind `Comments`
  Comments,
  /// A quiet run of the kind `Renamed`, or a run the index confirmed
  Renames,
  /// A block of the imports alone, which a review with its plan passes
  Imports,
  /// Lines that moved inside their file without a change (`LineMove`): their old lines are passed
  Moves,
  /// A file that moved to another path with no changed line
  RenamedFiles,
  /// A file whose line ends alone changed
  LineEnds,
}

/// What a range leaves to read, proven from the full marks of the diffs of each of its files (the same `DiffReading` a
/// file of the review window counts), never guessed: `roles` sums the files by their role, `unproven` names the files
/// no proof can be made of, `proofs` the kinds of proof that passed a change. Computed under the default whitespace
/// policy, as the plan reads its pair
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct RangeReading {
  /// Of each role with a changed file, in the order of `FileRole`
  pub roles: Vec<RoleReading>,
  /// In the order of git
  pub unproven: Vec<UnprovenFile>,
  /// Each kind once, in the order of `QuietProof`
  pub proofs: Vec<QuietProof>,
}

impl RangeReading {
  /// A rename of the range still waits for the index (`Unproven::RenamesPending`): the reading of a plan sent before
  /// the index answered, never kept
  #[must_use]
  pub fn pending(&self) -> bool {
    self.unproven.iter().any(|file| file.why == Unproven::RenamesPending)
  }

  /// The changes to read over every role
  #[must_use]
  pub fn to_read(&self) -> u32 {
    self.roles.iter().map(|role| role.to_read).sum()
  }
}

/// What a range leaves to read, as `domain_policy::range_reading::reading_summary` decides it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ReadingVerdict {
  /// No change to read and no file that cannot be proven; `proofs` say why the changes need none
  #[serde(rename_all = "camelCase")]
  NothingToRead { proofs: Vec<QuietProof> },
  /// What is left to read, and every file that cannot be proven, is of tests
  #[serde(rename_all = "camelCase")]
  TestsOnly { to_read: u32, files: u32 },
  /// What is left to read, and every file that cannot be proven, is prose
  #[serde(rename_all = "camelCase")]
  DocsOnly { to_read: u32, files: u32 },
  /// `to_read` changes in `files` files are left, of several roles or of code
  #[serde(rename_all = "camelCase")]
  ToRead { to_read: u32, files: u32 },
}

/// The verdict of a reading with what qualifies it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReadingSummary {
  pub verdict: ReadingVerdict,
  /// The files that cannot be proven but those that wait for the index
  pub unproven: u32,
  /// A rename waits for the index: the verdict may pass more once it answers
  pub pending: bool,
}

impl ReadingSummary {
  /// Nothing to read: a reader may pass the range
  #[must_use]
  pub const fn nothing_to_read(&self) -> bool {
    matches!(self.verdict, ReadingVerdict::NothingToRead { .. })
  }
}

/// What the author says a pull request is, by a label or the conventional type of its title
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ClaimKind {
  Refactor,
  Tests,
  Docs,
}

/// Where the author said it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ClaimSource {
  /// A label of the pull request, by its name as GitHub has it
  Label { name: String },
  /// The conventional type its title starts with (`refactor(diff): …`)
  Title { prefix: String },
}

/// The author's word of a pull request, from a closed list (`domain_policy::author_claim::claim_of`)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AuthorClaim {
  pub kind: ClaimKind,
  pub source: ClaimSource,
}

/// The diff proves the author's word wrong: it leaves `to_read` changes in `files` files outside the role the word
/// names (`domain_policy::author_claim::check`)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ClaimConflict {
  pub claim: AuthorClaim,
  pub to_read: u32,
  pub files: u32,
}

/// What the window shows of the reading of a range: the facts, the verdict and the author's word checked against them
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RangeReadingView {
  pub reading: RangeReading,
  pub summary: ReadingSummary,
  /// The diff proves the author's word wrong; `None` when it holds, when the author said nothing of the closed list,
  /// and while a rename waits for the index
  pub conflict: Option<ClaimConflict>,
}

/// The readings of the layers of a stack, summed (`domain_policy::range_reading::stack_summary`)
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StackReading {
  /// The layers with nothing to read
  pub nothing_to_read: u32,
  /// The layers with something to read
  pub to_read_layers: u32,
  /// The changes those leave to read
  pub to_read: u32,
  /// The layers whose reading is not known yet
  pub unknown: u32,
  /// The first layer, the bottom first, with something to read: the one the window opens on; `None` while a layer
  /// below it is not known, and when every layer has nothing to read
  pub first_to_read: Option<u32>,
}

//! The types that the app reads (specta): what a diff compares, the diff of each file, and the texts of blobs.

use object_cache::commit_files::FileStatus;
use serde::{Deserialize, Serialize};
use text_diff::merge_model::TextMerge;
use text_diff::model::{Scope, TextDiff};
use text_diff::policy::WhitespacePolicy;

/// The parameters of `get_file_diffs`
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetFileDiffsParams {
  pub repository_path: String,
  pub source: DiffSource,
  pub policy: WhitespacePolicy,
  /// The files in view, whose diffs come first; then the others in the order of the tree
  pub priority_paths: Vec<String>,
}

/// The parameters of `get_blob_texts`
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetBlobTextsParams {
  pub repository_path: String,
  /// At most `MAX_BLOB_REQUESTS`
  pub blobs: Vec<BlobRequest>,
}

/// What a diff compares
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DiffSource {
  /// A commit with its first parent
  Commit { id: String },
  /// Two commits, as `git diff base head`
  Range { base: String, head: String },
  /// The working tree with `HEAD`: staged, unstaged and untracked files
  Uncommitted,
}

/// The diff of one changed file, as `FileDiffs::read` gives it at once (the tests and the command line; the app
/// streams `FileDiffEvent`s)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DiffFile {
  pub path: String,
  /// The path before a rename or copy
  pub old_path: Option<String>,
  pub status: FileStatus,
  /// The blob id of the old text; `None` for an added file and a submodule, whose commits are in its content
  pub old_id: Option<String>,
  /// The blob id of the new text; `None` for a deleted file and a submodule. A file of the working tree has the id
  /// that git would give its bytes (`git hash-object --no-filters`).
  pub new_id: Option<String>,
  pub content: DiffContent,
}

/// A changed file before its content: the first event of `get_file_diffs` lists them
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DiffFileHeader {
  pub path: String,
  /// The path before a rename or copy
  pub old_path: Option<String>,
  pub status: FileStatus,
  /// The blob id of the old text; `None` for an added file and a submodule, whose commits are in its content
  pub old_id: Option<String>,
  /// The blob id of the new text; `None` for a deleted file and a submodule. A file of the working tree has the id
  /// that git would give its bytes (`git hash-object --no-filters`).
  pub new_id: Option<String>,
}

/// What `get_file_diffs` sends through its channel: `files` once, then one `content` for each file, the files in view
/// first, and a `scopes` after the `content` of a text that the caches did not have
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FileDiffEvent {
  /// The changed files in the order of git; `index` of the other events points into this list
  Files { files: Vec<DiffFileHeader> },
  /// The content of a file. A `TextDiff` that the caches did not have comes with empty scopes, which a `scopes` event
  /// of the same file fills later: the parse costs more than the comparison
  Content { index: u32, content: DiffContent },
  /// The scopes of the text of a file, for the labels of its hunks, and its blocks of the imports alone
  #[serde(rename_all = "camelCase")]
  Scopes {
    index: u32,
    old_scopes: Vec<Scope>,
    new_scopes: Vec<Scope>,
    import_blocks: Vec<u32>,
  },
}

/// What the views show of a file
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DiffContent {
  Text(TextDiff),
  /// A side is binary: a NUL in its first 8000 bytes, or git counts no lines
  Binary,
  /// A side is over `MAX_BLOB_BYTES`; the sizes in bytes, 0 for a missing side
  #[serde(rename_all = "camelCase")]
  TooLarge {
    old_bytes: u32,
    new_bytes: u32,
  },
  /// A submodule: the commit it points to on each side, `None` for a side without it, as git writes
  /// `Subproject commit <id>`
  #[serde(rename_all = "camelCase")]
  Submodule {
    old_commit: Option<String>,
    new_commit: Option<String>,
  },
  /// Git cannot read a blob of the file, as in a partial or a shallow clone
  Missing,
}

/// The parameters of `get_blob_syntax`, the tree-sitter highlighting of one blob (`syntax.rs`)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetBlobSyntaxParams {
  pub repository_path: String,
  /// The blob, with the path of its file in the working tree when it is one, as `get_blob_texts` reads it
  pub blob: BlobRequest,
  /// The path whose extension tells the grammar: for the old side of a rename, the old path
  pub file_path: String,
  /// The lines to answer, `[from_line, to_line)`: the pages the view asks for
  pub from_line: u32,
  pub to_line: u32,
  /// The text of a blob the view made (the result of a merge), or of one the backend answered `Missing` for
  pub text: Option<String>,
}

/// The parameters of `find_declaration`: a name in a diff of a review and the files of the review (`declarations.rs`)
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FindDeclarationParams {
  pub repository_path: String,
  pub site: DeclarationSite,
  /// The changed files of the review, whose sides are searched
  pub files: Vec<ReviewFileSides>,
  /// The revisions of the sides, for a declaration outside the review; none for the working tree
  #[serde(default)]
  pub revisions: DiffRevisions,
}

/// The parameters of `get_outside_place`: a place outside the review at a revision, by its path and its line from 1, and
/// the name there to select (`outside_place.rs`)
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct OutsidePlaceParams {
  pub repository_path: String,
  pub revision: String,
  pub path: String,
  pub line: u32,
  pub name: Option<String>,
}

/// The commits a review compares: the old side (the parent, the base of a range) and the new one
#[derive(Debug, Clone, Default, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DiffRevisions {
  pub old: Option<String>,
  pub new: Option<String>,
}

/// A file outside the review that holds a declaration: shown by itself, at the revision of its side
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct OutsideFile {
  /// The revision it is read at; none for the working tree
  pub revision: Option<String>,
  pub blob_id: String,
  /// The lines of its text, as the view counts them
  pub line_count: u32,
}

/// Where the name is: a side of a file of the review, a line from 0 and a UTF-16 column
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DeclarationSite {
  /// The path of the side: the old path for the old side of a rename
  pub path: String,
  pub side: DiffSide,
  pub blob: BlobRequest,
  pub line: u32,
  pub column: u32,
}

/// A side of a diff
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum DiffSide {
  Old,
  New,
}

/// A changed file of a review with the blobs of its sides
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewFileSides {
  pub path: String,
  pub old_path: Option<String>,
  pub old_id: Option<String>,
  pub new_id: Option<String>,
  /// The new side is the file of the working tree
  pub worktree: bool,
}

/// The declarations a name may mean, the most likely first
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FoundDeclarations {
  pub name: String,
  pub targets: Vec<DeclarationTarget>,
  /// Why the index of the repository did not answer, when it was asked
  pub index_error: Option<String>,
}

/// A declaration in a side of a file of the review
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DeclarationTarget {
  pub path: String,
  pub side: DiffSide,
  /// The line of the declared name, from 0, and its UTF-16 columns
  pub line: u32,
  pub column: u32,
  pub end_column: u32,
  /// What is declared: `class`, `function`, `property`, `constant`, `enumEntry`, `typeAlias`, `module`, `macro`, `local`
  pub kind: String,
  /// The name of the declaration it is a member of
  pub container: Option<String>,
  /// A file outside the review (found by `accio`); none for a file of the review
  pub outside: Option<OutsideFile>,
}

/// A blob that the views ask for: by its id, and for a file of the working tree, by its path too
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BlobRequest {
  pub id: String,
  /// The path of the file in the working tree, relative to the repository, when the id is of a working tree file
  pub path: Option<String>,
}

/// The text of a blob
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BlobText {
  /// The bytes as UTF-8, an invalid sequence replaced with U+FFFD
  Text {
    id: String,
    text: String,
  },
  Binary {
    id: String,
  },
  /// Over `MAX_BLOB_BYTES`
  TooLarge {
    id: String,
    size: u32,
  },
  /// No blob of the repository and no file of the working tree has the id
  Missing {
    id: String,
  },
}

impl BlobText {
  pub fn id(&self) -> &str {
    match self {
      Self::Text { id, .. } | Self::Binary { id } | Self::TooLarge { id, .. } | Self::Missing { id } => id,
    }
  }

  /// The size of the blob in bytes, for the weight of a cache and for `DiffContent::TooLarge`
  pub fn size(&self) -> u32 {
    match self {
      Self::Text { text, .. } => u32::try_from(text.len()).unwrap_or(u32::MAX),
      Self::TooLarge { size, .. } => *size,
      Self::Binary { .. } | Self::Missing { .. } => 0,
    }
  }
}

/// The parameters of `get_file_merges`
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetFileMergesParams {
  pub repository_path: String,
  pub source: MergeSource,
  pub policy: WhitespacePolicy,
  /// The files in view, whose merges come first; then the others in the order of the tree
  pub priority_paths: Vec<String>,
}

/// What a merge merges
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MergeSource {
  /// The cherry-pick of `commit` onto `target`: `parent` is the base, `target` ours (left), `commit` theirs (right),
  /// as the sync merges it. `resolved` is the copy that the sync committed when the words resolved the conflict: each
  /// file then names its blob in the copy (`FileMergeHeader::resolved_id`)
  CherryPick {
    parent: String,
    target: String,
    commit: String,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(optional))]
    resolved: Option<String>,
  },
}

/// The conflict type of a path, as `merge-tree` names it (`CONFLICT (<type>)`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum GitConflictKind {
  Content,
  AddAdd,
  ModifyDelete,
  RenameDelete,
  RenameRename,
  Binary,
  Other,
}

/// A conflicting file before its content: the first event of `get_file_merges` lists them
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FileMergeHeader {
  pub path: String,
  pub kind: GitConflictKind,
  /// The blob ids of the stages: base (1), left (2, ours, the target) and right (3, theirs, the picked commit); `None`
  /// for a stage that the path does not have, and for a submodule, whose commits are in its content
  pub base_id: Option<String>,
  pub left_id: Option<String>,
  pub right_id: Option<String>,
  /// The blob of the file in the copy that resolved the conflict by words (`MergeSource::CherryPick::resolved`): what
  /// the sync committed, which the view shows as the result; `None` without that copy or for a path it lacks
  pub resolved_id: Option<String>,
}

/// What the conflict view shows of a file
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MergeContent {
  Text(TextMerge),
  /// A side is binary
  Binary,
  /// A side is over `MAX_BLOB_BYTES`; the sizes in bytes, 0 for a missing side
  #[serde(rename_all = "camelCase")]
  TooLarge {
    base_bytes: u32,
    left_bytes: u32,
    right_bytes: u32,
  },
  /// A submodule: the commit of each stage
  #[serde(rename_all = "camelCase")]
  Submodule {
    base_commit: Option<String>,
    left_commit: Option<String>,
    right_commit: Option<String>,
  },
  /// Git cannot read a blob of the file
  Missing,
  /// One side deleted the file and the other changed it: the diff of the base to the changed side, as two-way
  OneSided {
    deleted: MergeSide,
    diff: TextDiff,
  },
}

/// A side of a merge besides the base
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum MergeSide {
  Left,
  Right,
}

/// What `get_file_merges` sends through its channel: `files` once, then one `content` for each file, the files in view
/// first, and a `scopes` after the `content` of a text merge that the caches did not have
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FileMergeEvent {
  /// The conflicting files in the order of git; `index` of the other events points into this list
  Files {
    files: Vec<FileMergeHeader>,
  },
  Content {
    index: u32,
    content: MergeContent,
  },
  /// The scopes of the three sides of a text merge
  #[serde(rename_all = "camelCase")]
  Scopes {
    index: u32,
    left_scopes: Vec<Scope>,
    base_scopes: Vec<Scope>,
    right_scopes: Vec<Scope>,
  },
}

/// The merge of one conflicting file, as `FileDiffs::read_merges` gives it at once (the tests and the command line)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FileMerge {
  #[serde(flatten)]
  pub header: FileMergeHeader,
  pub content: MergeContent,
}

/// The usages of a name (`usages.rs`): those of the file of the name, of the other files of the review, then those
/// outside the review that the index knows
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FoundUsages {
  pub name: String,
  pub usages: Vec<UsageTarget>,
  /// The index knows more usages outside the review than it answered
  pub more_outside: bool,
  /// Why the index of the repository did not answer, when it was asked
  pub index_error: Option<String>,
}

/// A usage: its place (`kind` is how it uses the name: `declaration`, `import`, `call`, `write`, `read`) and its line
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UsageTarget {
  pub target: DeclarationTarget,
  /// The text of its line, without the indent
  pub text: String,
  /// Where the name starts in `text`, in UTF-16
  pub text_column: u32,
}

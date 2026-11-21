//! The merge conflicts of a sync and the conflict view (`git-ops`: `merge_conflict`, `conflict_analysis`), and the
//! conflicts of git a copy resolved by words

use crate::commit::{Commit, SubjectParts};
use crate::landing::StackCommit;
use generated_files::generated_by::GeneratedBy;
use serde::{Deserialize, Serialize};

/// A copy whose cherry-pick conflicted in git and resolved by words (`word_resolve`): what its mapping note records and
/// the commit row says ("Resolved 3 conflicts by words")
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct WordResolutionSummary {
  /// The parent of the original commit: the base of the merge that the conflict view opens again
  pub parent: String,
  /// The commit the copy is built on: ours of that merge
  pub target: String,
  /// The conflicts of git that the words resolved: the `<<<<<<<` lines git would have left
  pub conflicts: u32,
  /// The resolved files
  pub file_count: u32,
  /// The resolved files with their conflicts, in git's order; empty for a copy reused by its mapping note, which records
  /// the counts only. The conflict view merges the commit again for them (`get_file_merges`)
  pub files: Vec<ResolvedFileSummary>,
  /// The resolved files that `generated_files` calls generated, by path: their text is a merge of words, not the output
  /// of their generator. The mapping note records them, so a reused copy has them too
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub generated: Vec<GeneratedFile>,
}

/// A generated file (`generated_files`) and why it is generated: one that the words merged in a copy of a sync, or one
/// that an update brought together with the upstream's
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct GeneratedFile {
  pub path: String,
  pub generated_by: GeneratedBy,
}

impl GeneratedFile {
  /// The one text form of a list, compact JSON: the tail of the resolve line of a mapping note, and the field of a record
  /// that `postcard` keeps (an object cache), which cannot read back the tagged `GeneratedBy`
  pub fn encode(files: &[Self]) -> String {
    encode_list(files)
  }

  /// The list `encode` wrote; `None` for a text it did not write
  pub fn decode(text: &str) -> Option<Vec<Self>> {
    decode_list(text)
  }
}

/// A list as compact JSON, the text form of a list of records with a tagged enum, which `postcard` cannot read back
fn encode_list<T: Serialize>(items: &[T]) -> String {
  serde_json::to_string(items).unwrap_or_default()
}

/// The list `encode_list` wrote; `None` for a text it did not write
fn decode_list<T: serde::de::DeserializeOwned>(text: &str) -> Option<Vec<T>> {
  serde_json::from_str(text).ok()
}

/// A generated file that the words merged in the copy of `commit` and that the tip of a branch still holds as they wrote
/// it: the copy that branch pushes and lands is no output of its generator. A later commit of the branch that writes
/// the file, or a copy that no longer needs the words (the commit it lacks is below it: stacked, moved or landed),
/// leaves it out (docs/design/git-operations.md, "Resolve by words")
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct RegenerateFile {
  /// The original commit whose copy the words merged, with its branch: the branch itself or one it is built on
  pub commit: StackCommit,
  pub path: String,
  pub generated_by: GeneratedBy,
  /// The last commit ahead of the baseline before `commit` that changes the file and that the copy lacks: in another
  /// branch, or in none (unassigned). `None` when no such commit is found
  #[serde(default, skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub lacks: Option<StackCommit>,
}

impl RegenerateFile {
  /// The one text form of a list for a record that `postcard` keeps (the build of a stack branch), as `GeneratedFile::encode`
  pub fn encode(files: &[Self]) -> String {
    encode_list(files)
  }

  /// The list `encode` wrote; `None` for a text it did not write
  pub fn decode(text: &str) -> Option<Vec<Self>> {
    decode_list(text)
  }
}

/// A file of `WordResolutionSummary`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ResolvedFileSummary {
  pub path: String,
  pub conflicts: u32,
}

/// A conflicting file of a cherry-pick, as a conflict is reported: what the view lists before it reads the contents
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictFile {
  pub file: String,
  /// The conflicts of the merged file, its `<<<<<<<` marker lines (`count_conflicts`); 0 when one side deleted the
  /// file and the other changed it (a modify/delete conflict)
  pub conflict_count: u32,
}

/// A merge conflict of a cherry-pick, as a sync reports it: the commits, the conflicting files with their conflict
/// counts and the conflict analysis, without file contents and diffs, which the views read from `file_diffs`
/// (`get_file_merges`, `get_file_diffs`): a conflict in big files is megabytes, and the sync reports it at every run
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MergeConflictInfo {
  /// Commit being applied (cherry-picked)
  pub commit: ConflictCommit,
  /// Original parent of the cherry-picked commit
  pub original_parent: ConflictCommit,
  /// Target branch HEAD where we're trying to apply the commit
  pub target_branch: ConflictCommit,
  pub conflicting_files: Vec<ConflictFile>,
  pub conflict_analysis: ConflictAnalysis,
}

impl MergeConflictInfo {
  /// A conflict known only by its commits and files, without conflict counts or analysis (`MergeMemo::light_conflicts`)
  pub fn light(commit: &str, original_parent: &str, target: &str, files: Vec<String>) -> Self {
    let commit_of = |hash: &str| ConflictCommit {
      hash: hash.to_string(),
      message: String::new(),
      author: String::new(),
      author_time: 0,
      committer_time: 0,
      subject_parts: SubjectParts::default(),
    };
    Self {
      commit: commit_of(commit),
      original_parent: commit_of(original_parent),
      target_branch: commit_of(target),
      conflicting_files: files.into_iter().map(|file| ConflictFile { file, conflict_count: 0 }).collect(),
      conflict_analysis: ConflictAnalysis::default(),
    }
  }
}

/// A commit involved in a merge conflict: the applied commit, its original parent, the target, or one referenced in conflict markers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictCommit {
  pub hash: String,
  pub message: String,
  pub author: String,
  pub author_time: u32,
  pub committer_time: u32,
  /// The parts of the subject the page shows (`Commit::subject_parts`)
  pub subject_parts: SubjectParts,
}

impl From<&Commit> for ConflictCommit {
  fn from(commit: &Commit) -> Self {
    Self {
      hash: commit.id.clone(),
      message: commit.subject.clone(),
      author: commit.author_name.clone(),
      author_time: commit.author_timestamp,
      committer_time: commit.committer_timestamp,
      subject_parts: commit.subject_parts.clone(),
    }
  }
}

/// Represents a commit that exists in the source branch but is missing from the target branch.
/// These commits might be causing merge conflicts. The view reads their diffs with `get_file_diffs`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MissingCommit {
  pub hash: String,
  pub subject: String,
  pub message: String,
  /// Whether the message has a body a reader is shown (`Commit::has_body`)
  pub has_body: bool,
  pub author_time: u32,
  pub committer_time: u32,
  pub author: String,
  /// The conflicting files the commit changes
  pub files_touched: Vec<String>,
  /// The branch the commit is in, `None` for an unassigned commit; set by the sync, which knows the grouping
  pub branch: Option<String>,
  /// The parts of `subject` the page shows (`sync_utils::commit_subject::subject_parts`)
  pub subject_parts: SubjectParts,
}

/// Analysis results for a merge conflict, including missing commits and divergence information.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictAnalysis {
  pub missing_commits: Vec<MissingCommit>,
  pub merge_base_hash: String,
  pub merge_base_subject: String,
  pub merge_base_message: String,
  pub merge_base_time: u32,
  pub merge_base_author: String,
  pub divergence_summary: DivergenceSummary,
}

/// Summary of how two branches have diverged from their common ancestor.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DivergenceSummary {
  pub commits_ahead_in_source: u32,  // How many commits the source branch is ahead
  pub commits_ahead_in_target: u32,  // How many commits the target branch is ahead
  pub common_ancestor_distance: u32, // How far back is the common ancestor
}

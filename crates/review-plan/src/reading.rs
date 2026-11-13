//! What a range leaves to read (openspec/specs/review/range-reading/spec.md): of every file of the diffs of a subject,
//! the `DiffReading` its badge in the review window counts (`stops(true)`: a review with its plan passes the blocks of
//! the imports alone), summed by the role of the file (`modules::reading_role`); the files no proof can be made of,
//! with why; and the kinds of proof that passed a change, read from the marks themselves. Proven from the full marks
//! of `file-diffs` (`FileDiffs::collect`, then the answers of the index, `answer_proofs`), never from a second model of
//! the diff, so the sum is the sum of the badges.
//!
//! - **The plan** reads it of the pair its window shows (`ReviewPlan::reading`, `plan_reading`): before the index
//!   answered its renames wait (`Unproven::RenamesPending`), and the plan is not final.
//! - **The layers of a stack** (`layer_readings`) are read in one phase each: one `diff-tree --stdin` of their lists and
//!   one `cat-file --batch` of the blobs no cache has (`FileDiffs::ready_ranges`), then their marks from memory, then the
//!   index asked layer by layer, the bottom first.

use crate::PlanSources;
use crate::modules::reading_role;
use anyhow::Result;
use code_tokens::grammar_of;
use domain_types::range_reading::{FileRole, QuietProof, RangeReading, RoleReading, Unproven, UnprovenFile};
use file_diffs::model::{DiffContent, DiffSource, GetFileDiffsParams};
use file_diffs::reading::DiffReading;
use file_diffs::service::Collected;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::commit_files::FileStatus;
use std::collections::BTreeSet;
use text_diff::model::{RenameProof, RunKind, TextDiff, WhitespacePolicy};

/// The whitespace policy a reading is of: the default, as the plan reads its pair
pub const READING_POLICY: WhitespacePolicy = WhitespacePolicy::Default;

/// What the diffs of `read` leave to read, file by file as their badges count it
#[must_use]
pub fn range_reading(read: &Collected) -> RangeReading {
  let mut roles: Vec<RoleReading> = Vec::new();
  let mut unproven: Vec<UnprovenFile> = Vec::new();
  let mut proofs: BTreeSet<QuietProof> = BTreeSet::new();
  for (header, content) in read.files() {
    let role = reading_role(&header.path);
    let (reading, why) = match content {
      Some(DiffContent::Text(diff)) => {
        let reading = DiffReading::of(diff);
        proofs.extend(proofs_of(diff, &reading, header.status));
        (reading, text_unproven(&header.path, role, diff, &reading))
      }
      Some(DiffContent::Binary) => (DiffReading::default(), Some(Unproven::Binary)),
      Some(DiffContent::TooLarge { .. }) => (DiffReading::default(), Some(Unproven::TooLarge)),
      Some(DiffContent::Submodule { .. }) => (DiffReading::default(), Some(Unproven::Submodule)),
      Some(DiffContent::Missing) | None => (DiffReading::default(), Some(Unproven::Missing)),
    };
    let to_read = reading.stops(true);
    let at = roles.iter().position(|known| known.role == role).unwrap_or_else(|| {
      roles.push(RoleReading {
        role,
        files: 0,
        files_to_read: 0,
        changes: 0,
        to_read: 0,
      });
      roles.len() - 1
    });
    let summed = &mut roles[at];
    summed.files += 1;
    summed.files_to_read += u32::from(to_read > 0);
    summed.changes += reading.changes;
    summed.to_read += to_read;
    if let Some(why) = why {
      unproven.push(UnprovenFile {
        path: header.path.clone(),
        role,
        why,
      });
    }
  }
  roles.sort_by_key(|role| role.role);
  RangeReading {
    roles,
    unproven,
    proofs: proofs.into_iter().collect(),
  }
}

/// Why a text cannot be proven, if it cannot: no grammar for a file of code or resources with a change to read (no run
/// can be proven in it; prose is read as it is), else a rename that is not local the index did not prove, waiting for
/// it first
fn text_unproven(path: &str, role: FileRole, diff: &TextDiff, reading: &DiffReading) -> Option<Unproven> {
  if reading.stops(true) > 0 && role != FileRole::Prose && grammar_of(path).is_none() {
    return Some(Unproven::NoGrammar);
  }
  let subject = diff.marks.subject.as_deref()?;
  let proofs = subject.declared.iter().map(|check| check.proof).chain(subject.used.iter().map(|used| used.proof));
  let (mut pending, mut unanswered) = (false, false);
  for proof in proofs {
    match proof {
      RenameProof::Asked => pending = true,
      RenameProof::Unavailable | RenameProof::Incomplete => unanswered = true,
      RenameProof::Confirmed | RenameProof::UsesOutside | RenameProof::OtherMeaning | RenameProof::OtherDeclaration => {}
    }
  }
  if pending {
    Some(Unproven::RenamesPending)
  } else {
    unanswered.then_some(Unproven::RenamesUnanswered)
  }
}

/// The kinds of proof that passed a change of `diff`: its quiet runs and the runs the index confirmed by their kind,
/// its moves, its blocks of the imports alone that had a start, a moved file with no changed line, a change of the
/// line ends alone
fn proofs_of(diff: &TextDiff, reading: &DiffReading, status: FileStatus) -> Vec<QuietProof> {
  let marks = &diff.marks;
  let confirmed = marks.subject.as_deref().map_or(&[][..], |subject| subject.confirmed.as_slice());
  let mut proofs: Vec<QuietProof> = marks
    .runs
    .iter()
    .enumerate()
    .filter(|(index, run)| run.quiet || u32::try_from(*index).is_ok_and(|index| confirmed.contains(&index)))
    .map(|(_, run)| match run.kind {
      RunKind::Formatting => QuietProof::Formatting,
      RunKind::Comments => QuietProof::Comments,
      RunKind::Renamed => QuietProof::Renames,
    })
    .collect();
  if !marks.moves.is_empty() {
    proofs.push(QuietProof::Moves);
  }
  if reading.to_read > reading.to_read_past_imports {
    proofs.push(QuietProof::Imports);
  }
  if reading.changes == 0 && matches!(status, FileStatus::Renamed | FileStatus::Copied) {
    proofs.push(QuietProof::RenamedFiles);
  }
  if reading.changes == 0 && diff.line_ends.is_some() {
    proofs.push(QuietProof::LineEnds);
  }
  proofs
}

/// The diffs of `source` collected under `READING_POLICY`, their renames asked of the index when there is one
fn reading_params(repository_path: &str, source: DiffSource) -> GetFileDiffsParams {
  GetFileDiffsParams {
    repository_path: repository_path.to_string(),
    source,
    policy: READING_POLICY,
    priority_paths: Vec::new(),
  }
}

/// What the plan reads of what its pair leaves to read: the diffs of `source` collected (the memory of the diffs, the
/// store, else one `diff-tree` and one `cat-file --batch`), their renames still asked when there is an index; then
/// `answer` puts the answers of the index in
pub(crate) struct PlanReading {
  params: GetFileDiffsParams,
  read: Collected,
}

impl PlanReading {
  pub(crate) fn collect(git: &GitCommandExecutor, sources: PlanSources<'_>, repository_path: &str, source: DiffSource) -> Result<Self> {
    let params = reading_params(repository_path, source);
    let read = sources.file_diffs.collect(git, sources.cache, sources.index.is_some(), &params, &mut |_| {})?;
    Ok(Self { params, read })
  }

  pub(crate) fn reading(&self) -> RangeReading {
    range_reading(&self.read)
  }

  /// The answers of the index put in: one query of every rename of the subject, none when memory has a final answer
  /// of it (`FileDiffs::rename_proofs`)
  pub(crate) fn answer(&mut self, git: &GitCommandExecutor, sources: PlanSources<'_>) -> Result<()> {
    sources.file_diffs.answer_proofs(git, sources.cache, sources.index, &self.params, &mut self.read)
  }
}

/// What each of `ranges` (`(base, head)`, full commit ids: the layers of a stack, the bottom first) leaves to read, told
/// to `read` by the place of the range, twice for a layer with a rename that is not local: as the diffs say it, its
/// renames waiting for the index, then with its answer. The diffs of every layer first, in one phase each
/// (`FileDiffs::ready_ranges`), then the index layer by layer, the bottom first; a layer the index is not asked of is
/// told once
pub fn layer_readings(
  git: &GitCommandExecutor,
  sources: PlanSources<'_>,
  repository_path: &str,
  ranges: &[(String, String)],
  mut read: impl FnMut(usize, RangeReading),
) -> Result<()> {
  let pairs: Vec<(&str, &str)> = ranges.iter().map(|(base, head)| (base.as_str(), head.as_str())).collect();
  sources.file_diffs.ready_ranges(git, repository_path, sources.cache, &pairs, READING_POLICY)?;
  // the layers whose renames wait for the index, by their place
  let mut waiting: Vec<(usize, PlanReading)> = Vec::new();
  for (index, (base, head)) in ranges.iter().enumerate() {
    let source = DiffSource::Range {
      base: base.clone(),
      head: head.clone(),
    };
    let layer = PlanReading::collect(git, sources, repository_path, source)?;
    let reading = layer.reading();
    let pending = reading.pending();
    read(index, reading);
    if pending {
      waiting.push((index, layer));
    }
  }
  for (index, mut layer) in waiting {
    layer.answer(git, sources)?;
    read(index, layer.reading());
  }
  Ok(())
}

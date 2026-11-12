//! The marks of a subject as a measurement reads them (`brd commits --mark-stats`,
//! openspec/specs/commit/quiet-changes/spec.md, "Measurement"; docs/design/diff-view.md, "Measured on real history"):
//! the diffs a view of the subject gets, through the stream a view reads, with what a view is not told.
//!
//! - **The time** from the start of the read to its first content and to its end, and of the answer of the index.
//! - **The texts** of both sides of each file, so the lines of a mark can be printed and read by a person.
//! - **The proofs**: the runs whose proof holds though their kind is a label (`PairMarks::proven`,
//!   `SubjectMarks::confirmed`), and the stops the file has once every kind with a proof is quiet. They are what the
//!   sample of a kind is read for before `marks::is_quiet` switches it on.
//! - **Whether the file was parsed** for its marks: the share of the files the question before the tokens leaves
//!   unread.
//!
//! The marks of each pair are read once more from its texts for the proofs, by the code that made them; the
//! measurement says whether both readings agree (`MeasuredText::differs`). `FileDiffs::without_marks` reads the same
//! subject without any mark, for what the marks cost.

use crate::accio::SymbolIndex;
use crate::marks::pair_grammar;
use crate::model::{BlobText, DiffContent, DiffFile, DiffSource, FileDiffEvent, GetFileDiffsParams};
use crate::service::FileDiffs;
use anyhow::Result;
use code_symbols::highlight::Grammar;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use std::cell::Cell;
use std::sync::Arc;
use std::time::{Duration, Instant};
use text_diff::model::{LineRun, starts_of};
use text_diff::policy::WhitespacePolicy;

/// A subject as a measurement reads it
#[derive(Debug)]
pub struct MeasuredSubject {
  /// Its files, in the order of git
  pub files: Vec<MeasuredFile>,
  /// From the start of the read to its first content; `None` for a subject without a file
  pub first_content: Option<Duration>,
  /// From the start of the read to the end of its stream
  pub end: Duration,
  /// The answer of the index after it; `None` when none was asked
  pub index: Option<Duration>,
}

/// A file of a measured subject
#[derive(Debug)]
pub struct MeasuredFile {
  /// The file as a view gets it: its marks are those of the stream, with what the subject and the index say
  pub file: DiffFile,
  /// What a measurement reads beside the diff of a text; `None` for a file that is no text and for a diff without
  /// marks
  pub text: Option<MeasuredText>,
}

/// What a measurement reads of a text file beside its diff
#[derive(Debug)]
pub struct MeasuredText {
  /// The grammar both sides are read by (`marks::pair_grammar`); without one the file has moves alone
  pub grammar: Option<Grammar>,
  /// The two texts, the old one first; the empty text for a side the file does not exist on
  pub texts: [Arc<BlobText>; 2],
  /// The tokens of a side were read for the marks: a block answered the question before the tokens with yes
  pub parsed: bool,
  /// For each run of the marks, in their order: its proof holds. A quiet run, a run the scopes of the file prove
  /// (`PairMarks::proven`) and a run the index confirmed (`SubjectMarks::confirmed`)
  pub proven: Vec<bool>,
  /// The renames of the marks, by their numbers, whose places the scopes of the file prove
  pub proven_renames: Vec<u32>,
  /// The starts of the file once every kind with a proof is quiet: `DiffMarks::starts` with the proven runs passed
  pub proven_starts: Vec<u32>,
  /// What of the marks of the pair, read again from its texts, is not what the view got: its `moves`, its `renames`,
  /// or its `runs` (every run of the pair is among those of the view, as quiet as there, and the subject alone adds
  /// runs to them). `None` when both readings agree, as they must
  pub differs: Option<&'static str>,
}

impl MeasuredText {
  /// The text of a side, `true` the new one
  pub fn text(&self, new_side: bool) -> &str {
    match &*self.texts[usize::from(new_side)] {
      BlobText::Text { text, .. } => text,
      BlobText::Binary { .. } | BlobText::TooLarge { .. } | BlobText::Missing { .. } => "",
    }
  }
}

impl FileDiffs {
  /// The subject `source` of the repository under `policy`, read as a view reads it and measured (module doc).
  /// `index` is asked of the names the subject renamed after its diffs, as the window that shows it asks
  pub fn measure(
    &self,
    git: &GitCommandExecutor,
    repository_path: &str,
    cache: &ObjectCache,
    index: Option<&dyn SymbolIndex>,
    source: &DiffSource,
    policy: WhitespacePolicy,
  ) -> Result<MeasuredSubject> {
    let params = GetFileDiffsParams {
      repository_path: repository_path.to_string(),
      source: source.clone(),
      policy,
      priority_paths: Vec::new(),
    };
    let started = Instant::now();
    let mut first_content = None;
    let mut read = self.collect(git, cache, index.is_some(), &params, &mut |event| {
      if first_content.is_none() && matches!(event, FileDiffEvent::Content { .. }) {
        first_content = Some(started.elapsed());
      }
    })?;
    let end = started.elapsed();
    self.answer_proofs(git, cache, index, &params, &mut read)?;
    let answered = index.map(|_| started.elapsed().saturating_sub(end));
    let files = read.into_files();
    if self.marks_off {
      let files = files.into_iter().map(|file| MeasuredFile { file, text: None }).collect();
      return Ok(MeasuredSubject {
        files,
        first_content,
        end,
        index: answered,
      });
    }

    // the texts of the pairs: memory has those the stream read
    let texts = files.iter().filter(|file| matches!(file.content, DiffContent::Text(_)));
    let blobs = self.read_blobs(git, repository_path, texts.flat_map(|file| [file.old_id.as_deref(), file.new_id.as_deref()]).flatten())?;
    let empty = Arc::new(BlobText::Text {
      id: String::new(),
      text: String::new(),
    });
    let files = files
      .into_iter()
      .map(|file| {
        let side = |id: Option<&str>| id.map_or_else(|| Some(Arc::clone(&empty)), |id| blobs.get(id).cloned());
        let text = match (&file.content, side(file.old_id.as_deref()), side(file.new_id.as_deref())) {
          (DiffContent::Text(_), Some(old), Some(new)) => Some(self.measured_text(&file, policy, [old, new])),
          _ => None,
        };
        MeasuredFile { file, text }
      })
      .collect();
    Ok(MeasuredSubject {
      files,
      first_content,
      end,
      index: answered,
    })
  }

  /// What a measurement reads of the text file `file` with the `texts` of its sides
  fn measured_text(&self, file: &DiffFile, policy: WhitespacePolicy, texts: [Arc<BlobText>; 2]) -> MeasuredText {
    let paths = [file.old_path.as_deref().unwrap_or(&file.path), file.path.as_str()];
    let mut measured = MeasuredText {
      grammar: pair_grammar(paths[0], paths[1]),
      texts,
      parsed: false,
      proven: Vec::new(),
      proven_renames: Vec::new(),
      proven_starts: Vec::new(),
      differs: None,
    };
    let DiffContent::Text(diff) = &file.content else { return measured };
    let parsed = Cell::new(false);
    let pair = self.marks_told(
      diff,
      policy,
      paths,
      [file.old_id.as_deref(), file.new_id.as_deref()],
      [measured.text(false), measured.text(true)],
      &|_| parsed.set(true),
    );
    measured.parsed = parsed.get();
    let shown = &diff.marks;
    let place = |run: &LineRun| (run.old_start, run.old_end, run.new_start, run.new_end, run.kind);
    let find = |run: &LineRun| shown.runs.iter().position(|other| place(other) == place(run));
    // a run of the view that holds `run` in its kind: the subject adds labels of the kind `Renamed` to the runs of
    // the pair, and one of them may hold a label of the pair
    let holds = |run: &LineRun| {
      shown.runs.iter().any(|other| {
        other.kind == run.kind
          && !other.quiet
          && other.old_start <= run.old_start
          && run.old_end <= other.old_end
          && other.new_start <= run.new_start
          && run.new_end <= other.new_end
      })
    };
    // the subject adds runs to those of the pair and takes none, and a quiet run is there as it is; the moves and
    // the renames are the pair's
    let same_runs = pair
      .marks
      .runs
      .iter()
      .all(|run| find(run).is_some_and(|at| shown.runs[at].quiet == run.quiet) || (!run.quiet && holds(run)))
      && (shown.subject.is_some() || pair.marks.runs.len() == shown.runs.len());
    measured.differs = if pair.marks.moves != shown.moves {
      Some("moves")
    } else if pair.marks.renames != shown.renames {
      Some("renames")
    } else if same_runs {
      None
    } else {
      Some("runs")
    };
    let mut proven: Vec<bool> = shown.runs.iter().map(|run| run.quiet).collect();
    for run in pair.proven.iter().filter_map(|run| pair.marks.runs.get(*run as usize)) {
      if let Some(at) = find(run) {
        proven[at] = true;
      }
    }
    for run in shown.subject.iter().flat_map(|subject| &subject.confirmed) {
      if let Some(proven) = proven.get_mut(*run as usize) {
        *proven = true;
      }
    }
    let passed: Vec<LineRun> = shown.runs.iter().zip(&proven).map(|(run, proven)| LineRun { quiet: *proven, ..run.clone() }).collect();
    measured.proven_starts = starts_of(&diff.blocks, &passed, &shown.moves);
    measured.proven = proven;
    measured.proven_renames = pair.proven_renames;
    measured
  }
}

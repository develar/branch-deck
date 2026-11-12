//! `brd commits --mark-stats` (hidden): what the marks of the diffs find on real commits, for the measurement that
//! decides which kinds of them are quiet and where their thresholds stand (openspec/specs/commit/quiet-changes/spec.md,
//! "Measurement"; docs/design/diff-view.md, "Measured on real history"). Each commit ahead of the baseline, or of
//! `--range <base>..<head>`, is read as a view of it reads it (`FileDiffs::measure`: the stream of the app, the same
//! caches), or the range as one subject with `--whole`.
//!
//! - **The counts**, in all and by the grammar of the files: the blocks, the stops the views have and the stops they
//!   have once every proven run is quiet, the runs of each kind, the moves within a file by their size and their
//!   edited lines, the renames by what proves them, the moves between files.
//! - **The times** of each read to its first content and to its end; `--no-marks` reads the same subjects without any
//!   mark, which is what the marks cost.
//! - **The sample** (`--mark-sample <N>`): N marks of each kind, spread evenly over the subjects, each with the lines
//!   of both sides, for a person to read: a quiet kind must hold no change of behaviour, a label must be true.
//!
//! It writes the object cache only, and nothing with `--no-cache`.

use bd_core::branch_view::read_grouping;
use bd_core::context::RepoContext;
use bd_core::error::CliError;
use code_symbols::highlight::Grammar;
use domain_types::error::ErrorCode;
use file_diffs::accio::{AccioCli, SymbolIndex};
use file_diffs::marks::is_quiet;
use file_diffs::marks_measure::{MeasuredFile, MeasuredSubject, MeasuredText};
use file_diffs::model::{DiffContent, DiffSource};
use file_diffs::service::{FileDiffs, MODEL_VERSION};
use git_ops::landed_pairing::walk_commits;
use git_ops::revs::resolve_commit;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::ops::Range;
use std::time::Duration;
use text_diff::model::{BlockKind, FileMove, LineMove, LineRun, Rename, RenameProof, RunKind, TextDiff};
use text_diff::moves::{MAX_EDITED_LINES, MIN_MOVE_CHARS, line_starts, line_text};
use text_diff::moves_across::{MIN_ACROSS_CHARS, MIN_ACROSS_LINES};
use text_diff::policy::WhitespacePolicy;

/// The lines of a side of a sampled mark that are printed: a longer side is cut, and says by how many lines
const MAX_SAMPLE_LINES: usize = 60;
/// The lines printed around the lines of a sampled mark
const SAMPLE_CONTEXT: u32 = 2;
/// The files named whose marks, read again, are not those a view got
const MAX_DIFFERING_FILES: usize = 20;
/// The subjects named with their times: the slowest ones
const SLOWEST_SUBJECTS: usize = 10;

#[derive(Debug, clap::Args)]
#[expect(clippy::struct_excessive_bools, reason = "independent command line flags")]
pub struct MarkStatsArgs {
  /// What the marks of the diffs find on each commit: the measurement of their quiet kinds and thresholds
  #[arg(long, hide = true, conflicts_with_all = ["brief", "files", "patch", "symbol_stats"])]
  pub mark_stats: bool,
  /// With --mark-stats: the commits of `<base>..<head>` without its merges, in place of those ahead of the baseline
  #[arg(long, hide = true, value_name = "BASE..HEAD", requires = "mark_stats")]
  pub range: Option<String>,
  /// With --range: the range as one subject, as a review of it reads it
  #[arg(long, hide = true, requires = "range")]
  pub whole: bool,
  /// With --mark-stats: the marks of each kind to print with the lines of both sides
  #[arg(long, hide = true, value_name = "N", default_value_t = 0, requires = "mark_stats")]
  pub mark_sample: usize,
  /// With --mark-stats: read the subjects without any mark and without a store, for what the marks cost
  #[arg(long, hide = true, requires = "mark_stats")]
  pub no_marks: bool,
  /// With --mark-stats: ask the index of the repository of the names each subject renamed, as its window does
  #[arg(long, hide = true, requires = "mark_stats", conflicts_with = "no_marks")]
  pub index: bool,
}

#[derive(Debug, Serialize)]
pub struct MarkStatsOutput {
  /// The last subject read
  pub head: String,
  /// The subjects read: commits, or one range
  pub subjects: usize,
  /// Those with a text file of each grammar (`languages`): a subject of two grammars counts for both
  pub subjects_of: BTreeMap<&'static str, usize>,
  pub model_version: u32,
  /// False with --no-marks
  pub marks: bool,
  /// The kinds of runs that are quiet where their proof holds (`file_diffs::marks::is_quiet`)
  pub quiet_kinds: Vec<&'static str>,
  pub limits: MarkLimits,
  pub total: MarkCounts,
  /// The first files of `total.differ`, each as its subject, its path and what differs
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub differ_in: Vec<String>,
  /// By the grammar both sides of a file are read by, `none` without one
  pub languages: BTreeMap<&'static str, MarkCounts>,
  pub time: MarkTimes,
  /// With --mark-sample: marks of each kind, in the order of the subjects
  pub sample: Vec<SampleMark>,
}

/// The thresholds of the moves the counts were read under
#[derive(Debug, Serialize)]
pub struct MarkLimits {
  pub min_move_chars: usize,
  /// The share of edited lines a move may hold, as numerator and denominator
  pub max_edited_lines: [i64; 2],
  pub min_across_chars: u32,
  pub min_across_lines: u32,
}

#[derive(Debug, Default, Serialize)]
pub struct MarkCounts {
  /// The text files with a block that is not ignored
  pub files: usize,
  /// Those whose tokens were read for their marks
  pub parsed: usize,
  /// Those whose marks, read again from their texts, are not the marks a view got: none, or the proofs counted here
  /// are not of what the views show
  pub differ: usize,
  /// The blocks that are not ignored: the stops without marks
  pub blocks: usize,
  /// The stops of the views
  pub stops: usize,
  /// The stops once every run whose proof holds is quiet
  pub stops_proven: usize,
  /// The lines of those blocks
  pub old_lines: u64,
  pub new_lines: u64,
  pub formatting: RunCounts,
  pub comments: RunCounts,
  pub renamed: RunCounts,
  pub moves: MoveCounts,
  pub renames: RenameCounts,
  pub across: AcrossCounts,
}

/// The runs of one kind
#[derive(Debug, Default, Serialize)]
pub struct RunCounts {
  pub runs: usize,
  /// Those a reader passes
  pub quiet: usize,
  /// Those whose proof holds: the quiet ones, and those that are labels while their kind is one
  pub proven: usize,
  pub old_lines: u64,
  pub new_lines: u64,
}

/// The moves within a file
#[derive(Debug, Default, Serialize)]
pub struct MoveCounts {
  pub moves: usize,
  pub lines: u64,
  /// Without an edited line, and with one
  pub exact: usize,
  pub edited: usize,
  pub edited_lines: u64,
  pub reindented: usize,
  /// Of one line
  pub one_line: usize,
  /// By the characters of their exact lines that are no whitespace: under 32, under 48, under 96, and more
  pub chars: [usize; 4],
  /// By the share of their edited lines among their lines: none, up to an eighth, up to a quarter
  pub edited_share: [usize; 3],
  /// Those none of whose exact lines stands once among the deleted lines and once among the added ones of its file:
  /// which of the equal lines moved, nothing says. In all, and of one line
  pub unanchored: usize,
  pub one_line_unanchored: usize,
}

/// The renames of the files
#[derive(Debug, Default, Serialize)]
pub struct RenameCounts {
  /// The names bound inside a body, and those of them the scopes prove at every place
  pub local: usize,
  pub local_proven: usize,
  pub local_sites: u64,
  /// The names that are not local, where their file declares them, and where a file only uses them
  pub declared: usize,
  pub used: usize,
  pub other_sites: u64,
  /// What the index said of those, by the proof of each `declared` and `used` name
  pub proofs: BTreeMap<&'static str, usize>,
}

/// The moves between two files of a subject
#[derive(Debug, Default, Serialize)]
pub struct AcrossCounts {
  pub moves: usize,
  pub lines: u64,
  /// By the characters of their lines that are no whitespace: under 64, under 96, and more
  pub chars: [usize; 3],
}

#[derive(Debug, Default, Serialize)]
pub struct MarkTimes {
  /// Of each subject, from the start of its read, in milliseconds
  pub first_content_ms: Spread,
  pub end_ms: Spread,
  /// The answers of the index, with --index
  #[serde(skip_serializing_if = "Option::is_none")]
  pub index_ms: Option<Spread>,
  /// The subjects that took the longest to their end
  pub slowest: Vec<SubjectTime>,
}

/// How a time spreads over the subjects, in milliseconds
#[derive(Debug, Default, Serialize)]
pub struct Spread {
  pub sum: f64,
  pub median: f64,
  pub p95: f64,
  pub max: f64,
}

#[derive(Debug, Serialize)]
pub struct SubjectTime {
  pub subject: String,
  pub files: usize,
  pub first_content_ms: f64,
  pub end_ms: f64,
}

/// A mark with its lines, for a person to read
#[derive(Debug, Serialize)]
pub struct SampleMark {
  pub kind: &'static str,
  pub subject: String,
  pub path: String,
  pub language: &'static str,
  /// The first line of the mark in the old text and in the new one, from 1, and its lines there
  pub old_line: u32,
  pub old_count: u32,
  pub new_line: u32,
  pub new_count: u32,
  /// What the mark says beside its lines
  pub note: String,
  /// The lines of each side: `| ` before a line of the mark, `~ ` before an edited line of a move, two spaces
  /// before a line around it
  pub old: Vec<String>,
  pub new: Vec<String>,
}

/// The subjects of `args`: each commit ahead of the baseline, each commit of `--range`, or the range whole
fn subjects_of(context: &RepoContext, args: &MarkStatsArgs) -> Result<Vec<DiffSource>, CliError> {
  let Some(range) = &args.range else {
    return Ok(read_grouping(context, None)?.commit_ids.into_iter().map(|id| DiffSource::Commit { id }).collect());
  };
  let invalid = |message: String| CliError::new(ErrorCode::InvalidArguments, message);
  let (base, head) = range.split_once("..").ok_or_else(|| invalid(format!("--range takes <base>..<head>, not {range:?}")))?;
  let resolved = |rev: &str| resolve_commit(&context.git, &context.repository, rev)?.ok_or_else(|| invalid(format!("No commit {rev:?}")));
  let (base, head) = (resolved(base)?, resolved(head)?);
  if args.whole {
    return Ok(vec![DiffSource::Range { base, head }]);
  }
  let commits = walk_commits(&context.git, &context.repository, &["--reverse", "--no-merges"], &[&head, &format!("^{base}")])?;
  Ok(commits.into_iter().map(|commit| DiffSource::Commit { id: commit.id }).collect())
}

/// The name of a subject in the output: the id of a commit, the two ids of a range
fn subject_name(context: &RepoContext, source: &DiffSource) -> String {
  let short = |id: &str| {
    if context.short_ids { git_ops::revs::short_id(id).to_string() } else { id.to_string() }
  };
  match source {
    DiffSource::Commit { id } => short(id),
    DiffSource::Range { base, head } => format!("{}..{}", short(base), short(head)),
    DiffSource::Uncommitted => "uncommitted".to_string(),
  }
}

pub fn mark_stats(context: &RepoContext, args: &MarkStatsArgs) -> Result<MarkStatsOutput, CliError> {
  let subjects = subjects_of(context, args)?;
  let diffs = if args.no_marks { FileDiffs::without_marks() } else { FileDiffs::default() };
  let cache = context.cache();
  let never_canceled = || false;
  let search = if args.index {
    AccioCli::discover().map(|accio| accio.search(&never_canceled))
  } else {
    None
  };
  let index = search.as_ref().map(|search| search as &dyn SymbolIndex);

  let mut languages: BTreeMap<&'static str, MarkCounts> = BTreeMap::new();
  let mut sampler = Sampler::new(args.mark_sample);
  let mut times: Vec<SubjectTime> = Vec::with_capacity(subjects.len());
  let mut index_times: Vec<f64> = Vec::new();
  let mut subjects_of: BTreeMap<&'static str, usize> = BTreeMap::new();
  let mut differ_in: Vec<String> = Vec::new();
  for source in &subjects {
    let measured = diffs.measure(&context.git, &context.repository, &cache, index, source, WhitespacePolicy::Default)?;
    let subject = subject_name(context, source);
    let mut read: BTreeSet<&'static str> = BTreeSet::new();
    for file in &measured.files {
      let DiffContent::Text(diff) = &file.file.content else { continue };
      let language = language_of(file.text.as_ref());
      read.insert(language);
      count_file(languages.entry(language).or_default(), diff, file.text.as_ref());
      if let Some(differs) = file.text.as_ref().and_then(|text| text.differs)
        && differ_in.len() < MAX_DIFFERING_FILES
      {
        differ_in.push(format!("{subject} {}: {differs}", file.file.path));
      }
      if let Some(text) = &file.text
        && sampler.size > 0
      {
        sample_file(&mut sampler, &subject, &measured, file, diff, text);
      }
    }
    times.push(SubjectTime {
      subject,
      files: measured.files.len(),
      first_content_ms: measured.first_content.map_or(0.0, millis),
      end_ms: millis(measured.end),
    });
    index_times.extend(measured.index.map(millis));
    for language in read {
      *subjects_of.entry(language).or_default() += 1;
    }
  }

  let mut total = MarkCounts::default();
  for counts in languages.values() {
    total.add(counts);
  }
  let head = times.last().map_or_default(|time| time.subject.clone());
  let first_content_ms = Spread::of(times.iter().map(|time| time.first_content_ms).collect());
  let end_ms = Spread::of(times.iter().map(|time| time.end_ms).collect());
  times.sort_by(|a, b| b.end_ms.total_cmp(&a.end_ms));
  times.truncate(SLOWEST_SUBJECTS);
  Ok(MarkStatsOutput {
    head,
    subjects: subjects.len(),
    subjects_of,
    model_version: MODEL_VERSION,
    marks: !args.no_marks,
    quiet_kinds: [RunKind::Formatting, RunKind::Comments, RunKind::Renamed]
      .into_iter()
      .filter(|kind| is_quiet(*kind))
      .map(run_name)
      .collect(),
    limits: MarkLimits {
      min_move_chars: MIN_MOVE_CHARS,
      max_edited_lines: MAX_EDITED_LINES.into(),
      min_across_chars: MIN_ACROSS_CHARS,
      min_across_lines: MIN_ACROSS_LINES,
    },
    total,
    differ_in,
    languages,
    time: MarkTimes {
      first_content_ms,
      end_ms,
      index_ms: (!index_times.is_empty()).then(|| Spread::of(index_times)),
      slowest: times,
    },
    sample: sampler.into_sample(),
  })
}

/// A time in milliseconds, to a tenth
fn millis(time: Duration) -> f64 {
  (time.as_secs_f64() * 10_000.0).round() / 10.0
}

impl Spread {
  fn of(mut times: Vec<f64>) -> Self {
    if times.is_empty() {
      return Self::default();
    }
    times.sort_by(f64::total_cmp);
    let at = |share: usize| times[(times.len() * share / 100).min(times.len() - 1)];
    Self {
      sum: (times.iter().sum::<f64>() * 10.0).round() / 10.0,
      median: at(50),
      p95: at(95),
      max: times[times.len() - 1],
    }
  }
}

const fn run_name(kind: RunKind) -> &'static str {
  match kind {
    RunKind::Formatting => "formatting",
    RunKind::Comments => "comments",
    RunKind::Renamed => "renamed",
  }
}

const fn proof_name(proof: RenameProof) -> &'static str {
  match proof {
    RenameProof::Asked => "asked",
    RenameProof::Confirmed => "confirmed",
    RenameProof::UsesOutside => "usesOutside",
    RenameProof::OtherMeaning => "otherMeaning",
    RenameProof::OtherDeclaration => "otherDeclaration",
    RenameProof::Incomplete => "incomplete",
    RenameProof::Unavailable => "unavailable",
  }
}

/// The grammar both sides of a file are read by, as the counts name it
fn language_of(text: Option<&MeasuredText>) -> &'static str {
  match text.and_then(|text| text.grammar) {
    None => "none",
    Some(Grammar::Kotlin) => "kotlin",
    Some(Grammar::KotlinScript) => "kotlinScript",
    Some(Grammar::Java) => "java",
    Some(Grammar::Rust) => "rust",
    Some(Grammar::TypeScript) => "typescript",
    Some(Grammar::Tsx) => "tsx",
    Some(Grammar::JavaScript) => "javascript",
    Some(Grammar::CSharp) => "csharp",
    Some(Grammar::Starlark) => "starlark",
    Some(Grammar::StarlarkBuild) => "starlarkBuild",
  }
}

impl MarkCounts {
  fn add(&mut self, other: &Self) {
    self.files += other.files;
    self.parsed += other.parsed;
    self.differ += other.differ;
    self.blocks += other.blocks;
    self.stops += other.stops;
    self.stops_proven += other.stops_proven;
    self.old_lines += other.old_lines;
    self.new_lines += other.new_lines;
    for (runs, more) in [
      (&mut self.formatting, &other.formatting),
      (&mut self.comments, &other.comments),
      (&mut self.renamed, &other.renamed),
    ] {
      runs.runs += more.runs;
      runs.quiet += more.quiet;
      runs.proven += more.proven;
      runs.old_lines += more.old_lines;
      runs.new_lines += more.new_lines;
    }
    let (moves, more) = (&mut self.moves, &other.moves);
    moves.moves += more.moves;
    moves.lines += more.lines;
    moves.exact += more.exact;
    moves.edited += more.edited;
    moves.edited_lines += more.edited_lines;
    moves.reindented += more.reindented;
    moves.one_line += more.one_line;
    for (count, more) in moves.chars.iter_mut().zip(more.chars) {
      *count += more;
    }
    for (count, more) in moves.edited_share.iter_mut().zip(more.edited_share) {
      *count += more;
    }
    moves.unanchored += more.unanchored;
    moves.one_line_unanchored += more.one_line_unanchored;
    let (renames, more) = (&mut self.renames, &other.renames);
    renames.local += more.local;
    renames.local_proven += more.local_proven;
    renames.local_sites += more.local_sites;
    renames.declared += more.declared;
    renames.used += more.used;
    renames.other_sites += more.other_sites;
    for (proof, count) in &more.proofs {
      *renames.proofs.entry(proof).or_default() += count;
    }
    self.across.moves += other.across.moves;
    self.across.lines += other.across.lines;
    for (count, more) in self.across.chars.iter_mut().zip(other.across.chars) {
      *count += more;
    }
  }
}

/// Counts the marks of the diff of a text file; `text` is what the measurement read beside it, `None` for a diff
/// without marks
fn count_file(counts: &mut MarkCounts, diff: &TextDiff, text: Option<&MeasuredText>) {
  let blocks = || diff.blocks.iter().filter(|block| block.kind != BlockKind::Ignored);
  if blocks().next().is_none() {
    return;
  }
  let marks = &diff.marks;
  counts.files += 1;
  counts.blocks += blocks().count();
  counts.stops += marks.starts.len() / 3;
  counts.stops_proven += text.map_or(marks.starts.len(), |text| text.proven_starts.len()) / 3;
  counts.old_lines += blocks().map(|block| u64::from(block.old_end - block.old_start)).sum::<u64>();
  counts.new_lines += blocks().map(|block| u64::from(block.new_end - block.new_start)).sum::<u64>();
  let Some(text) = text else { return };
  counts.parsed += usize::from(text.parsed);
  counts.differ += usize::from(text.differs.is_some());

  for (run, proven) in marks.runs.iter().zip(&text.proven) {
    let runs = match run.kind {
      RunKind::Formatting => &mut counts.formatting,
      RunKind::Comments => &mut counts.comments,
      RunKind::Renamed => &mut counts.renamed,
    };
    runs.runs += 1;
    runs.quiet += usize::from(run.quiet);
    runs.proven += usize::from(*proven);
    runs.old_lines += u64::from(run.old_end - run.old_start);
    runs.new_lines += u64::from(run.new_end - run.new_start);
  }

  let (old, new) = (SideLines::new(text.text(false)), SideLines::new(text.text(true)));
  let free = FreeLines::of(diff, &old, &new);
  for moved in &marks.moves {
    let moves = &mut counts.moves;
    let unanchored = !free.anchors(moved, &old);
    moves.unanchored += usize::from(unanchored);
    moves.one_line_unanchored += usize::from(unanchored && moved.len == 1);
    moves.moves += 1;
    moves.lines += u64::from(moved.len);
    moves.exact += usize::from(moved.edited.is_empty());
    moves.edited += usize::from(!moved.edited.is_empty());
    moves.edited_lines += moved.edited.len() as u64;
    moves.reindented += usize::from(moved.indent != 0);
    moves.one_line += usize::from(moved.len == 1);
    moves.chars[match exact_chars(moved, &old) {
      0..32 => 0,
      32..48 => 1,
      48..96 => 2,
      _ => 3,
    }] += 1;
    let edited = moved.edited.len() as u64 * 8;
    moves.edited_share[if edited == 0 {
      0
    } else if edited <= u64::from(moved.len) {
      1
    } else {
      2
    }] += 1;
  }

  for (number, rename) in (0u32..).zip(&marks.renames) {
    let renames = &mut counts.renames;
    if rename.local {
      renames.local += 1;
      renames.local_proven += usize::from(text.proven_renames.contains(&number));
      renames.local_sites += u64::from(rename.sites);
    } else {
      renames.declared += 1;
      renames.other_sites += u64::from(rename.sites);
    }
  }
  if let Some(subject) = &marks.subject {
    let renames = &mut counts.renames;
    renames.used += subject.used.len();
    renames.other_sites += subject.used.iter().map(|used| u64::from(used.rename.sites)).sum::<u64>();
    let proofs = subject.declared.iter().map(|check| check.proof).chain(subject.used.iter().map(|used| used.proof));
    for proof in proofs {
      *renames.proofs.entry(proof_name(proof)).or_default() += 1;
    }
    counts.across.moves += subject.moved_in.len();
    counts.across.lines += subject.moved_in.iter().map(|moved| u64::from(moved.len)).sum::<u64>();
    for moved in &subject.moved_in {
      let chars: usize = (moved.start..moved.start + moved.len).map(|line| solid_chars(new.line(line))).sum();
      counts.across.chars[match chars {
        0..64 => 0,
        64..96 => 1,
        _ => 2,
      }] += 1;
    }
  }
}

/// The characters of a line that are no whitespace
fn solid_chars(line: &str) -> usize {
  line.chars().filter(|character| !character.is_whitespace()).count()
}

/// The old lines of a move that are not edited, from 0
fn exact_lines(moved: &LineMove) -> impl Iterator<Item = u32> + '_ {
  (0..moved.len)
    .filter(|offset| moved.edited.binary_search(offset).is_err())
    .map(|offset| moved.old_start + offset)
}

/// The characters that are no whitespace of the lines of a move that are not edited, in its old text
fn exact_chars(moved: &LineMove, old: &SideLines<'_>) -> usize {
  exact_lines(moved).map(|line| solid_chars(old.line(line))).sum()
}

/// The lines of a file a move may take, by their text without the spaces and tabs at their ends: those of the blocks
/// that are a change and in no run, counted on each side
struct FreeLines<'t> {
  old: HashMap<&'t str, u32>,
  new: HashMap<&'t str, u32>,
}

/// A line as a move reads it: without the spaces and tabs at its ends
fn move_key(line: &str) -> &str {
  line.trim_matches([' ', '\t'])
}

impl<'t> FreeLines<'t> {
  fn of(diff: &TextDiff, old: &SideLines<'t>, new: &SideLines<'t>) -> Self {
    let mut free = Self {
      old: HashMap::new(),
      new: HashMap::new(),
    };
    let runs = &diff.marks.runs;
    for block in diff.blocks.iter().filter(|block| block.kind == BlockKind::Changed) {
      for line in block.old_start..block.old_end {
        if !runs.iter().any(|run| (run.old_start..run.old_end).contains(&line)) {
          *free.old.entry(move_key(old.line(line))).or_default() += 1;
        }
      }
      for line in block.new_start..block.new_end {
        if !runs.iter().any(|run| (run.new_start..run.new_end).contains(&line)) {
          *free.new.entry(move_key(new.line(line))).or_default() += 1;
        }
      }
    }
    free
  }

  /// Whether a line of `moved` that is not edited and not blank stands once among the old lines a move may take and
  /// once among the new ones: only such a line says which of the equal lines moved
  fn anchors(&self, moved: &LineMove, old: &SideLines<'_>) -> bool {
    exact_lines(moved)
      .map(|line| move_key(old.line(line)))
      .any(|key| !key.is_empty() && self.old.get(key) == Some(&1) && self.new.get(key) == Some(&1))
  }
}

/// A text with the byte each of its lines starts at, as the comparison counts lines
struct SideLines<'t> {
  text: &'t str,
  starts: Vec<usize>,
}

impl<'t> SideLines<'t> {
  fn new(text: &'t str) -> Self {
    Self { text, starts: line_starts(text) }
  }

  /// The line `line`, from 0, without its line end
  fn line(&self, line: u32) -> &'t str {
    line_text(self.text, &self.starts, line).trim_end_matches(['\n', '\r'])
  }

  fn count(&self) -> u32 {
    u32::try_from(self.starts.len()).unwrap_or(u32::MAX)
  }

  /// The lines `lines` with `SAMPLE_CONTEXT` lines around them, each after what `mark` says of it: `| ` for a
  /// line of the mark, two spaces for a line around it; at most `MAX_SAMPLE_LINES`, then how many were left out
  fn printed(&self, lines: Range<u32>, mark: &dyn Fn(u32) -> &'static str) -> Vec<String> {
    let from = lines.start.saturating_sub(SAMPLE_CONTEXT);
    let to = lines.end.saturating_add(SAMPLE_CONTEXT).min(self.count());
    let mut printed: Vec<String> = (from..to)
      .take(MAX_SAMPLE_LINES)
      .map(|line| format!("{}{}", if lines.contains(&line) { mark(line) } else { "  " }, self.line(line)))
      .collect();
    let all = (to.saturating_sub(from)) as usize;
    if all > MAX_SAMPLE_LINES {
      printed.push(format!("… {} more lines", all - MAX_SAMPLE_LINES));
    }
    printed
  }
}

/// A line of a mark, in a printed sample
const fn marked(_: u32) -> &'static str {
  "| "
}

/// The marks of each kind kept for the sample: of the marks offered, in the order of the subjects, every one at a
/// stride that doubles whenever twice the sample is kept, so the sample spreads evenly over the subjects whatever
/// their number and holds the same marks whenever the same subjects are read
struct Sampler {
  size: usize,
  kinds: BTreeMap<&'static str, Reservoir>,
}

struct Reservoir {
  seen: usize,
  stride: usize,
  kept: Vec<SampleMark>,
}

impl Sampler {
  const fn new(size: usize) -> Self {
    Self { size, kinds: BTreeMap::new() }
  }

  /// Offers a mark of `kind`, made by `make` only when it is kept
  fn offer(&mut self, kind: &'static str, make: impl FnOnce() -> SampleMark) {
    let reservoir = self.kinds.entry(kind).or_insert(Reservoir {
      seen: 0,
      stride: 1,
      kept: Vec::new(),
    });
    if reservoir.seen.is_multiple_of(reservoir.stride) {
      reservoir.kept.push(make());
      if reservoir.kept.len() > self.size * 2 {
        let mut index = 0;
        reservoir.kept.retain(|_| {
          index += 1;
          index % 2 == 1
        });
        reservoir.stride *= 2;
      }
    }
    reservoir.seen += 1;
  }

  fn into_sample(self) -> Vec<SampleMark> {
    let size = self.size.max(1);
    self
      .kinds
      .into_values()
      .flat_map(|reservoir| {
        // `size` of them at even steps, or all that were kept
        let kept = reservoir.kept.len();
        let picked: BTreeSet<usize> = (0..size.min(kept)).map(|index| index * kept / size.min(kept)).collect();
        reservoir
          .kept
          .into_iter()
          .enumerate()
          .filter(move |(index, _)| picked.contains(index))
          .map(|(_, mark)| mark)
      })
      .collect()
  }
}

/// What a sampled mark of a file starts with
struct SampleOf<'a> {
  subject: &'a str,
  path: &'a str,
  language: &'static str,
  old: &'a SideLines<'a>,
  new: &'a SideLines<'a>,
}

impl SampleOf<'_> {
  /// A mark of `kind` at the old lines `old` and the new lines `new`, each printed with `mark` before its lines
  fn mark(&self, kind: &'static str, old: Range<u32>, new: Range<u32>, note: String, mark: [&dyn Fn(u32) -> &'static str; 2]) -> SampleMark {
    SampleMark {
      kind,
      subject: self.subject.to_string(),
      path: self.path.to_string(),
      language: self.language,
      old_line: old.start + 1,
      old_count: old.end - old.start,
      new_line: new.start + 1,
      new_count: new.end - new.start,
      note,
      old: self.old.printed(old, mark[0]),
      new: self.new.printed(new, mark[1]),
    }
  }
}

/// Offers the marks of the text file `file` of the subject `measured` to the sample
fn sample_file(sampler: &mut Sampler, subject: &str, measured: &MeasuredSubject, file: &MeasuredFile, diff: &TextDiff, text: &MeasuredText) {
  let (old, new) = (SideLines::new(text.text(false)), SideLines::new(text.text(true)));
  let of = SampleOf {
    subject,
    path: &file.file.path,
    language: language_of(Some(text)),
    old: &old,
    new: &new,
  };
  let marks = &diff.marks;
  let confirmed = |run: usize| marks.subject.as_ref().is_some_and(|subject| subject.confirmed.iter().any(|number| *number as usize == run));

  for (number, (run, proven)) in marks.runs.iter().zip(&text.proven).enumerate() {
    let kind = match run.kind {
      RunKind::Formatting => "formatting",
      RunKind::Comments => "comments",
      RunKind::Renamed if confirmed(number) => "renamedConfirmed",
      RunKind::Renamed if *proven => "renamedProven",
      RunKind::Renamed => "renamedLabel",
    };
    sampler.offer(kind, || {
      let note = format!("quiet: {}, proven: {proven}{}", run.quiet, names_in(run, marks.renames.iter().chain(used_renames(diff))));
      of.mark(kind, run.old_start..run.old_end, run.new_start..run.new_end, note, [&marked, &marked])
    });
  }

  let free = FreeLines::of(diff, &old, &new);
  for moved in &marks.moves {
    let kind = match (free.anchors(moved, &old), moved.edited.is_empty(), moved.len) {
      (false, ..) => "moveUnanchored",
      (true, false, _) => "moveEdited",
      (true, true, 1) => "moveOneLine",
      (true, true, _) => "move",
    };
    sampler.offer(kind, || {
      let note = format!(
        "{} lines, {} edited, indent {}, {} characters in its exact lines, blank lines with it: {} before, {} after",
        moved.len,
        moved.edited.len(),
        moved.indent,
        exact_chars(moved, &old),
        moved.blanks_before,
        moved.blanks_after
      );
      // the lines of the move, and the blank lines that moved with it before and after them
      let edited = |start: u32| {
        move |line: u32| {
          if line.checked_sub(start).is_some_and(|offset| moved.edited.binary_search(&offset).is_ok()) {
            "~ "
          } else {
            "| "
          }
        }
      };
      of.mark(kind, moved.old_lines(), moved.new_lines(), note, [&edited(moved.old_start), &edited(moved.new_start)])
    });
  }

  for (number, rename) in (0u32..).zip(&marks.renames) {
    let proof = marks.subject.as_ref().and_then(|subject| subject.declared.iter().find(|check| check.rename == number));
    let kind = match (rename.local, text.proven_renames.contains(&number)) {
      (true, true) => "renameLocalProven",
      (true, false) => "renameLocal",
      (false, _) => "renameDeclared",
    };
    sampler.offer(kind, || rename_sample(&of, kind, rename, proof.map(|check| check.proof)));
  }
  for used in marks.subject.iter().flat_map(|subject| &subject.used) {
    sampler.offer("renameUsed", || rename_sample(&of, "renameUsed", &used.rename, Some(used.proof)));
  }

  // each move between two files once, where its lines stand now
  let moved_in: &[FileMove] = marks.subject.as_ref().map_or(&[], |subject| &subject.moved_in);
  for moved in moved_in {
    let from = measured.files.get(moved.file as usize);
    let from_text = from.and_then(|from| from.text.as_ref());
    sampler.offer("moveAcross", || {
      let from_lines = SideLines::new(from_text.map_or("", |text| text.text(false)));
      let from_path = from.map_or("", |from| from.file.path.as_str());
      let lines = moved.line..moved.line + moved.len;
      SampleMark {
        note: format!("{} lines from {from_path}:{}", moved.len, moved.line + 1),
        old: from_lines.printed(lines.clone(), &marked),
        ..of.mark("moveAcross", lines, moved.start..moved.start + moved.len, String::new(), [&|_| "", &marked])
      }
    });
  }
}

/// The names another file of the subject declares and this one uses
fn used_renames(diff: &TextDiff) -> impl Iterator<Item = &Rename> {
  diff.marks.subject.iter().flat_map(|subject| subject.used.iter().map(|used| &used.rename))
}

/// The renames with a place in the old lines of `run`, as ` · old -> new` each
fn names_in<'a>(run: &LineRun, renames: impl Iterator<Item = &'a Rename>) -> String {
  renames
    .filter(|rename| rename.old_sites.as_chunks::<3>().0.iter().any(|site| (run.old_start..run.old_end).contains(&site[0])))
    .fold(String::new(), |mut names, rename| {
      let _ = write!(names, " · {} -> {}{}", rename.old, rename.new, if rename.local { " (local)" } else { "" });
      names
    })
}

/// A rename with the lines from its first place to its last one, on both sides
fn rename_sample(of: &SampleOf<'_>, kind: &'static str, rename: &Rename, proof: Option<RenameProof>) -> SampleMark {
  let note = format!(
    "{} -> {}, {} places, {} declared{}",
    rename.old,
    rename.new,
    rename.sites,
    rename.declared.len(),
    proof.map_or_else(String::new, |proof| format!(", {}", proof_name(proof)))
  );
  let at = |sites: &'_ [u32]| {
    let lines: Vec<u32> = sites.as_chunks::<3>().0.iter().map(|site| site[0]).collect();
    move |line: u32| if lines.contains(&line) { "| " } else { "  " }
  };
  let [old, new] = rename.spans();
  of.mark(kind, old, new, note, [&at(&rename.old_sites), &at(&rename.new_sites)])
}

//! The marks of a diff as words (`show`): one line of its blocks, runs, moves, renames, starts and what the other files
//! of its subject say of it, for the tests of the marks, these of the crate (`marks_bindings_test.rs`) and those of a
//! reader that compares the marks of whole files (Branch Deck's `file-diffs`), as `summary_text` words a summary.
//! Behind the feature `marks`, with `marks_bindings`.

use diff_marks::model::{DiffMarks, FileMove, Rename, SubjectMarks};
use line_diff::model::{BlockKind, LineDiff, RunKind, starts_of};
use std::fmt::Write as _;

/// A diff of the comparison with the marks of its pair: what the tests of the marks read, of the values of line-diff
/// and diff-marks. It reads as its diff (`Deref`)
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarkedDiff {
  pub diff: LineDiff,
  pub marks: DiffMarks,
}

impl From<LineDiff> for MarkedDiff {
  /// The diff of the comparison alone, every block a stop
  fn from(diff: LineDiff) -> Self {
    let marks = DiffMarks {
      starts: starts_of(&diff.blocks, &[], &[]),
      ..DiffMarks::default()
    };
    Self { diff, marks }
  }
}

impl std::ops::Deref for MarkedDiff {
  type Target = LineDiff;

  fn deref(&self) -> &LineDiff {
    &self.diff
  }
}

/// A rename as words: its names, `local` for a name bound inside a body, the numbers of the places that declare it,
/// and `line:column` of each place of the old text, then of the new one
pub fn show_rename(rename: &Rename) -> String {
  let places = |sites: &[u32]| -> String {
    let places: Vec<String> = sites.as_chunks::<3>().0.iter().map(|site| format!("{}:{}", site[0], site[1])).collect();
    places.join(" ")
  };
  format!(
    "{}->{}{} declared {:?} at {} / {}",
    rename.old,
    rename.new,
    if rename.local { " local" } else { "" },
    rename.declared,
    places(&rename.old_sites),
    places(&rename.new_sites)
  )
}

/// The blocks, the runs, the moves, the renames and the starts of a diff as one line: `old/new` lines of each block
/// and each run, `old->new` lines of each move
pub fn show(diff: &MarkedDiff) -> String {
  let blocks: Vec<String> = diff
    .blocks
    .iter()
    .map(|block| {
      let kind = match block.kind {
        BlockKind::Changed => "",
        BlockKind::WhitespaceOnly => " pale",
        BlockKind::Ignored => " ignored",
      };
      format!("{}..{}/{}..{}{kind}", block.old_start, block.old_end, block.new_start, block.new_end)
    })
    .collect();
  let runs: Vec<String> = diff
    .marks
    .runs
    .iter()
    .map(|run| {
      let kind = match run.kind {
        RunKind::Formatting => "formatting",
        RunKind::Comments => "comments",
        RunKind::Renamed => "renamed",
      };
      format!(
        "{}..{}/{}..{} {kind}{}",
        run.old_start,
        run.old_end,
        run.new_start,
        run.new_end,
        if run.quiet { " quiet" } else { "" }
      )
    })
    .collect();
  let moves: Vec<String> = diff
    .marks
    .moves
    .iter()
    .map(|line_move| {
      let (edited, blanks) = (line_move.edited.len(), (line_move.blanks_before, line_move.blanks_after));
      format!(
        "{}..{}->{}..{}{}{}{}",
        line_move.old_start,
        line_move.old_start + line_move.len,
        line_move.new_start,
        line_move.new_start + line_move.len,
        if blanks == (0, 0) {
          String::new()
        } else {
          format!(" blanks {}+{}", blanks.0, blanks.1)
        },
        if line_move.indent == 0 {
          String::new()
        } else {
          format!(" indent {:+}", line_move.indent)
        },
        if edited == 0 { String::new() } else { format!(" edited {edited}") },
      )
    })
    .collect();
  // the moves only where a diff has them: most cases of the runs have none
  let moves = if moves.is_empty() { String::new() } else { format!(" moves [{}]", moves.join(", ")) };
  // the renames likewise
  let renames: Vec<String> = diff.marks.renames.iter().map(show_rename).collect();
  let renames = if renames.is_empty() {
    String::new()
  } else {
    format!(" renames [{}]", renames.join(", "))
  };
  let subject = diff.marks.subject.as_deref().map_or_default(show_subject);
  format!(
    "blocks [{}] runs [{}]{moves}{renames} starts {:?}{subject}",
    blocks.join(", "),
    runs.join(", "),
    diff.marks.starts
  )
}

/// What the other files of a subject say of a file, each part only where it says something: the lines that moved
/// out of it and into it (`lines->file:line`), the names another file declares (as a rename, with the file and the
/// line of the declaration and its proof), the proof of each rename it declares, and the runs the index confirmed
fn show_subject(subject: &SubjectMarks) -> String {
  let moved = |moves: &[FileMove]| -> Vec<String> {
    moves
      .iter()
      .map(|moved| format!("{}..{}->{}:{}", moved.start, moved.start + moved.len, moved.file, moved.line))
      .collect()
  };
  let used: Vec<String> = subject
    .used
    .iter()
    .map(|used| format!("{} in {}:{} {:?}", show_rename(&used.rename), used.file, used.line, used.proof))
    .collect();
  let declared: Vec<String> = subject.declared.iter().map(|check| format!("{} {:?}", check.rename, check.proof)).collect();
  let confirmed: Vec<String> = subject.confirmed.iter().map(u32::to_string).collect();
  [
    ("moved out", moved(&subject.moved_out)),
    ("moved in", moved(&subject.moved_in)),
    ("used", used),
    ("declared", declared),
    ("confirmed", confirmed),
  ]
  .into_iter()
  .filter(|(_, items)| !items.is_empty())
  .fold(String::new(), |mut shown, (name, items)| {
    write!(shown, " {name} [{}]", items.join(", ")).unwrap();
    shown
  })
}

//! The marks of a diff (openspec/specs/commit/quiet-changes/spec.md; docs/design/diff-view.md, "Marks"): the runs of
//! lines that changed in their formatting, their comments or a renamed name alone, the names the change renamed, the
//! lines that moved within the file, and the line the reading of each block starts at
//! (`text_diff::model::DiffMarks`). They change no block of the diff.
//!
//! - **Proof, not a guess.** A run is quiet only when the tokens of its lines are the same on both sides
//!   (`code_symbols::tokens`, openspec/specs/commit/equal-by-tokens/spec.md): the same tokens parse to the same tree, so
//!   the lines are the same program. A pair whose sides are not read by one grammar, a text without tokens and a side
//!   over the size the inner ranges are compared for have no runs, and every block of them stays a stop.
//! - **What is never marked**: a block the policy cut off (`Ignored`); a block with a side of no lines; lines that are
//!   blank on both sides. A pale block (`WhitespaceOnly`) passes the same test of tokens as any other, with no
//!   shortcut: a space inside a string is equal under the `Ignore` policy and is no formatting.
//! - **Before the tokens**, a block is asked a question of its characters alone (`may_have_run`,
//!   `marks_renames::may_hold_rename`) that every block with a run answers with yes, so a file whose blocks all answer
//!   no is never parsed for its marks.
//! - **Limits by work, never by time**: the size of a side (`MAX_INNER_BYTES`, `MAX_INNER_LINES` of the comparison),
//!   the tokens of a block (`MAX_BLOCK_TOKENS`), the tokens that differ in it (`MAX_TOKEN_EDITS`), the comparisons of
//!   a file (`MAX_FILE_TOKEN_WORK`), the line pairs of the question before the tokens (`MAX_LINE_PAIRS`) and the names
//!   asked of a file (`MAX_RENAME_WORK`). The runs read the blocks and the tokens, never the inner ranges, so a diff
//!   whose inner ranges ran out of their time (`line_level_only` within the size) has the runs of its blocks: the
//!   same blocks have the same marks on every machine and under every load.
//! - **Quiet** is decided for a kind in one place, `is_quiet`: a kind is switched on there once a sample of its runs
//!   of real ranges was read and held no change of behaviour (`brd commits --mark-stats`, `marks_measure`;
//!   docs/design/diff-view.md, "Measured on real history"). The runs of a kind that is off are labels:
//!   `PairMarks::proven` names those whose proof holds (`has_proof`), and `PairMarks::as_proven` gives the marks as
//!   they would be: what such a measurement reads. Formatting and the renamed names are on; comments have no proof.
//! - **Renames** (`marks_renames`) are read from the names the alignment of the tokens put at one place and proven by
//!   the bindings of both texts; the blocks that hold a place of one are then read again with the two names read as
//!   one, which adds the runs of the lines that hold a place and takes no run the first reading found: a line that
//!   changed in its formatting alone is a line of formatting beside a rename too. A name bound inside a body is a
//!   rename only where a line changed in it alone, and its runs are proven, and so quiet, only for a value of its
//!   body alone in a member nothing else of which changed but formatting, comments and such names. A name that is
//!   not local says whether the code that declares it changed in more than names (`PairMarks::redeclared`): the
//!   index proves the uses of no such name.
//! - **What the pair leaves** (`PairMarks::undeclared`, `pending`, `proven_renames`; `leftovers`): the names that
//!   changed at every place of the file, which declares none of them, with the runs their blocks would have were
//!   they renames. The pass over the files of the subject (`marks_subject`) knows whether another file declares
//!   them; nothing of them is a mark of the pair.
//! - **Moves** (`text_diff::moves::find_moves`) need no grammar: every pair of texts has them, from its lines alone,
//!   after the runs, whose lines no move takes. A move is never quiet: its new lines are in no run, so the block they
//!   stand in always has a start; its old lines are nothing to read where they left (`starts_of`). Whether the
//!   indentation of the file has a meaning (`code_symbols::tokens::indentation_matters`, of either path) decides what
//!   a line at another indent is in a move, so it is in the key of the diff (`service::diff_options`).
//!
//! `marks_runs` reads the runs of one block from its tokens, `marks_renames` the renames of a file.

use crate::leftovers::PendingBlock;
use crate::marks_renames::{Candidates, may_hold_rename, renames_of};
use crate::marks_runs::{Asked, BlockLines, NO_RENAME, runs_of_block};
use crate::service::{FileDiffs, fits_inner};
use code_symbols::highlight::{Grammar, grammar_of};
use code_symbols::tokens::{Tokens, indentation_matters, is_blank, tokens};
use std::cell::OnceCell;
use std::ops::Range;
use std::sync::Arc;
use text_diff::model::{BlockKind, DiffBlock, DiffMarks, LineMove, LineRun, Rename, RunKind, TextDiff, starts_of};
use text_diff::moves::find_moves;
use text_diff::policy::WhitespacePolicy;

/// A side of a block with more tokens of code is not compared token by token: a block of some three thousand lines
pub const MAX_BLOCK_TOKENS: usize = 1 << 15;
/// A block whose sides differ in more tokens of code than this, beside the tokens only one side has, has no run cut
/// out of it: the comparison of its tokens gives up after work bound by the square of this
pub const MAX_TOKEN_EDITS: usize = 2_048;
/// The work of the token comparisons of one file, in steps of the comparison (the tokens of a block times the edits
/// allowed in it): one block at both its limits. A block that what is left does not pay for is only asked whole
pub const MAX_FILE_TOKEN_WORK: u64 = 2 * (MAX_BLOCK_TOKENS as u64) * (MAX_TOKEN_EDITS as u64);
/// The question before the tokens compares at most this many pairs of lines of a block; a block with more is parsed
pub const MAX_LINE_PAIRS: usize = 4_096;
/// The work of the names of one file, for its renames: a resolution of a name, one per spelling asked and one more
/// per binding, costs the tokens of its text where it walks the tree (Java, C#, Starlark) and an eighth of them where
/// it reads the names of one spelling (`code_symbols::navigate::resolution_work`). A Java file of 2,500 lines has
/// some twenty-six resolutions of each side (thirteen renamed locals), a Kotlin file of that size some two hundred,
/// a file of 250 lines ten times as many; a place past them is not answered, and its name is no rename. Set by
/// what the names of a file may cost before its content goes out (`benches/marks_bench.rs`, "renamed locals at the
/// limit of their work")
pub const MAX_RENAME_WORK: u64 = 1 << 20;

/// The tokens that memory keeps, weighed by their size (64 bytes a token): the sides of some three hundred files of
/// 200 lines, for the next commit of a branch and another whitespace policy
pub const TOKENS_CACHE_BYTES: u64 = 32 << 20;

/// The memory the tokens of a blob take in the cache
pub(crate) fn tokens_weight(tokens: Option<&Tokens>) -> u32 {
  u32::try_from(tokens.map_or(0, Tokens::heap_bytes).saturating_add(64)).unwrap_or(u32::MAX)
}

impl FileDiffs {
  /// The marks of `diff`, the diff of a file whose sides are at `paths` (the old one first), have the blob `ids` and
  /// the `texts`. The tokens of a side are read once per blob and grammar and kept in memory: the next pair with the
  /// blob (the next commit of a branch, another whitespace policy) parses nothing
  pub(crate) fn marks(&self, diff: &TextDiff, policy: WhitespacePolicy, paths: [&str; 2], ids: [Option<&str>; 2], texts: [&str; 2]) -> PairMarks {
    self.marks_told(diff, policy, paths, ids, texts, &|_| {})
  }

  /// `marks`, telling `asked` of each side whose tokens the marks read (`true` the new one): a file none is read of
  /// answered the question before the tokens with no at every block
  pub(crate) fn marks_told(&self, diff: &TextDiff, policy: WhitespacePolicy, paths: [&str; 2], ids: [Option<&str>; 2], texts: [&str; 2], asked: &dyn Fn(bool)) -> PairMarks {
    let grammar = pair_grammar(paths[0], paths[1]);
    marks_of(diff, texts[0], texts[1], grammar, policy, pair_indented(paths[0], paths[1]), |new_side| {
      asked(new_side);
      let (side, grammar) = (usize::from(new_side), grammar?);
      match ids[side] {
        Some(id) => self
          .marks_memory
          .tokens
          .get_with((grammar.id(), id.to_string()), || tokens(grammar, texts[side]).map(Arc::new)),
        None => tokens(grammar, texts[side]).map(Arc::new),
      }
    })
  }
}

/// Whether a run of `kind` is quiet: a reader passes it, the navigation of changes does not stop in it, the Inline
/// view shows its new lines alone. The one place that decides it, for every kind: a kind that is not quiet here is a
/// label wherever it is marked, and one that is, is quiet where its lines are proven (`has_proof`).
///
/// A kind is switched on by a measurement, never by its proof alone: a sample of its proven runs of real ranges of a
/// large repository, read by a person, with no change of behaviour among them (`PairMarks::proven` names the runs,
/// `brd commits --mark-stats --mark-sample` prints them). Measured (docs/design/diff-view.md, "Measured on real
/// history"): 200 runs of formatting and 145 runs of a renamed local held none, the latter once a rename is proven
/// only where nothing but quiet lines changed between its places (`marks_renames::Renames::prove`): before that
/// rule, the lines of a variable that another one took the place of read as a rename
pub const fn is_quiet(kind: RunKind) -> bool {
  match kind {
    RunKind::Formatting | RunKind::Renamed => true,
    RunKind::Comments => false,
  }
}

/// Whether a run of `kind` can be proven to change no behaviour: by the tokens of its lines for formatting, for a
/// renamed name by places of a value of its body alone (`marks_renames`). No proof says that of a comment: a tool may
/// read it. What `is_quiet` may switch on: a run of such a kind whose proof holds is quiet then, and one of
/// `PairMarks::proven` until then
pub const fn has_proof(kind: RunKind) -> bool {
  match kind {
    RunKind::Formatting | RunKind::Renamed => true,
    RunKind::Comments => false,
  }
}

/// The grammar both sides of a pair are read by, from their paths (`old_path` of a rename): `None` when a side has
/// none or the two differ. Only such a pair has runs: the tokens of two grammars prove nothing of each other
pub fn pair_grammar(old_path: &str, new_path: &str) -> Option<Grammar> {
  let grammar = grammar_of(new_path)?;
  (old_path == new_path || grammar_of(old_path) == Some(grammar)).then_some(grammar)
}

/// Whether the indentation of a side of a pair has a meaning, from their paths (`old_path` of a rename): a line of
/// a move at another indent than the others is then an edited line of it
pub fn pair_indented(old_path: &str, new_path: &str) -> bool {
  indentation_matters(new_path) || (old_path != new_path && indentation_matters(old_path))
}

/// The marks of a pair of texts, and what the pair leaves to a reader of the other files of its change and to the
/// measurement of its proofs
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PairMarks {
  pub marks: DiffMarks,
  /// The names that changed at every place of the file and are declared in none of them (`Rename::declared` is
  /// empty), by their first place: nothing of them is marked, since only the file of the declaration says what was
  /// renamed. For the pass over the files of a change, which has the declaration
  pub undeclared: Vec<Rename>,
  /// The blocks that hold a place of an `undeclared` name, read with those names as one too: the runs of the kind
  /// `Renamed` they have once another file of the subject declares the names. No run of them shares a line with a
  /// move
  pub pending: Vec<PendingBlock>,
  /// The renames of `marks.renames`, by their numbers, whose places the scopes of the file prove: a value of its
  /// body alone
  pub proven_renames: Vec<u32>,
  /// The renames of `marks.renames`, by their numbers, that are not local and whose declaration changed in more
  /// than names (`marks_renames::Renames::read_declarations`): the code that declares the new name is not the code
  /// that declared the old one, so the two are two things whatever the index says of their uses
  pub redeclared: Vec<u32>,
  /// The runs of `marks.runs`, by their numbers, that are proven and are labels all the same, since their kind is not
  /// switched on (`is_quiet`): what the measurement that decides the kind reads a sample of. None while every kind
  /// with a proof is on
  pub proven: Vec<u32>,
}

impl PairMarks {
  /// The marks as they are once every kind with a proof is switched on: the proven runs quiet, and the starts of
  /// `blocks`, the blocks of the diff, that follow from them. For the measurement that decides `is_quiet` and for the
  /// tests of a proof; no view shows them
  #[must_use]
  pub fn as_proven(&self, blocks: &[DiffBlock]) -> DiffMarks {
    let mut marks = self.marks.clone();
    for run in &self.proven {
      if let Some(run) = marks.runs.get_mut(*run as usize) {
        run.quiet = true;
      }
    }
    marks.starts = starts_of(blocks, &marks.runs, &marks.moves);
    marks
  }
}

/// The marks of `diff`, the diff of `old` and `new`, both read by `grammar` (`pair_grammar`; `None` has no runs and no
/// renames). `tokens` reads the tokens of a side, `false` the old one and `true` the new one, `None` for a text
/// without tokens; each side is asked once at most, and only when a block may have a run. `policy` is the whitespace
/// policy the diff was made under and `indented` whether the indentation of the file has a meaning
/// (`pair_indented`): what the whitespace at the ends of a line is to a move. The moves and `starts` are filled for
/// every diff.
///
/// A pair with a side over the size the inner ranges are compared for has no runs: a limit of work. What the time
/// did to the diff changes nothing here: the runs read its blocks and the tokens, the moves its blocks and the lines
/// of the texts, never its inner ranges, so a diff that is by lines alone for lack of time has the marks of its
/// blocks
pub fn marks_of(
  diff: &TextDiff,
  old: &str,
  new: &str,
  grammar: Option<Grammar>,
  policy: WhitespacePolicy,
  indented: bool,
  tokens: impl Fn(bool) -> Option<Arc<Tokens>>,
) -> PairMarks {
  let PairRuns {
    mut runs,
    renames,
    undeclared,
    mut pending,
    proven_renames,
    redeclared,
  } = match grammar {
    Some(grammar) if fits_inner(old, diff.old_line_count) && fits_inner(new, diff.new_line_count) => {
      runs_of(diff, &SideText::new(grammar, old), &SideText::new(grammar, new), true, &tokens)
    }
    _ => PairRuns::default(),
  };
  // the one place a kind is asked: a proven run of a kind that is not switched on is a label
  let mut proven = Vec::new();
  for (index, run) in (0u32..).zip(&mut runs) {
    if run.quiet && !is_quiet(run.kind) {
      run.quiet = false;
      proven.push(index);
    }
  }
  // after the runs: a line of a run is in no move
  let moves = find_moves(old, new, &diff.blocks, &runs, policy, indented);
  // and a run that waits for another file takes no line of a move either: the moves are read once, here
  without_moved_lines(&mut pending, &moves);
  PairMarks {
    marks: DiffMarks {
      starts: starts_of(&diff.blocks, &runs, &moves),
      runs,
      moves,
      renames,
      subject: None,
    },
    undeclared,
    pending,
    proven_renames,
    redeclared,
    proven,
  }
}

/// Takes the runs that share a line with one of `moves` out of `pending`, and the blocks that hold no run then
pub(crate) fn without_moved_lines(pending: &mut Vec<PendingBlock>, moves: &[LineMove]) {
  if pending.is_empty() || moves.is_empty() {
    return;
  }
  let meets = |lines: Range<u32>, moved: Range<u32>| lines.start < moved.end && moved.start < lines.end;
  for block in pending.iter_mut() {
    block.runs.retain(|pending| {
      let run = &pending.run;
      !moves
        .iter()
        .any(|moved| meets(run.old_start..run.old_end, moved.old_lines()) || meets(run.new_start..run.new_end, moved.new_lines()))
    });
  }
  pending.retain(|block| !block.runs.is_empty());
}

/// The runs of a pair, each quiet when its lines are proven (`has_proof`), the renames some of them were cut by, and
/// what the pair leaves to the other files of its subject
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct PairRuns {
  pub(crate) runs: Vec<LineRun>,
  pub(crate) renames: Vec<Rename>,
  pub(crate) undeclared: Vec<Rename>,
  pub(crate) pending: Vec<PendingBlock>,
  /// The renames of `renames`, by their numbers, whose places are proven
  pub(crate) proven_renames: Vec<u32>,
  /// The renames of `renames`, by their numbers, that are not local and whose declaration changed in more than
  /// names
  pub(crate) redeclared: Vec<u32>,
}

/// The runs of the blocks of `diff` and its renames; `ask_first` leaves a block out that answers no before the tokens
/// are read (`may_have_run`), which changes no answer: without it every block of two sides is read from the tokens.
///
/// The blocks are read twice at most: once for their runs of formatting and of comments and for the names the
/// alignment of their tokens put at one place (`read_blocks`), then, where those names are a rename
/// (`marks_renames`), the blocks that hold a place of one again, with the two names of each place read as one. The
/// second reading adds runs, those of the lines that changed in such a name alone first of all, and takes none:
/// every run the first reading found lies in a run of its kind. The blocks that hold a place of a name the file
/// declares none of are read a third time, with those names read as one too, for the reader of the other files
/// (`Renames::pending`); that reading changes no run of the pair. Every reading spends the one work of the file
/// (`MAX_FILE_TOKEN_WORK`). A run comes out of them quiet when its lines are proven; whether its kind is quiet,
/// `marks_of` asks
pub(crate) fn runs_of(diff: &TextDiff, old: &SideText<'_>, new: &SideText<'_>, ask_first: bool, tokens: &dyn Fn(bool) -> Option<Arc<Tokens>>) -> PairRuns {
  let FirstReading {
    mut runs,
    candidates,
    tokens,
    mut work,
  } = read_blocks(diff, old, new, ask_first, tokens);
  let mut pair = PairRuns::default();
  if let Some([old_tokens, new_tokens]) = &tokens
    && !candidates.is_empty()
  {
    let mut found = renames_of(old.grammar, [old.text, new.text], [old_tokens, new_tokens], &diff.blocks, &candidates);
    // a name bound inside a body none of whose lines changed in it alone is no rename: the blocks are read again
    // without it, from their first reading and with the work it left
    let (first, paid) = (runs.clone(), work);
    loop {
      found.cut(&diff.blocks, [old_tokens, new_tokens], &has_text(old, new), &mut work, &mut runs);
      if !found.keep_the_named(&runs, old_tokens) {
        break;
      }
      runs.clone_from(&first);
      work = paid;
    }
    let declarations = found.with_declarations(&diff.blocks, [old_tokens, new_tokens], &has_text(old, new), &mut work);
    found.prove(&diff.blocks, [old_tokens, new_tokens], &mut runs, &declarations);
    found.read_declarations(&diff.blocks, [old_tokens, new_tokens], &runs, &declarations);
    pair.pending = found.pending(&diff.blocks, [old_tokens, new_tokens], &has_text(old, new), &mut work);
    pair.proven_renames = (0u32..).zip(&found.found).filter(|(_, found)| found.proven).map(|(number, _)| number).collect();
    pair.redeclared = (0u32..).zip(&found.found).filter(|(_, found)| found.redeclared).map(|(number, _)| number).collect();
    pair.renames = found.found.into_iter().map(|found| found.rename).collect();
    pair.undeclared = found.undeclared.into_iter().map(|found| found.rename).collect();
  }
  pair.runs = runs.into_iter().flatten().collect();
  pair
}

/// The blocks of a diff after their first reading, from the tokens of the two texts as they are
pub(crate) struct FirstReading {
  /// The runs of each block: of formatting and of comments
  pub(crate) runs: Vec<Vec<LineRun>>,
  /// The names the alignment of the tokens put at one place
  candidates: Candidates,
  /// The tokens of the two sides, when a block was read from them and their lines are the lines of the blocks
  tokens: Option<[Arc<Tokens>; 2]>,
  /// What the reading left of the work of the file
  work: u64,
}

/// The first reading of the blocks of `diff` (`runs_of`, which says what `ask_first` and `tokens` are)
pub(crate) fn read_blocks(diff: &TextDiff, old: &SideText<'_>, new: &SideText<'_>, ask_first: bool, tokens: &dyn Fn(bool) -> Option<Arc<Tokens>>) -> FirstReading {
  let mut runs: Vec<Vec<LineRun>> = vec![Vec::new(); diff.blocks.len()];
  let mut candidates = Candidates::default();
  let mut read: Option<Option<[Arc<Tokens>; 2]>> = None;
  let mut work = MAX_FILE_TOKEN_WORK;
  let has_text = has_text(old, new);
  let asked = Asked {
    has_text: &has_text,
    renamed: NO_RENAME,
  };
  for (block, runs) in diff.blocks.iter().zip(&mut runs) {
    let lines = BlockLines {
      old: block.old_start..block.old_end,
      new: block.new_start..block.new_end,
    };
    if block.kind == BlockKind::Ignored || lines.old.is_empty() || lines.new.is_empty() {
      continue;
    }
    // asked of every block that is read too: a block that answers no gives no name
    let renamed = may_hold_rename(old.lines(lines.old.clone()), new.lines(lines.new.clone()));
    if ask_first && !renamed && !may_have_run(old, new, &lines) {
      continue;
    }
    let read = read.get_or_insert_with(|| {
      let read = [tokens(false)?, tokens(true)?];
      // the lines of the tokens are the lines of the blocks, or nothing is proven
      (read[0].line_count() == diff.old_line_count && read[1].line_count() == diff.new_line_count).then_some(read)
    });
    let Some([old_tokens, new_tokens]) = read else { break };
    let cut = runs_of_block(old_tokens, new_tokens, &lines, &asked, &mut work, &mut |old_token, new_token| {
      if renamed {
        candidates.take(old_token, new_token);
      }
    });
    *runs = cut.unwrap_or_default();
  }
  FirstReading {
    runs,
    candidates,
    tokens: read.flatten(),
    work,
  }
}

/// Whether lines of a side of a pair (`true` the new one) hold a character that is no blank
fn has_text<'t>(old: &'t SideText<'t>, new: &'t SideText<'t>) -> impl Fn(bool, Range<u32>) -> bool + 't {
  move |new_side: bool, lines: Range<u32>| !(if new_side { new } else { old }).is_blank(lines)
}

/// One side of a pair before its tokens: its text, the grammar it is read by and its lines as the view counts them
pub(crate) struct SideText<'t> {
  grammar: Grammar,
  text: &'t str,
  /// The byte each line starts at (`\n`, `\r\n` and a lone `\r` end a line), made when first asked
  line_starts: OnceCell<Vec<u32>>,
  /// For each form of the block comments of the grammar (`block_comments`), where a comment that opened before may
  /// be open, and where not again (`open_comments`)
  open_comments: OnceCell<Vec<Vec<(u32, bool)>>>,
  /// For each line, whether what starts a comment stands before it since the last `\n` (`after_comment_start`)
  after_comment_start: OnceCell<Vec<bool>>,
}

impl<'t> SideText<'t> {
  pub(crate) const fn new(grammar: Grammar, text: &'t str) -> Self {
    Self {
      grammar,
      text,
      line_starts: OnceCell::new(),
      open_comments: OnceCell::new(),
      after_comment_start: OnceCell::new(),
    }
  }

  fn line_starts(&self) -> &[u32] {
    self.line_starts.get_or_init(|| {
      let bytes = self.text.as_bytes();
      let mut starts = vec![0];
      for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' || (*byte == b'\r' && bytes.get(index + 1) != Some(&b'\n')) {
          starts.push(u32::try_from(index + 1).unwrap_or(u32::MAX));
        }
      }
      starts
    })
  }

  /// The text of `lines` (from 0, the end excluded), their line ends included
  pub(crate) fn lines(&self, lines: Range<u32>) -> &'t str {
    let starts = self.line_starts();
    let at = |line: u32| starts.get(line as usize).map_or(self.text.len(), |start| *start as usize);
    let start = at(lines.start);
    self.text.get(start..at(lines.end).max(start)).unwrap_or_default()
  }

  /// Whether `lines` hold nothing but blanks (`code_symbols::tokens::is_blank`)
  pub(crate) fn is_blank(&self, lines: Range<u32>) -> bool {
    self.lines(lines).chars().all(is_blank)
  }

  /// Each of `lines` that holds a character that is no blank
  fn lines_with_text(&self, lines: Range<u32>) -> Vec<&'t str> {
    lines.map(|line| self.lines(line..line + 1)).filter(|line| !line.chars().all(is_blank)).collect()
  }

  /// Whether a block comment that opened before `line` may still be open at its start, in any form of the block
  /// comments of the grammar
  fn in_open_comment(&self, line: u32) -> bool {
    let Some(start) = self.line_starts().get(line as usize) else { return false };
    let forms = self
      .open_comments
      .get_or_init(|| block_comments(self.grammar).iter().map(|(open, nests)| open_comments(self.text, *open, *nests)).collect());
    forms.iter().any(|changes| {
      let before = changes.partition_point(|(at, _)| at <= start);
      before > 0 && changes[before - 1].1
    })
  }

  /// Whether what starts a comment of the grammar stands before `line` since the last `\n`. A line of the view ends
  /// at a lone `\r` too, where a lexer may read a line comment on to the `\n` (Rust's does): the comment then has
  /// characters in the lines after the `\r`, which hold nothing that starts one. No for every line that starts
  /// after a `\n`, so a text without a lone `\r` is only read for its line ends
  fn after_comment_start(&self, line: u32) -> bool {
    let after = self.after_comment_start.get_or_init(|| {
      let (starts, bytes) = (self.line_starts(), self.text.as_bytes());
      let mut after = vec![false; starts.len()];
      for index in 1..starts.len() {
        let (previous, start) = (starts[index - 1] as usize, starts[index] as usize);
        // the line before ended at a lone `\r`: what it holds, or what stands before it, may run into this line
        after[index] = bytes.get(start - 1) == Some(&b'\r') && (after[index - 1] || self.text.get(previous..start).is_some_and(|text| holds_comment_start(self.grammar, text)));
      }
      after
    });
    after.get(line as usize).copied().unwrap_or_default()
  }
}

/// The question a block is asked before the tokens of its file are read, beside the one of a rename
/// (`marks_renames::may_hold_rename`): whether its lines may hold a run of formatting or of comments. Every block with
/// such a run answers yes (the tests ask every block both ways), from its characters alone:
///
/// - a run of formatting has the same text on both sides without its blanks, so the first line of it that is not
///   blank starts on one side as it does on the other, and its last such line ends as the other
///   (`may_hold_formatting`);
/// - a run of comments has a comment in its lines (`may_hold_comment`).
pub(crate) fn may_have_run(old: &SideText<'_>, new: &SideText<'_>, lines: &BlockLines) -> bool {
  may_hold_formatting(old, new, lines) || may_hold_comment(old, lines.old.clone()) || may_hold_comment(new, lines.new.clone())
}

/// Whether a range of the old lines and a range of the new ones may be the same text without their blanks, with a
/// character that is no blank: some old line starts as some new line does (one is the start of the other, their
/// blanks aside) and some old line ends as some new line does. Yes without the comparison for a block of more than
/// `MAX_LINE_PAIRS` pairs of lines
fn may_hold_formatting(old: &SideText<'_>, new: &SideText<'_>, lines: &BlockLines) -> bool {
  let (old, new) = (old.lines_with_text(lines.old.clone()), new.lines_with_text(lines.new.clone()));
  if old.is_empty() || new.is_empty() {
    return false;
  }
  if old.len().saturating_mul(new.len()) > MAX_LINE_PAIRS {
    return true;
  }
  let pairs = || old.iter().flat_map(|old| new.iter().map(move |new| (*old, *new)));
  pairs().any(|(old, new)| one_starts_the_other(solids(old), solids(new))) && pairs().any(|(old, new)| one_starts_the_other(solids(old).rev(), solids(new).rev()))
}

/// The characters of a line that are no blanks
fn solids(line: &str) -> impl DoubleEndedIterator<Item = char> {
  line.chars().filter(|character| !is_blank(*character))
}

/// Whether one of two sequences is the start of the other
fn one_starts_the_other(mut first: impl Iterator<Item = char>, mut second: impl Iterator<Item = char>) -> bool {
  loop {
    match (first.next(), second.next()) {
      (Some(first), Some(second)) if first == second => {}
      (Some(_), Some(_)) => return false,
      _ => return true,
    }
  }
}

/// What starts a comment of `grammar` in a line: every comment of it starts with one of these
const fn comment_starts(grammar: Grammar) -> &'static [&'static str] {
  match grammar {
    Grammar::Starlark | Grammar::StarlarkBuild => &["#"],
    // the HTML-like comments of a script too
    Grammar::TypeScript | Grammar::Tsx | Grammar::JavaScript => &["//", "/*", "#!", "<!--", "-->"],
    Grammar::Kotlin | Grammar::KotlinScript | Grammar::Java | Grammar::Rust => &["//", "/*", "#!"],
    // a comment of Razor (`@* … *@`) is a comment to Roslyn, and an error
    Grammar::CSharp => &["//", "/*", "#!", "@*"],
  }
}

/// Whether `text` holds what starts a comment of `grammar`
fn holds_comment_start(grammar: Grammar, text: &str) -> bool {
  comment_starts(grammar).iter().any(|start| text.contains(start))
}

/// The forms of the block comments of `grammar`: the two characters that open one, which close it in reverse (`/*`
/// and `*/`, the `@*` and `*@` of Razor), and whether a comment may hold another (`/* a /* b */ c */` is one comment
/// of Kotlin and of Rust). A grammar that is not known to end a comment at its first end nests: the answer that
/// leaves no comment out
const fn block_comments(grammar: Grammar) -> &'static [([u8; 2], bool)] {
  const SLASH: [u8; 2] = *b"/*";
  const RAZOR: [u8; 2] = *b"@*";
  match grammar {
    Grammar::Starlark | Grammar::StarlarkBuild => &[],
    Grammar::Kotlin | Grammar::KotlinScript | Grammar::Rust => &[(SLASH, true)],
    Grammar::Java | Grammar::TypeScript | Grammar::Tsx | Grammar::JavaScript => &[(SLASH, false)],
    Grammar::CSharp => &[(SLASH, false), (RAZOR, false)],
  }
}

/// Whether a comment may have a character in `lines` of `side`: a comment starts in them, one that started before
/// them on their line of the lexer runs into them (`SideText::after_comment_start`), or a block comment that opened
/// before them may still be open at their start. Yes for every line a comment token of the grammar is in
pub(crate) fn may_hold_comment(side: &SideText<'_>, lines: Range<u32>) -> bool {
  holds_comment_start(side.grammar, side.lines(lines.clone())) || side.after_comment_start(lines.start) || side.in_open_comment(lines.start)
}

/// The places of `text` where the answer to "may a block comment that opened before be open here" changes, each with
/// the answer from it on, in order; no before the first. `open` are the two characters that open the comment, which
/// close it in reverse; below they are written `/*` and `*/`. Without a lexer: yes whenever one is open, sometimes
/// when none is (a `/*` inside a string).
///
/// - Where comments do not nest (`nests` is false), a comment that is open at a place opened at a `/*` with no `*/`
///   after it, so the last `/*` of the text before the place has none after it either.
/// - Where they nest, between the `/*` that opened it and the place the lexer read as many `*/` as `/*` at most. Read
///   as text, the `/*` between them are at least the lexer's; the `*/` are the lexer's but for one that shares its
///   `*` with a `/*` before it (`/*/`). So the comment may be open when, after some `/*`, the text has no more `*/`
///   than `/*` and `/*/` together.
///
/// `least` is that difference for the `/*` that has the smallest one
fn open_comments(text: &str, open: [u8; 2], nests: bool) -> Vec<(u32, bool)> {
  let bytes = text.as_bytes();
  let close = [open[1], open[0]];
  let mut changes: Vec<(u32, bool)> = Vec::new();
  let mut least: Option<i64> = None;
  let mut index = 0;
  while index + 1 < bytes.len() {
    let pair = [bytes[index], bytes[index + 1]];
    let step = if pair == close {
      // the next character may start a `/*`
      least = least.map(|least| least + 1);
      1
    } else if pair == open {
      // each `/*` before it counts this one; of a `/*/` its `*/` too and the `/*/` itself, one less in all, so the
      // `*/` is passed here. This one counts what follows its two characters: nothing yet
      least = Some(least.filter(|_| nests).map_or(0, |least| (least - 1).min(0)));
      2
    } else {
      index += 1;
      continue;
    };
    // the answer holds from after the two characters: a line that starts with `*/` starts inside the comment
    let open = least.is_some_and(|least| least <= 0);
    if open != changes.last().is_some_and(|(_, open)| *open) {
      changes.push((u32::try_from(index + 2).unwrap_or(u32::MAX), open));
    }
    index += step;
  }
  changes
}

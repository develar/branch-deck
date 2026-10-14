//! Which commits depend on which, approximately, from the zero-context hunks of `diff_scan`.
//!
//! Every line that a commit of the range adds is owned by that commit, and the owned line ranges of each file are
//! carried forward through the later commits in the coordinates of the newest version. A later commit depends on an
//! earlier one when one of its hunks removes or replaces an owned line or is adjacent to one (git's three-way merge
//! conflicts on adjacent changes too): `-a,b` with `b > 0` touches lines `a-1 ..= a+b`, a pure insertion after line `a`
//! touches lines `a` and `a+1`. Besides lines:
//! - a change to a file created in the range depends on the commit that created it;
//! - re-adding a file deleted in the range depends on the commit that deleted it;
//! - a binary file or a type change depends on the previous commit that changed the file;
//! - renames are followed (`-M`), copies are not.
//!
//! The approximation sees only text: it misses semantic dependencies (a call to a function another commit added in
//! another file) and over-reports independent edits that happen to be adjacent.

use crate::diff_scan::CommitDiff;
use object_cache::commit_diff::{FileChange, Hunk};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// `from` depends on `to` (an older commit), through `files`; both are positions in the input
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
  pub from: usize,
  pub to: usize,
  pub files: Vec<String>,
}

/// Lines `start..end` (1-based, end exclusive) of the current version, added by the commit at `owner`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
  start: u32,
  end: u32,
  owner: usize,
}

#[derive(Debug, Default)]
struct FileState {
  spans: Vec<Span>,
  created_by: Option<usize>,
  deleted_by: Option<usize>,
  last_writer: Option<usize>,
}

/// The dependency edges between `commits` (oldest first), ordered by `from`, then `to`
pub fn compute_dependencies(commits: &[CommitDiff]) -> Vec<Dependency> {
  let mut files: HashMap<String, FileState> = HashMap::new();
  // (from, to) -> files
  let mut edges: BTreeMap<(usize, usize), BTreeSet<String>> = BTreeMap::new();
  for (index, commit) in commits.iter().enumerate() {
    for change in &commit.files {
      let source = change.old_path.as_deref().filter(|_| change.status == 'R').unwrap_or(&change.path);
      let mut state = files.remove(source).unwrap_or_default();
      let mut owners = BTreeSet::new();
      apply_change(&mut state, change, index, &mut owners);
      for owner in owners.into_iter().filter(|&owner| owner != index) {
        edges.entry((index, owner)).or_default().insert(change.path.clone());
      }
      files.insert(change.path.clone(), state);
    }
  }
  edges
    .into_iter()
    .map(|((from, to), files)| Dependency {
      from,
      to,
      files: files.into_iter().collect(),
    })
    .collect()
}

/// The connected components of `edges` between positions `0..count` that have two or more members, each oldest first,
/// ordered by their oldest member
pub fn connected_components(count: usize, edges: impl IntoIterator<Item = (usize, usize)>) -> Vec<Vec<usize>> {
  // union-find whose root is the oldest member of its set
  fn root(parents: &mut [usize], mut index: usize) -> usize {
    while parents[index] != index {
      parents[index] = parents[parents[index]];
      index = parents[index];
    }
    index
  }
  let mut parents: Vec<usize> = (0..count).collect();
  for (a, b) in edges {
    let (a, b) = (root(&mut parents, a), root(&mut parents, b));
    parents[a.max(b)] = a.min(b);
  }
  let mut components: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
  for index in 0..count {
    components.entry(root(&mut parents, index)).or_default().push(index);
  }
  components.into_values().filter(|component| component.len() >= 2).collect()
}

/// Update the state of a file for the change of the commit at `index`, collecting the commits it depends on
fn apply_change(state: &mut FileState, change: &FileChange, index: usize, owners: &mut BTreeSet<usize>) {
  match change.status {
    'A' | 'C' => {
      owners.extend(state.deleted_by);
      *state = FileState {
        created_by: Some(index),
        ..FileState::default()
      };
      apply_hunks(&mut state.spans, &change.hunks, index, owners);
    }
    'D' => {
      owners.extend(state.created_by);
      owners.extend(state.spans.iter().map(|span| span.owner));
      *state = FileState {
        deleted_by: Some(index),
        ..FileState::default()
      };
    }
    status => {
      owners.extend(state.created_by);
      if change.binary || status == 'T' {
        owners.extend(state.last_writer);
        state.spans.clear();
      } else {
        apply_hunks(&mut state.spans, &change.hunks, index, owners);
      }
    }
  }
  state.last_writer = Some(index);
}

/// Apply the hunks of one file change (ascending and disjoint, in the coordinates before the change) in one sweep over
/// the spans: the owners come from the old spans, then the spans move to the coordinates after the change
fn apply_hunks(spans: &mut Vec<Span>, hunks: &[Hunk], owner: usize, owners: &mut BTreeSet<usize>) {
  for hunk in hunks {
    let (touch_start, touch_end) = touched_lines(hunk);
    let first = spans.partition_point(|span| span.end <= touch_start);
    owners.extend(spans[first..].iter().take_while(|span| span.start < touch_end).map(|span| span.owner));
  }

  let mut old = std::mem::take(spans).into_iter();
  let mut piece = old.next();
  // lines after the hunks applied so far move by this much
  let mut shift = 0i64;
  for hunk in hunks {
    let (cut_start, cut_end) = replaced_lines(hunk);
    // the pieces before the replaced lines keep their lines
    while let Some(current) = piece {
      if current.start >= cut_start {
        break;
      }
      if current.end <= cut_start {
        push_span(spans, current, shift);
        piece = old.next();
      } else {
        push_span(spans, Span { end: cut_start, ..current }, shift);
        piece = Some(Span { start: cut_start, ..current });
        break;
      }
    }
    // the replaced lines are gone
    while let Some(current) = piece {
      if current.start >= cut_end {
        break;
      }
      piece = if current.end <= cut_end { old.next() } else { Some(Span { start: cut_end, ..current }) };
    }
    if hunk.new_lines > 0 {
      let start = cut_start;
      push_span(
        spans,
        Span {
          start,
          end: start + hunk.new_lines,
          owner,
        },
        shift,
      );
    }
    shift += i64::from(hunk.new_lines) - i64::from(hunk.old_lines);
  }
  while let Some(current) = piece {
    push_span(spans, current, shift);
    piece = old.next();
  }
}

/// The lines a hunk touches, adjacent lines included: `-a,b` with `b > 0` touches `a-1 ..= a+b`, a pure insertion after
/// line `a` touches `a` and `a+1`
fn touched_lines(hunk: &Hunk) -> (u32, u32) {
  let (a, b) = (hunk.old_start, hunk.old_lines);
  if b > 0 { (a.saturating_sub(1), a + b + 1) } else { (a, a + 2) }
}

/// The lines a hunk replaces; a pure insertion after line `a` replaces nothing at `a + 1`
fn replaced_lines(hunk: &Hunk) -> (u32, u32) {
  let start = if hunk.old_lines > 0 { hunk.old_start } else { hunk.old_start + 1 };
  (start, start + hunk.old_lines)
}

/// Append a span moved by `shift` lines, merged into the previous one when they continue each other
fn push_span(spans: &mut Vec<Span>, span: Span, shift: i64) {
  let moved = |line: u32| u32::try_from(i64::from(line) + shift).unwrap_or(0);
  let span = Span {
    start: moved(span.start),
    end: moved(span.end),
    owner: span.owner,
  };
  match spans.last_mut() {
    Some(last) if last.end == span.start && last.owner == span.owner => last.end = span.end,
    _ => spans.push(span),
  }
}

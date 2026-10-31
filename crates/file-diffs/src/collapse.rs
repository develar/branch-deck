//! Which files the views start collapsed, and why: one rule for every view of a diff (the review, the Files section,
//! the conflicts), which the page only words

use crate::model::{Collapse, DiffContent};
use generated_files::names::generated_name;

/// A file with more changed lines starts collapsed
pub const MAX_OPEN_CHANGED_LINES: u32 = 1000;

/// Why a file at `path` with `content` starts collapsed: its name first (a lock file says so even when it is large),
/// then a content without text, then more than `MAX_OPEN_CHANGED_LINES` changed lines
pub fn collapse_of(path: &str, content: &DiffContent) -> Option<Collapse> {
  if let Some(name) = generated_name(path) {
    return Some(Collapse::Generated { name });
  }
  match content {
    DiffContent::Text(diff) => {
      let changed = diff.additions + diff.deletions;
      (changed > MAX_OPEN_CHANGED_LINES).then_some(Collapse::Large { changed })
    }
    DiffContent::Binary | DiffContent::TooLarge { .. } | DiffContent::Submodule { .. } | DiffContent::Missing => Some(Collapse::NoText),
  }
}

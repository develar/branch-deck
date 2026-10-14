//! Areas: the directories a commit changes, cut to a fixed depth, so that an agent sees where a commit belongs without
//! its file list (`bd commits --brief`)

use crate::generated::is_hub_file;
use object_cache::commit_diff::FileChange;
use schemars::JsonSchema;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::HashMap;

/// The number of directories an area keeps by default
pub const DEFAULT_AREA_DEPTH: usize = 3;

/// The number of areas per commit
const TOP_AREAS: usize = 3;

/// An area with a smaller share of a commit's lines, in percent, is left out, unless it is the first: a peripheral
/// edit tells little about where the commit belongs
const MIN_AREA_SHARE: u32 = 5;

#[derive(Debug, Serialize, JsonSchema)]
pub struct DirLines {
  /// `.` for the top level
  pub dir: String,
  /// Added plus deleted lines
  pub lines: u32,
}

/// The first `depth` directories of `path`, at most all of them; `.` for a file at the top level
pub fn area_of(path: &str, depth: usize) -> &str {
  match path.match_indices('/').take(depth).last() {
    Some((end, _)) => &path[..end],
    None => ".",
  }
}

/// The areas of `changes` with the most added plus deleted lines, at most three, and without those under 5 % of the
/// lines except the first; hub files don't count. An area of renames or binary files only has 0 lines, so it is listed
/// only when the commit has no other lines
pub fn top_areas<'a>(changes: impl IntoIterator<Item = &'a FileChange>, depth: usize) -> Vec<DirLines> {
  let counted = changes
    .into_iter()
    .filter(|change| !is_hub_file(&change.path))
    .map(|change| (change.path.as_str(), if change.binary { 0 } else { change.additions + change.deletions }));
  let areas = area_lines(counted, depth);
  let total: u32 = areas.iter().map(|(_, lines)| lines).sum();
  areas
    .into_iter()
    .take(TOP_AREAS)
    .enumerate()
    .filter(|&(index, (_, lines))| index == 0 || u64::from(lines) * 100 >= u64::from(total) * u64::from(MIN_AREA_SHARE))
    .map(|(_, (dir, lines))| DirLines { dir: dir.to_string(), lines })
    .collect()
}

/// The lines of each area of `(path, lines)`, the most lines first and on a tie by name
pub fn area_lines<'a>(files: impl IntoIterator<Item = (&'a str, u32)>, depth: usize) -> Vec<(&'a str, u32)> {
  let mut lines: HashMap<&str, u32> = HashMap::new();
  for (path, counted) in files {
    *lines.entry(area_of(path, depth)).or_default() += counted;
  }
  let mut areas: Vec<(&str, u32)> = lines.into_iter().collect();
  areas.sort_by_key(|&(dir, lines)| (Reverse(lines), dir));
  areas
}

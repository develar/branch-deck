//! The shape of a changed block for the views: similar pairs for the Inline view, and ranges dropped. Branch Deck's
//! own, after the inline diff of Space: a block whose lines changed only a little reads best as one row per pair, and
//! the ranges of a block that changed almost completely are noise.

/// A pair of lines is similar when less than this share of its chars changed, as `(numerator, denominator)`; to be
/// tuned on real commits
pub const SIMILAR_PAIR_MAX_CHANGED: (usize, usize) = (1, 3);
/// A pair of lines is similar only when neither line has more ranges than this; to be tuned on real commits
pub const SIMILAR_PAIR_MAX_RANGES: usize = 4;
/// The ranges of a block are dropped when at least this share of its chars changed; to be tuned on real commits
pub const DROP_RANGES_MIN_CHANGED: (usize, usize) = (2, 3);

/// The changed ranges of one line of a block: UTF-16 offsets inside the line, without its line end
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineRanges {
  /// The length of the line in UTF-16 code units, without its line end
  pub length: usize,
  pub ranges: Vec<(usize, usize)>,
}

impl LineRanges {
  /// The number of changed UTF-16 code units of the line
  pub fn changed(&self) -> usize {
    self.ranges.iter().map(|(start, end)| end - start).sum()
  }
}

/// Whether the ranges of a block are noise: at least `DROP_RANGES_MIN_CHANGED` of its chars changed
pub fn ranges_dropped(old: &[LineRanges], new: &[LineRanges]) -> bool {
  let lines = || old.iter().chain(new);
  let changed: usize = lines().map(LineRanges::changed).sum();
  let total: usize = lines().map(|line| line.length).sum();
  total > 0 && changed * DROP_RANGES_MIN_CHANGED.1 >= total * DROP_RANGES_MIN_CHANGED.0
}

/// Whether the Inline view may draw pair i of a block as one row: the sides have the same number of lines, and each
/// pair is similar
pub fn inline_pairs(old: &[LineRanges], new: &[LineRanges]) -> bool {
  old.len() == new.len() && old.iter().zip(new).all(|(line1, line2)| is_similar_pair(line1, line2))
}

fn is_similar_pair(line1: &LineRanges, line2: &LineRanges) -> bool {
  if line1.ranges.len() > SIMILAR_PAIR_MAX_RANGES || line2.ranges.len() > SIMILAR_PAIR_MAX_RANGES {
    return false;
  }
  let total = line1.length + line2.length;
  let changed = line1.changed() + line2.changed();
  total == 0 || changed * SIMILAR_PAIR_MAX_CHANGED.1 < total * SIMILAR_PAIR_MAX_CHANGED.0
}

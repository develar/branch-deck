//! Helpers of the tests: the texts and the seed of the property tests.

use proptest::prelude::{Strategy, any};
use proptest::test_runner::{Config, RngSeed};

/// `cases` cases from one seed, so that a run tries the same texts on every machine
pub(crate) fn proptest_config(cases: u32) -> Config {
  let rng_seed = RngSeed::Fixed(0x7E57_D1FF);
  Config {
    cases,
    rng_seed,
    ..Config::default()
  }
}

/// A text of fewer than `max_lines` lines from `contents`, each with a line end of `ends` but the last one, which
/// may go without
pub(crate) fn text_of_lines(contents: Vec<&'static str>, ends: Vec<&'static str>, max_lines: usize) -> impl Strategy<Value = String> {
  let line = (proptest::sample::select(contents), proptest::sample::select(ends));
  (proptest::collection::vec(line, 0..max_lines), any::<bool>()).prop_map(|(lines, open_end)| {
    let last = lines.len().saturating_sub(1);
    lines
      .iter()
      .enumerate()
      .flat_map(|(index, (content, end))| [*content, if index == last && open_end { "" } else { end }])
      .collect()
  })
}

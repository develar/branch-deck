//! The highlighting of files: the time of each grammar on a corpus, or with `--show` the coloured pieces of each line.
//!
//! `cargo run --release -p code-symbols --example highlight -- [--show] <files…>`

use code_symbols::highlight::{grammar_of, highlight};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

fn main() {
  let mut args: Vec<String> = std::env::args().skip(1).collect();
  let show = args.iter().position(|it| it == "--show").map(|index| args.remove(index)).is_some();
  let mut totals: BTreeMap<String, (usize, usize, Duration, usize)> = BTreeMap::new();
  for path in &args {
    let Some(grammar) = grammar_of(path) else { continue };
    let Ok(text) = std::fs::read_to_string(path) else { continue };
    let start = Instant::now();
    let runs = highlight(grammar, &text);
    let elapsed = start.elapsed();
    let entry = totals.entry(format!("{grammar:?}")).or_default();
    entry.0 += 1;
    entry.1 += text.len();
    entry.2 += elapsed;
    if runs.is_none() {
      entry.3 += 1;
    }
    if show && let Some(runs) = runs {
      println!("{path}: {} lines, {} runs", runs.line_count(), runs.runs.len() / 2);
    }
  }
  for (grammar, (files, bytes, time, failed)) in totals {
    #[expect(clippy::cast_precision_loss, reason = "a report")]
    let megabytes = bytes as f64 / 1e6;
    let per_mb = time.as_secs_f64() * 1000.0 / megabytes;
    println!(
      "{grammar:<10} {files:>4} files {:>6.2} MB {:>8.1} ms {per_mb:>7.1} ms/MB  without highlighting: {failed}",
      megabytes,
      time.as_secs_f64() * 1000.0
    );
  }
}

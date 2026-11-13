//! The cost of the comparisons of a scan without a deadline, against the wall-time budgets of the views: every text
//! file a commit of a range modifies, compared once under `DIFF_LIMITS` and once under `DiffBudget::NONE`, the blocks of
//! the two compared.
//!
//! ```text
//! cargo run --release -p file-diffs --example scan_limits -- <repository> <base>..<head> [--threads N] [--slowest K]
//! ```
//!
//! It prints the pairs, the summed and the longest time of each way (the sum of the calls on every thread, and the
//! wall time of all of them), the pairs whose comparison without a deadline took longer than the inner budget of the
//! views (100 ms) and than their line budget (1 s), the pairs whose blocks differ between the two ways, and the
//! `--slowest` (10) pairs without a deadline with their sizes. The decision of `blocks_of` (a scan is bounded by its
//! work alone) rests on these numbers: docs/design/git-operations.md, "The budgets of a scan".

use file_diffs::service::DIFF_LIMITS;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cat_file_batch::{BatchEntry, for_each_object};
use git_ops::commit_files::MAX_BLOB_BYTES;
use git_ops::object_texts::{into_text, is_binary};
use line_diff::cancellation::Deadline;
use line_diff::model::DiffBudget;
use std::collections::HashMap;
use std::io::Write as _;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use text_diff::model::{WhitespacePolicy, diff_texts};

/// A modified file of a commit: its path and its two blobs
struct Pair {
  path: String,
  old: String,
  new: String,
}

/// What one pair cost: the time under the views' budgets and without a deadline, whether their blocks are the same,
/// whether the views' diff is by lines alone, and the sizes of the two texts
struct Measured {
  view: Duration,
  none: Duration,
  same_blocks: bool,
  view_line_level: bool,
  none_line_level: bool,
  bytes: (usize, usize),
}

fn main() {
  let mut args: Vec<String> = std::env::args().skip(1).collect();
  let take = |args: &mut Vec<String>, name: &str| -> Option<String> {
    let index = args.iter().position(|arg| arg == name)?;
    args.remove(index);
    (index < args.len()).then(|| args.remove(index))
  };
  let threads: usize = take(&mut args, "--threads")
    .and_then(|it| it.parse().ok())
    .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, usize::from));
  let slowest: usize = take(&mut args, "--slowest").and_then(|it| it.parse().ok()).unwrap_or(10);
  let [repository, range] = <[String; 2]>::try_from(args).unwrap_or_else(|_| {
    let _ = writeln!(std::io::stderr(), "usage: scan_limits <repository> <base>..<head> [--threads N] [--slowest K]");
    std::process::exit(2)
  });
  let git = GitCommandExecutor::new();

  let started = Instant::now();
  let pairs = modified_pairs(&git, &repository, &range);
  let ids: Vec<String> = {
    let mut seen = std::collections::HashSet::new();
    pairs
      .iter()
      .flat_map(|pair| [pair.old.clone(), pair.new.clone()])
      .filter(|id| seen.insert(id.clone()))
      .collect()
  };
  let mut texts: HashMap<String, String> = HashMap::with_capacity(ids.len());
  for_each_object(&git, &repository, &ids, MAX_BLOB_BYTES, |position, entry| {
    if let BatchEntry::Object { kind: "blob", content, .. } = entry
      && !is_binary(&content)
    {
      texts.insert(ids[position].clone(), into_text(content));
    }
    Ok(())
  })
  .expect("cat-file");
  let pairs: Vec<&Pair> = pairs.iter().filter(|pair| texts.contains_key(&pair.old) && texts.contains_key(&pair.new)).collect();
  let _ = writeln!(
    std::io::stderr(),
    "{} text pairs of {range}, blobs read in {:.1} s",
    pairs.len(),
    started.elapsed().as_secs_f64()
  );

  // each way on every thread at once, as a scan compares; per pair its time, its blocks and whether by lines alone
  let run = |budget: fn() -> DiffBudget| {
    let next = AtomicUsize::new(0);
    let out = Mutex::new(Vec::with_capacity(pairs.len()));
    let wall_started = Instant::now();
    std::thread::scope(|scope| {
      for _ in 0..threads {
        scope.spawn(|| {
          loop {
            let index = next.fetch_add(1, Ordering::Relaxed);
            let Some(pair) = pairs.get(index) else { break };
            let start = Instant::now();
            let diff = diff_texts(&texts[&pair.old], &texts[&pair.new], WhitespacePolicy::Default, &budget());
            let time = start.elapsed();
            out.lock().unwrap().push((index, time, diff.blocks, diff.line_level_only));
          }
        });
      }
    });
    let wall = wall_started.elapsed();
    let mut out = out.into_inner().unwrap();
    out.sort_by_key(|(index, ..)| *index);
    (out, wall)
  };
  let (views, view_wall) = run(|| DiffBudget {
    inner: Some(Deadline::after(DIFF_LIMITS.file)),
    lines: Deadline::after(DIFF_LIMITS.lines),
  });
  let (none, none_wall) = run(|| DiffBudget::NONE);
  let results: Vec<(usize, Measured)> = views
    .into_iter()
    .zip(none)
    .map(|((index, view, view_blocks, view_line_level), (_, none, none_blocks, none_line_level))| {
      let pair = pairs[index];
      let measured = Measured {
        view,
        none,
        same_blocks: view_blocks == none_blocks,
        view_line_level,
        none_line_level,
        bytes: (texts[&pair.old].len(), texts[&pair.new].len()),
      };
      (index, measured)
    })
    .collect();
  report(&pairs, &results, [view_wall, none_wall], slowest);
}

/// The modified text files of every commit of `range` but merges: `log --raw` of their blobs
fn modified_pairs(git: &GitCommandExecutor, repository: &str, range: &str) -> Vec<Pair> {
  let output = git
    .execute_command(&["log", "--no-merges", "--raw", "--no-abbrev", "--no-renames", "-z", "--format=", range, "--"], repository)
    .expect("git log");
  let mut pairs = Vec::new();
  let mut fields = output.split('\0').filter(|field| !field.is_empty());
  while let Some(meta) = fields.next() {
    let meta = meta.trim_start_matches('\n');
    let Some(path) = fields.next() else { break };
    // :<old mode> <new mode> <old blob> <new blob> <status>
    let parts: Vec<&str> = meta.trim_start_matches(':').split(' ').collect();
    if let [old_mode, new_mode, old, new, "M"] = parts.as_slice()
      && old_mode.starts_with("100")
      && new_mode.starts_with("100")
    {
      pairs.push(Pair {
        path: path.to_string(),
        old: (*old).to_string(),
        new: (*new).to_string(),
      });
    }
  }
  pairs
}

fn report(pairs: &[&Pair], results: &[(usize, Measured)], walls: [Duration; 2], slowest: usize) {
  let sum = |pick: fn(&Measured) -> Duration| results.iter().map(|(_, measured)| pick(measured)).sum::<Duration>();
  let max = |pick: fn(&Measured) -> Duration| results.iter().map(|(_, measured)| pick(measured)).max().unwrap_or_default();
  let over = |limit: Duration| results.iter().filter(|(_, measured)| measured.none > limit).count();
  println!("pairs\t{}", results.len());
  println!(
    "views: sum\t{:.2} s\tmax\t{:.3} s\twall\t{:.2} s",
    sum(|m| m.view).as_secs_f64(),
    max(|m| m.view).as_secs_f64(),
    walls[0].as_secs_f64()
  );
  println!(
    "none:  sum\t{:.2} s\tmax\t{:.3} s\twall\t{:.2} s",
    sum(|m| m.none).as_secs_f64(),
    max(|m| m.none).as_secs_f64(),
    walls[1].as_secs_f64()
  );
  println!("none over 100 ms\t{}\tover 1 s\t{}", over(DIFF_LIMITS.file), over(DIFF_LIMITS.lines));
  println!(
    "line level: views\t{}\tnone\t{}",
    results.iter().filter(|(_, m)| m.view_line_level).count(),
    results.iter().filter(|(_, m)| m.none_line_level).count()
  );
  println!("blocks differ\t{}", results.iter().filter(|(_, m)| !m.same_blocks).count());
  let mut order: Vec<&(usize, Measured)> = results.iter().collect();
  order.sort_by_key(|(_, measured)| std::cmp::Reverse(measured.none));
  for (index, measured) in order.into_iter().take(slowest) {
    println!(
      "slow\t{:.3} s\tviews {:.3} s\t{}+{} bytes\t{}{}",
      measured.none.as_secs_f64(),
      measured.view.as_secs_f64(),
      measured.bytes.0,
      measured.bytes.1,
      pairs[*index].path,
      if measured.view_line_level && !measured.none_line_level { "\tcut by time" } else { "" }
    );
  }
}

//! The oracle of the rule "equal by tokens" on real files: every file of a grammar under the given paths has its
//! tokens checked (`code_tokens::checks`: their order, lines and columns, only blanks between them, the same tokens,
//! the same content and the same text without its blanks once the text is formatted again, another token for a
//! renamed name).
//!
//! ```text
//! cargo run --release -p code-tokens --example tokens -- [--threads N] [--show N] [--none] <files and directories…>
//! ```
//!
//! Prints each file a check fails on (`path<TAB>what`), the first `--show` (40) of them, with `--none` each file
//! without tokens (`path<TAB>no tokens`), and a line of totals: the files, those without tokens (over the size limit,
//! nested too deeply, a script oxc read only after a recovery, tokens that are not those of the text), the tokens,
//! the lines formatted again, the failures and the time. Exits with 1 when a check failed. A directory is walked
//! without its `.git`, `node_modules`, `target`, `build` and `out`.

use code_tokens::checks;
use code_tokens::grammar_of;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

#[derive(Default)]
struct Totals {
  files: usize,
  none: Vec<String>,
  tokens: usize,
  lines: usize,
  failures: Vec<String>,
}

fn walk(path: &Path, out: &mut Vec<PathBuf>) {
  if path.is_dir() {
    let skipped = path
      .file_name()
      .is_some_and(|name| [".git", "node_modules", "target", "build", "out"].iter().any(|it| name == *it));
    if skipped {
      return;
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(path).into_iter().flatten().flatten().map(|entry| entry.path()).collect();
    entries.sort();
    for entry in entries {
      walk(&entry, out);
    }
  } else if path.to_str().is_some_and(|it| grammar_of(it).is_some()) {
    out.push(path.to_path_buf());
  }
}

#[expect(clippy::print_stdout, clippy::print_stderr, reason = "an example prints its answer")]
fn main() {
  let mut args: Vec<String> = std::env::args().skip(1).collect();
  let mut value = |flag: &str| {
    let at = args.iter().position(|it| it == flag)?;
    args.remove(at);
    (at < args.len()).then(|| args.remove(at))
  };
  let threads = value("--threads")
    .and_then(|it| it.parse().ok())
    .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, usize::from));
  let show = value("--show").and_then(|it| it.parse().ok()).unwrap_or(40);
  let list_none = args.iter().position(|it| it == "--none").map(|at| args.remove(at)).is_some();
  let mut files = Vec::new();
  for path in &args {
    walk(Path::new(path), &mut files);
  }

  let started = Instant::now();
  let next = AtomicUsize::new(0);
  let totals = Mutex::new(Totals::default());
  std::thread::scope(|scope| {
    for _ in 0..threads.max(1) {
      // a parse needs 1 MB of stack
      let worker = std::thread::Builder::new().stack_size(8 << 20).spawn_scoped(scope, || {
        while let Some(path) = files.get(next.fetch_add(1, Ordering::Relaxed)) {
          let name = path.to_string_lossy();
          let (Some(grammar), Ok(text)) = (grammar_of(&name), std::fs::read_to_string(path)) else {
            continue;
          };
          let checked = checks::check(grammar, &name, &text);
          let mut totals = totals.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
          totals.files += 1;
          match checked {
            Ok(None) => totals.none.push(name.into_owned()),
            Ok(Some(checked)) => {
              totals.tokens += checked.tokens;
              totals.lines += checked.lines;
            }
            Err(what) => totals.failures.push(format!("{name}\t{what}")),
          }
        }
      });
      if let Err(error) = worker {
        eprintln!("no worker thread: {error}");
      }
    }
  });

  let mut totals = totals.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner);
  totals.failures.sort();
  for failure in totals.failures.iter().take(show) {
    println!("{failure}");
  }
  if list_none {
    totals.none.sort();
    for name in &totals.none {
      println!("{name}\tno tokens");
    }
  }
  println!(
    "{} files, {} without tokens, {} tokens, {} lines formatted again, {} failures, {:.1} s",
    totals.files,
    totals.none.len(),
    totals.tokens,
    totals.lines,
    totals.failures.len(),
    started.elapsed().as_secs_f64()
  );
  if !totals.failures.is_empty() {
    std::process::exit(1);
  }
}

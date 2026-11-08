//! The oracle of a change of parser or of rules: every fact of the three consumers of a syntax tree, as stable text, for
//! a list of files, and the comparison of two such runs.
//!
//! ```text
//! cargo run --release -p code-symbols --example facts -- [--families summary,highlight,navigation] [--threads N]
//!     [--probes K] [--nav-max-bytes B] [--nav-max-depth D] (--list FILE --root DIR | <files…>) > facts.txt 2> timing.txt
//! cargo run --release -p code-symbols --example facts -- --compare before.txt after.txt [--show N] [--family F]
//! ```
//!
//! A line is `path<TAB>family<TAB>fact` (a line break, a tab or a backslash in a fact escaped as in Rust), the files in the order of the list (a line of the list may be `PATH<TAB>LEVEL`,
//! as the Java corpus has them: the path is taken), every family of a file at least one line, so two runs over the
//! same list compare group by group:
//!
//! - `summary`: `summary_text::render_full` of `extract::summarize`, one line per line of it;
//! - `highlight`: `lines N` (or `none` without highlighting), then each line that has runs as `L<line> <col>:<style> …`,
//!   the column in UTF-16 units of the line, the style as its role (`-` for the default foreground) with `+i`, `+b`,
//!   `+u` for italic, bold, underline;
//! - `navigation`: `probes N`, then per probe `L:C word => <reference_at> | <usages_at>`, and `usages_of name => …` of
//!   the first and the middle probed word. The probes are words of the text, not of a tree, so they are the same for
//!   any parser: every start of an identifier-like word (a letter, `_` or `$` after no letter, digit, `_` or `$`),
//!   at most `--probes` (8) of them per file, taken at the indices `i * count / K`. A file over `--nav-max-bytes`
//!   (65 536) or with brackets (`(`, `[`, `{`, counted in the text alone) nested deeper than `--nav-max-depth` (128) has
//!   none (`probes skipped: …`): the tree-sitter navigation of Kotlin and Java before psi-syntax, the baseline of the
//!   move, took minutes on such a file (`BigIf.java`, an `else if` chain of 115 KB, `Parenth2.java`, parentheses 1000
//!   deep), as its `Node::parent` climbs from the root; the defaults keep a run comparable with that baseline.
//!
//! The time of each family (the sum of its calls on every thread) and the wall time go to stderr, and every 30 s the
//! files a worker has been on that long (`slow<TAB>path<TAB>N s`). `--compare` reads two
//! runs over the same list and prints each moved group (`moved<TAB>family<TAB>path<TAB>-removed +added` with its first
//! `--show` (4) lines that only one run has) and a table of the moved files and lines per family.

use code_symbols::extract::summarize;
use code_symbols::highlight::{BOLD, ITALIC, ROLE_BITS, Role, UNDERLINE, grammar_of, highlight};
use code_symbols::language::language_of;
use code_symbols::navigate::{reference_at, usages_at, usages_of};
use code_symbols::summary_text::render_full;
use fact_text::{access_counts, arguments, inputs, probes, push, reference_text, take_value, usages_at_text};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::io::{BufWriter, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

#[path = "shared/fact_text.rs"]
mod fact_text;

const FAMILIES: [&str; 3] = ["summary", "highlight", "navigation"];

fn main() {
  let mut args = arguments();
  let families: Vec<String> =
    take_value(&mut args, "--families").map_or_else(|| FAMILIES.iter().map(ToString::to_string).collect(), |it| it.split(',').map(ToString::to_string).collect());
  let threads = take_value(&mut args, "--threads")
    .and_then(|it| it.parse().ok())
    .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, usize::from));
  let nav_max_bytes = take_value(&mut args, "--nav-max-bytes").map_or(1 << 16, |it| it.parse().unwrap_or(1 << 16));
  let nav_max_depth = take_value(&mut args, "--nav-max-depth").map_or(128, |it| it.parse().unwrap_or(128));
  let (probes, root, paths) = inputs(&mut args);
  let options = Options {
    families,
    probes,
    nav_max_bytes,
    nav_max_depth,
  };
  run(&paths, root.as_deref(), threads, &options);
}

struct Options {
  families: Vec<String>,
  probes: usize,
  nav_max_bytes: usize,
  nav_max_depth: usize,
}

/// The time of each family: (files, the sum of its calls)
type Times = HashMap<&'static str, (usize, Duration)>;

/// The facts of every file, by `threads` workers, written in the order of `paths` as they come
fn run(paths: &[String], root: Option<&Path>, threads: usize, options: &Options) {
  let started = Instant::now();
  let next = AtomicUsize::new(0);
  let times: Mutex<Times> = Mutex::new(HashMap::new());
  // the file each worker is on, since when
  let busy: Vec<Mutex<Option<(usize, Instant)>>> = (0..threads.max(1)).map(|_| Mutex::new(None)).collect();
  let (sender, receiver) = mpsc::sync_channel::<(usize, String)>(threads * 4);
  std::thread::scope(|scope| {
    for slot in &busy {
      let sender = sender.clone();
      let (next, times) = (&next, &times);
      scope.spawn(move || {
        let mut own: Times = HashMap::new();
        loop {
          let index = next.fetch_add(1, Ordering::Relaxed);
          let Some(path) = paths.get(index) else { break };
          let full = root.map_or_else(|| PathBuf::from(path), |it| it.join(path));
          *lock(slot) = Some((index, Instant::now()));
          let out = facts_of(path, &full, options, &mut own);
          *lock(slot) = None;
          if sender.send((index, out)).is_err() {
            break;
          }
        }
        let mut times = lock(times);
        for (family, (files, time)) in own {
          let entry = times.entry(family).or_default();
          entry.0 += files;
          entry.1 += time;
        }
      });
    }
    drop(sender);
    let mut stdout = BufWriter::new(std::io::stdout().lock());
    let mut pending: BTreeMap<usize, String> = BTreeMap::new();
    let mut written = 0;
    loop {
      match receiver.recv_timeout(SLOW) {
        Ok((index, out)) => {
          pending.insert(index, out);
          while let Some(out) = pending.remove(&written) {
            let _ = stdout.write_all(out.as_bytes());
            written += 1;
          }
        }
        Err(mpsc::RecvTimeoutError::Timeout) => report_slow(paths, &busy),
        Err(mpsc::RecvTimeoutError::Disconnected) => break,
      }
    }
    let _ = stdout.flush();
  });
  let times = times.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner);
  for family in FAMILIES {
    if let Some((files, time)) = times.get(family) {
      let _ = writeln!(std::io::stderr(), "timing\t{family}\t{files} files\t{:.1} ms", time.as_secs_f64() * 1000.0);
    }
  }
  let _ = writeln!(
    std::io::stderr(),
    "timing\twall\t{} files\t{:.1} ms\t{threads} threads",
    paths.len(),
    started.elapsed().as_secs_f64() * 1000.0
  );
}

/// Every line of every family of one file
fn facts_of(path: &str, full: &Path, options: &Options, times: &mut Times) -> String {
  let mut out = String::new();
  let Ok(source) = std::fs::read(full) else {
    let _ = writeln!(out, "{path}\tread\tcannot read");
    return out;
  };
  let wants = |family: &str| options.families.iter().any(|it| it == family);
  if wants("summary")
    && let Some(language) = language_of(path)
  {
    let start = Instant::now();
    let summary = summarize(language, path, &source);
    time(times, "summary", start);
    for it in render_full(&summary).lines() {
      push(&mut out, path, "summary", it);
    }
  }
  let Some(grammar) = grammar_of(path) else { return out };
  let Ok(text) = std::str::from_utf8(&source) else {
    for family in ["highlight", "navigation"].into_iter().filter(|it| wants(it)) {
      push(&mut out, path, family, "not utf-8");
    }
    return out;
  };
  if wants("highlight") {
    let start = Instant::now();
    let runs = highlight(grammar, text);
    time(times, "highlight", start);
    match runs {
      None => push(&mut out, path, "highlight", "none"),
      Some(runs) => {
        push(&mut out, path, "highlight", &format!("lines {}", runs.line_count()));
        for index in 0..runs.line_count() {
          let pairs = runs.line(index);
          if pairs.is_empty() {
            continue;
          }
          let mut fact = format!("L{}", index + 1);
          for [column, style] in pairs.as_chunks::<2>().0 {
            let _ = write!(fact, " {column}:{}", style_text(*style));
          }
          push(&mut out, path, "highlight", &fact);
        }
      }
    }
  }
  let depth = bracket_depth(text);
  if wants("navigation") && (text.len() > options.nav_max_bytes || depth > options.nav_max_depth) {
    push(&mut out, path, "navigation", &format!("probes skipped: {} bytes, brackets {depth} deep", text.len()));
  } else if wants("navigation") {
    let words = probes(text, options.probes);
    push(&mut out, path, "navigation", &format!("probes {}", words.len()));
    let start = Instant::now();
    let mut asked: Vec<&str> = Vec::new();
    for (index, (position, word)) in words.iter().enumerate() {
      let reference = reference_at(grammar, text, *position).map_or_else(|| "none".to_string(), |it| reference_text(&it));
      let usages = usages_at(grammar, text, *position).map_or_else(|| "none".to_string(), |it| usages_at_text(&it));
      push(
        &mut out,
        path,
        "navigation",
        &format!("{}:{} {word} => {reference} | {usages}", position.line, position.column),
      );
      if (index == 0 || index == words.len() / 2) && !asked.contains(word) {
        asked.push(word);
      }
    }
    for word in asked {
      let usages = usages_of(grammar, text, word);
      push(
        &mut out,
        path,
        "navigation",
        &format!("usages_of {word} => {} {}", usages.len(), access_counts(usages.iter().map(|it| it.access))),
      );
    }
    time(times, "navigation", start);
  }
  out
}

/// A file whose facts take longer is named on stderr, every `SLOW` while it lasts
const SLOW: Duration = Duration::from_secs(30);

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
  mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// `slow<TAB>path<TAB>N s` of each file a worker has been on for `SLOW` or longer
fn report_slow(paths: &[String], busy: &[Mutex<Option<(usize, Instant)>>]) {
  for slot in busy {
    if let Some((index, since)) = *lock(slot)
      && since.elapsed() >= SLOW
    {
      let _ = writeln!(std::io::stderr(), "slow\t{}\t{} s", paths[index], since.elapsed().as_secs());
    }
  }
}

fn time(times: &mut Times, family: &'static str, start: Instant) {
  let entry = times.entry(family).or_default();
  entry.0 += 1;
  entry.1 += start.elapsed();
}

/// `keyword`, `field+i`, `-` for the default foreground
fn style_text(style: u32) -> String {
  let role = (style & ((1 << ROLE_BITS) - 1)) as usize;
  let mut text = role
    .checked_sub(1)
    .and_then(|it| Role::NAMES.get(it))
    .map_or_else(|| if role == 0 { "-".to_string() } else { format!("role{role}") }, ToString::to_string);
  for (bit, name) in [(ITALIC, "+i"), (BOLD, "+b"), (UNDERLINE, "+u")] {
    if style & bit != 0 {
      text.push_str(name);
    }
  }
  text
}

/// How deep the brackets of the text nest, strings and comments included: a bound of the nesting of any tree of it
fn bracket_depth(text: &str) -> usize {
  let (mut depth, mut deepest) = (0usize, 0usize);
  for byte in text.bytes() {
    match byte {
      b'(' | b'[' | b'{' => {
        depth += 1;
        deepest = deepest.max(depth);
      }
      b')' | b']' | b'}' => depth = depth.saturating_sub(1),
      _ => {}
    }
  }
  deepest
}

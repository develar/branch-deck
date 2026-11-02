//! The oracle of a change of parser or of rules: every fact of the three consumers of a syntax tree, as stable text, for
//! a list of files, and the comparison of two such runs.
//!
//! ```text
//! cargo run --release -p code-symbols --example facts -- [--families summary,highlight,navigation] [--threads N]
//!     [--probes K] (--list FILE --root DIR | <files…>) > facts.txt 2> timing.txt
//! cargo run --release -p code-symbols --example facts -- --compare before.txt after.txt [--show N] [--family F]
//! ```
//!
//! A line is `path<TAB>family<TAB>fact`, the files in the order of the list (a line of the list may be `PATH<TAB>LEVEL`,
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
//!   at most `--probes` (8) of them per file, taken at the indices `i * count / K`.
//!
//! The time of each family (the sum of its calls on every thread) and the wall time go to stderr, and every 30 s the
//! files a worker has been on that long (`slow<TAB>path<TAB>N s`). `--compare` reads two
//! runs over the same list and prints each moved group (`moved<TAB>family<TAB>path<TAB>-removed +added` with its first
//! `--show` (4) lines that only one run has) and a table of the moved files and lines per family.

use code_symbols::extract::summarize;
use code_symbols::highlight::{BOLD, ITALIC, ROLE_BITS, Role, UNDERLINE, grammar_of, highlight};
use code_symbols::language::language_of;
use code_symbols::navigate::{Access, Position, ReferenceKind, UsagesAt, reference_at, usages_at, usages_of};
use code_symbols::summary_text::render_full;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::io::{BufRead, BufReader, BufWriter, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

const FAMILIES: [&str; 3] = ["summary", "highlight", "navigation"];

fn main() {
  let mut args: Vec<String> = std::env::args().skip(1).collect();
  if take_flag(&mut args, "--compare") {
    let show = take_value(&mut args, "--show").map_or(4, |it| it.parse().unwrap_or(4));
    let family = take_value(&mut args, "--family");
    let [before, after] = args.as_slice() else {
      let _ = writeln!(std::io::stderr(), "--compare BEFORE AFTER");
      std::process::exit(2);
    };
    std::process::exit(compare(Path::new(before), Path::new(after), show, family.as_deref()));
  }
  let families: Vec<String> =
    take_value(&mut args, "--families").map_or_else(|| FAMILIES.iter().map(ToString::to_string).collect(), |it| it.split(',').map(ToString::to_string).collect());
  let threads = take_value(&mut args, "--threads")
    .and_then(|it| it.parse().ok())
    .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, usize::from));
  let probes: usize = take_value(&mut args, "--probes").map_or(8, |it| it.parse().unwrap_or(8));
  let root = take_value(&mut args, "--root").map(PathBuf::from);
  let mut paths: Vec<String> = Vec::new();
  if let Some(list) = take_value(&mut args, "--list") {
    let text = std::fs::read_to_string(&list).unwrap_or_else(|error| panic!("{list}: {error}"));
    paths.extend(text.lines().filter(|it| !it.is_empty()).map(|it| it.split('\t').next().unwrap_or(it).to_string()));
  }
  paths.extend(args);
  let options = Options { families, probes };
  run(&paths, root.as_deref(), threads, &options);
}

struct Options {
  families: Vec<String>,
  probes: usize,
}

/// Whether `flag` is among the arguments, taken out of them
fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
  args.iter().position(|it| it == flag).map(|index| args.remove(index)).is_some()
}

fn take_value(args: &mut Vec<String>, flag: &str) -> Option<String> {
  let index = args.iter().position(|it| it == flag)?;
  args.remove(index);
  (index < args.len()).then(|| args.remove(index))
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
    let summary = summarize(language, &source);
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
  if wants("navigation") {
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

fn push(out: &mut String, path: &str, family: &str, fact: &str) {
  let _ = writeln!(out, "{path}\t{family}\t{fact}");
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

/// The words of the text to ask about, by the text alone: at most `limit`, spread over it
fn probes(text: &str, limit: usize) -> Vec<(Position, &str)> {
  let is_part = |it: char| it.is_alphanumeric() || it == '_' || it == '$';
  let mut words: Vec<(usize, &str)> = Vec::new();
  let mut previous: Option<char> = None;
  for (offset, character) in text.char_indices() {
    if (character.is_alphabetic() || character == '_' || character == '$') && !previous.is_some_and(is_part) {
      let end = text[offset..].find(|it: char| !is_part(it)).map_or(text.len(), |it| offset + it);
      words.push((offset, &text[offset..end]));
    }
    previous = Some(character);
  }
  let count = words.len();
  let picked: Vec<(usize, &str)> = if count <= limit {
    words
  } else {
    (0..limit).map(|it| words[it * count / limit]).collect()
  };
  picked.into_iter().map(|(offset, word)| (position_of(text, offset), word)).collect()
}

/// The line from 0 (`\n`, `\r\n` or a lone `\r` ends one, as the view splits them) and the UTF-16 column of an offset
fn position_of(text: &str, offset: usize) -> Position {
  let before = &text[..offset];
  let line_start = before.rfind(['\n', '\r']).map_or(0, |it| it + 1);
  let mut line = 0u32;
  let mut previous = '\0';
  for character in before.chars() {
    if character == '\r' || (character == '\n' && previous != '\r') {
      line += 1;
    }
    previous = character;
  }
  let column = u32::try_from(before[line_start..].encode_utf16().count()).unwrap_or(u32::MAX);
  Position { line, column }
}

fn reference_text(reference: &code_symbols::navigate::ReferenceAt) -> String {
  let kind = match reference.kind {
    ReferenceKind::Value => "value",
    ReferenceKind::Call => "call",
    ReferenceKind::Member => "member",
    ReferenceKind::MemberCall => "member_call",
    ReferenceKind::Type => "type",
    ReferenceKind::Declaration => "declaration",
  };
  let local = reference.local.map_or_else(|| "-".to_string(), |it| format!("{}:{}", it.line, it.column));
  format!(
    "{kind} {} q={} at {}:{}-{} local={local}",
    reference.name,
    reference.qualifier.as_deref().unwrap_or("-"),
    reference.start.line,
    reference.start.column,
    reference.end_column
  )
}

fn usages_at_text(usages: &UsagesAt) -> String {
  format!(
    "{}{} {} {}",
    usages.name,
    if usages.local { " local" } else { "" },
    usages.usages.len(),
    access_counts(usages.usages.iter().map(|it| it.access))
  )
}

/// `declaration=1 call=2`, the kinds in a fixed order, without those of none
fn access_counts(accesses: impl Iterator<Item = Access>) -> String {
  let order = [Access::Declaration, Access::Import, Access::Call, Access::Write, Access::Read];
  let mut counts = [0usize; 5];
  for access in accesses {
    if let Some(index) = order.iter().position(|it| *it == access) {
      counts[index] += 1;
    }
  }
  let parts: Vec<String> = order
    .iter()
    .zip(counts)
    .filter(|(_, count)| *count > 0)
    .map(|(access, count)| format!("{}={count}", access.name()))
    .collect();
  parts.join(" ")
}

/// The lines of one file and family in a run, read group by group
struct Groups {
  lines: std::io::Lines<BufReader<std::fs::File>>,
  next: Option<String>,
}

impl Groups {
  fn open(path: &Path) -> Self {
    let file = std::fs::File::open(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let mut lines = BufReader::new(file).lines();
    let next = lines.next().and_then(Result::ok);
    Self { lines, next }
  }

  /// The next group: (path, family) and its facts
  fn next_group(&mut self) -> Option<((String, String), Vec<String>)> {
    let first = self.next.take()?;
    let key = group_key(&first);
    let mut facts = vec![fact_of(&first)];
    loop {
      self.next = self.lines.next().and_then(Result::ok);
      match &self.next {
        Some(line) if group_key(line) == key => facts.push(fact_of(line)),
        _ => break,
      }
    }
    Some((key, facts))
  }
}

fn group_key(line: &str) -> (String, String) {
  let mut parts = line.splitn(3, '\t');
  (parts.next().unwrap_or_default().to_string(), parts.next().unwrap_or_default().to_string())
}

fn fact_of(line: &str) -> String {
  line.splitn(3, '\t').nth(2).unwrap_or_default().to_string()
}

/// Prints the moved groups of two runs over the same list and the moved files and lines per family; exit code 1 when
/// any moved
fn compare(before: &Path, after: &Path, show: usize, only: Option<&str>) -> i32 {
  let mut before = Groups::open(before);
  let mut after = Groups::open(after);
  // family -> (files, moved files, removed lines, added lines)
  let mut totals: BTreeMap<String, (usize, usize, usize, usize)> = BTreeMap::new();
  let mut stdout = BufWriter::new(std::io::stdout().lock());
  loop {
    let (old, new) = (before.next_group(), after.next_group());
    let (old, new) = match (old, new) {
      (None, None) => break,
      (old, new) => (old, new),
    };
    if let (Some((old_key, _)), Some((new_key, _))) = (&old, &new)
      && old_key != new_key
    {
      let _ = writeln!(
        stdout,
        "out of step\t{}\t{}\t!=\t{}\t{}: the runs are not of the same list",
        old_key.0, old_key.1, new_key.0, new_key.1
      );
      let _ = stdout.flush();
      return 2;
    }
    let ((path, family), old_facts, new_facts) = match (old, new) {
      (Some((key, old)), Some((_, new))) => (key, old, new),
      (Some((key, old)), None) => (key, old, Vec::new()),
      (None, Some((key, new))) => (key, Vec::new(), new),
      (None, None) => break,
    };
    if only.is_some_and(|it| it != family) {
      continue;
    }
    let total = totals.entry(family.clone()).or_default();
    total.0 += 1;
    if old_facts == new_facts {
      continue;
    }
    let (removed, added) = multiset_difference(&old_facts, &new_facts);
    total.1 += 1;
    total.2 += removed.len();
    total.3 += added.len();
    let _ = writeln!(stdout, "moved\t{family}\t{path}\t-{} +{}", removed.len(), added.len());
    for it in removed.iter().take(show) {
      let _ = writeln!(stdout, "  - {it}");
    }
    for it in added.iter().take(show) {
      let _ = writeln!(stdout, "  + {it}");
    }
    if removed.is_empty() && added.is_empty() {
      let _ = writeln!(stdout, "  (the same lines in another order)");
    }
  }
  let _ = writeln!(stdout, "\nfamily\tfiles\tmoved files\tremoved lines\tadded lines");
  let mut moved = false;
  for (family, (files, moved_files, removed, added)) in &totals {
    moved |= *moved_files > 0;
    let _ = writeln!(stdout, "{family}\t{files}\t{moved_files}\t{removed}\t{added}");
  }
  let _ = stdout.flush();
  i32::from(moved)
}

/// The lines only `old` has and those only `new` has, each in its order, counting repeats
fn multiset_difference<'a>(old: &'a [String], new: &'a [String]) -> (Vec<&'a str>, Vec<&'a str>) {
  let mut counts: HashMap<&str, isize> = HashMap::new();
  for it in old {
    *counts.entry(it).or_default() += 1;
  }
  for it in new {
    *counts.entry(it).or_default() -= 1;
  }
  let mut left = counts.clone();
  let removed = old
    .iter()
    .filter(|it| {
      let count = left.entry(it.as_str()).or_default();
      if *count > 0 {
        *count -= 1;
        true
      } else {
        false
      }
    })
    .map(String::as_str)
    .collect();
  let added = new
    .iter()
    .filter(|it| {
      let count = counts.entry(it.as_str()).or_default();
      if *count < 0 {
        *count += 1;
        true
      } else {
        false
      }
    })
    .map(String::as_str)
    .collect();
  (removed, added)
}

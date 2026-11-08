//! What the oracles of `examples/` share: the arguments they take, the stable text they print, the words of a file they
//! ask about, and the comparison of two runs. One reading of the format, so that `facts.rs` and `accio_facts.rs` can be
//! compared with each other and with themselves.

use code_symbols::navigate::{Access, Position, ReferenceAt, ReferenceKind, UsagesAt};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::io::{BufRead, BufReader, BufWriter, Write as _};
use std::path::{Path, PathBuf};

/// Whether `flag` is among the arguments, taken out of them
pub(crate) fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
  args.iter().position(|it| it == flag).map(|index| args.remove(index)).is_some()
}

pub(crate) fn take_value(args: &mut Vec<String>, flag: &str) -> Option<String> {
  let index = args.iter().position(|it| it == flag)?;
  args.remove(index);
  (index < args.len()).then(|| args.remove(index))
}

/// The arguments of the run; `--compare BEFORE AFTER [--show N] [--family F]` is answered here and ends the process
pub(crate) fn arguments() -> Vec<String> {
  let mut args: Vec<String> = std::env::args().skip(1).collect();
  if !take_flag(&mut args, "--compare") {
    return args;
  }
  let show = take_value(&mut args, "--show").map_or(4, |it| it.parse().unwrap_or(4));
  let family = take_value(&mut args, "--family");
  let [before, after] = args.as_slice() else {
    let _ = writeln!(std::io::stderr(), "--compare BEFORE AFTER");
    std::process::exit(2);
  };
  std::process::exit(compare(Path::new(before), Path::new(after), show, family.as_deref()));
}

/// What a run reads: `--probes K` (8), `--root DIR`, and the files of `--list FILE` (a line may be `PATH<TAB>LEVEL`, as
/// the Java corpus has them: the path is taken) and of the arguments left, which the call takes out
pub(crate) fn inputs(args: &mut Vec<String>) -> (usize, Option<PathBuf>, Vec<String>) {
  let probes = take_value(args, "--probes").map_or(8, |it| it.parse().unwrap_or(8));
  let root = take_value(args, "--root").map(PathBuf::from);
  let mut paths: Vec<String> = Vec::new();
  if let Some(list) = take_value(args, "--list") {
    let text = std::fs::read_to_string(&list).unwrap_or_else(|error| panic!("{list}: {error}"));
    paths.extend(text.lines().filter(|it| !it.is_empty()).map(|it| it.split('\t').next().unwrap_or(it).to_string()));
  }
  paths.extend(std::mem::take(args));
  (probes, root, paths)
}

/// One line of facts; a line break or a tab inside the fact (the qualifier `a⏎.b` of a member, a header with a text
/// block) as `\n`, `\r`, `\t`, so that every line starts with its path and family
pub(crate) fn push(out: &mut String, path: &str, family: &str, fact: &str) {
  let _ = write!(out, "{path}\t{family}\t");
  for character in fact.chars() {
    match character {
      '\n' => out.push_str("\\n"),
      '\r' => out.push_str("\\r"),
      '\t' => out.push_str("\\t"),
      '\\' => out.push_str("\\\\"),
      _ => out.push(character),
    }
  }
  out.push('\n');
}

/// The words of the text to ask about, by the text alone: at most `limit`, spread over it
pub(crate) fn probes(text: &str, limit: usize) -> Vec<(Position, &str)> {
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
pub(crate) fn position_of(text: &str, offset: usize) -> Position {
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

pub(crate) fn reference_text(reference: &ReferenceAt) -> String {
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

pub(crate) fn usages_at_text(usages: &UsagesAt) -> String {
  format!(
    "{}{} {} {}",
    usages.name,
    if usages.local { " local" } else { "" },
    usages.usages.len(),
    access_counts(usages.usages.iter().map(|it| it.access))
  )
}

/// `declaration=1 call=2`, the kinds in a fixed order, without those of none
pub(crate) fn access_counts(accesses: impl Iterator<Item = Access>) -> String {
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

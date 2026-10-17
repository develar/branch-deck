//! Prints the summary of each file as JSON with the names resolved, or with `--stats` one line of numbers per file and
//! a table per language, or with `--tree` the syntax tree (kinds, fields and lines) for writing rules.
//!
//! `cargo run --release -p code-symbols --example summarize -- [--stats | --tree] <files…>`

use code_symbols::extract::{grammar, summarize};
use code_symbols::language::{Language, language_of};
use code_symbols::summary::{FileSummary, ImportTarget, Shape, TypeRef};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::Instant;

fn main() {
  let mut args: Vec<String> = std::env::args().skip(1).collect();
  let stats = take_flag(&mut args, "--stats");
  let tree = take_flag(&mut args, "--tree");
  let mut totals: BTreeMap<Language, Totals> = BTreeMap::new();
  if stats {
    // the first parse of a grammar pages in its tables; keep that out of the first file's time
    for language in Language::ALL {
      summarize(language, b"x");
    }
  }
  for path in &args {
    let Some(language) = language_of(path) else {
      println!("{path}: no language");
      continue;
    };
    let source = match std::fs::read(path) {
      Ok(source) => source,
      Err(error) => {
        println!("{path}: {error}");
        continue;
      }
    };
    if tree {
      print_tree(language, &source);
      continue;
    }
    let start = Instant::now();
    let summary = summarize(language, &source);
    let millis = start.elapsed().as_secs_f64() * 1000.0;
    if stats {
      let error_share = f64::from(summary.error_bytes) / f64::from(summary.total_bytes.max(1));
      println!(
        "{path}\t{language:?}\t{} B\t{} lines\t{millis:.2} ms\terrors {:.2}%\t{} declarations\t{} references{}",
        summary.total_bytes,
        summary.lines,
        error_share * 100.0,
        summary.declarations.len(),
        summary.references.len(),
        if summary.truncated { "\ttruncated" } else { "" },
      );
      totals.entry(language).or_default().add(&summary, millis);
    } else {
      println!("{}", serde_json::to_string_pretty(&resolved(path, &summary)).unwrap_or_default());
    }
  }
  if stats {
    println!("\nlanguage\tfiles\tMiB\ttotal ms\tp99 ms\tmax ms\tdegraded\ttruncated");
    for (language, totals) in &mut totals {
      totals.millis.sort_by(f64::total_cmp);
      let p99 = totals.millis[(totals.millis.len() * 99 / 100).min(totals.millis.len() - 1)];
      let max = totals.millis.last().copied().unwrap_or_default();
      #[expect(clippy::cast_precision_loss, reason = "file sizes are far below 2^52")]
      let mib = totals.bytes as f64 / f64::from(1u32 << 20);
      println!(
        "{language:?}\t{}\t{mib:.2}\t{:.1}\t{p99:.2}\t{max:.2}\t{} ({:.1}%)\t{}",
        totals.millis.len(),
        totals.millis.iter().sum::<f64>(),
        totals.degraded,
        f64::from(totals.degraded) * 100.0 / f64::from(u32::try_from(totals.millis.len()).unwrap_or(u32::MAX)),
        totals.truncated,
      );
    }
  }
}

fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
  let before = args.len();
  args.retain(|it| it != flag);
  args.len() != before
}

#[derive(Default)]
struct Totals {
  bytes: u64,
  millis: Vec<f64>,
  degraded: u32,
  truncated: u32,
}

impl Totals {
  fn add(&mut self, summary: &FileSummary, millis: f64) {
    self.bytes += u64::from(summary.total_bytes);
    self.millis.push(millis);
    self.degraded += u32::from(summary.is_degraded());
    self.truncated += u32::from(summary.truncated);
  }
}

fn resolved(path: &str, summary: &FileSummary) -> Value {
  let name = |id: u32| summary.name(id).to_string();
  let path_of = |ids: &[u32]| ids.iter().map(|it| summary.name(*it)).collect::<Vec<_>>().join(".");
  let container = |id: Option<u32>| id.and_then(|it| summary.declarations.get(it as usize)).map(|it| name(it.name));
  let type_ref = |it: &Option<TypeRef>| {
    it.as_ref()
      .map(|it| json!({ "names": it.names.iter().map(|it| name(*it)).collect::<Vec<_>>(), "nullable": it.nullable, "arrays": it.arrays }))
  };
  let shape = |id: Option<u32>| match id.and_then(|it| summary.shapes.get(it as usize)) {
    None => Value::Null,
    Some(Shape::Override { function, receiver, parameters }) => json!({
      "override": { "function": function, "receiver": receiver.map(name), "parameters": parameters.iter().map(type_ref).collect::<Vec<_>>() },
    }),
    Some(Shape::Call { arguments, spread, enclosed }) => json!({
      "call": {
        "arguments": arguments.iter().map(|(it, argument)| json!({ "name": it.map(name), "argument": format!("{argument:?}") })).collect::<Vec<_>>(),
        "spread": spread,
        "enclosed": enclosed,
      },
    }),
  };
  json!({
    "path": path,
    "language": format!("{:?}", summary.language),
    "package": summary.package,
    "imports": summary.imports.iter().map(|it| json!({
      "path": path_of(&it.path),
      "target": match it.target {
        ImportTarget::Name(id) => name(id),
        ImportTarget::All => "*".to_string(),
        ImportTarget::Default => "default".to_string(),
        ImportTarget::Namespace => "namespace".to_string(),
      },
      "alias": it.alias.map(name),
      "exported": it.exported,
      "specifier": it.specifier,
      "line": it.line,
    })).collect::<Vec<_>>(),
    "declarations": summary.declarations.iter().map(|it| json!({
      "name": name(it.name),
      "kind": format!("{:?}", it.kind),
      "container": container(it.container),
      "receiver": it.receiver.map(name),
      "reach": format!("{:?}", it.reach),
      "member": format!("{:?}", it.member),
      "signature": it.signature.as_ref().map(|signature| signature.parameters.iter().map(|it| json!({
        "name": name(it.name),
        "type": type_ref(&it.type_ref),
        "optional": it.optional,
        "vararg": it.vararg,
      })).collect::<Vec<_>>()),
      "overrides": it.overrides,
      "overridable": it.overridable,
      "supertypes": it.supertypes.iter().map(|it| path_of(it)).collect::<Vec<_>>(),
      "lines": format!("{}-{} (name {})", it.start_line, it.end_line, it.name_line),
      "in_error": it.in_error,
    })).collect::<Vec<_>>(),
    "references": summary.references.iter().map(|it| json!({
      "name": name(it.name),
      "line": it.line,
      "role": format!("{:?}", it.role),
      "qualifier": it.qualifier.as_deref().map(path_of),
      "container": container(it.container),
      "shape": shape(it.shape),
    })).collect::<Vec<_>>(),
    "error_bytes": summary.error_bytes,
    "total_bytes": summary.total_bytes,
    "lines": summary.lines,
    "truncated": summary.truncated,
  })
}

fn print_tree(language: Language, source: &[u8]) {
  let Some(grammar) = grammar(language) else { return };
  let mut parser = tree_sitter::Parser::new();
  if parser.set_language(&grammar).is_err() {
    return;
  }
  let Some(tree) = parser.parse(source, None) else { return };
  let mut cursor = tree.walk();
  let mut depth = 0usize;
  loop {
    let node = cursor.node();
    if node.is_named() || node.is_missing() {
      let field = cursor.field_name().map_or_default(|it| format!("{it}: "));
      let text = if node.named_child_count() == 0 {
        format!(" {:?}", node.utf8_text(source).unwrap_or_default())
      } else {
        String::new()
      };
      let flag = if node.is_missing() {
        " MISSING"
      } else if node.is_error() {
        " ERROR"
      } else {
        ""
      };
      println!("{}{field}{}{flag} @{}{text}", "  ".repeat(depth), node.kind(), node.start_position().row + 1);
    }
    if cursor.goto_first_child() {
      depth += 1;
      continue;
    }
    while !cursor.goto_next_sibling() {
      if !cursor.goto_parent() {
        return;
      }
      depth -= 1;
    }
  }
}

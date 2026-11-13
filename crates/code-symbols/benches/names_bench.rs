//! What the bindings of many names cost (docs/design/diff-view.md, "The bindings of many names"): `names_at` of 200
//! places of a file asked together, from one parse, and each of them asked alone, a parse per place. `cargo bench -p
//! code-symbols --bench names_bench`, on a quiet machine.
//!
//! The file of each language is 250 functions of one shape: two parameters and three locals, each read a few times, so
//! every function has its own bindings of the same spellings and a spelling has 250 declarations. The places asked are
//! the declaration of a local in each of the first 200 functions, each with a new name: 200 entries, a resolution of
//! the spelling and one more per entry for its clashes.

use code_symbols::navigate::{NameQuestion, Position, names_at};
use code_tokens::Grammar;
use criterion::{Criterion, criterion_group, criterion_main};
use std::fmt::Write as _;
use std::hint::black_box;

const FUNCTIONS: usize = 250;
const PLACES: usize = 200;

/// A language of the benchmark: its grammar, the head and the tail of its file, and its function of the number `{n}`,
/// whose local `total` is declared in a line that holds `declares`
struct Language {
  name: &'static str,
  grammar: Grammar,
  head: &'static str,
  function: &'static str,
  tail: &'static str,
  declares: &'static str,
}

const LANGUAGES: [Language; 6] = [
  Language {
    name: "Kotlin",
    grammar: Grammar::Kotlin,
    head: "package bench\n\nclass Ledger(private val report: Report) {\n",
    function: "  fun handle{n}(entries: List<Entry>, scale: Int): Long {\n    var total = 0L\n    for (entry in entries) {\n      val amount = entry.amount * scale\n      total += amount\n    }\n    val result = total + scale\n    return report.send(result, total)\n  }\n\n",
    tail: "}\n",
    declares: "var total",
  },
  Language {
    name: "Java",
    grammar: Grammar::Java,
    head: "package bench;\n\nclass Ledger {\n  Report report;\n",
    function: "  long handle{n}(List<Entry> entries, int scale) {\n    long total = 0;\n    for (Entry entry : entries) {\n      long amount = entry.amount * scale;\n      total += amount;\n    }\n    long result = total + scale;\n    return report.send(result, total);\n  }\n\n",
    tail: "}\n",
    declares: "long total",
  },
  Language {
    name: "C#",
    grammar: Grammar::CSharp,
    head: "namespace Bench;\n\nclass Ledger {\n  Report report;\n",
    function: "  long Handle{n}(List<Entry> entries, int scale) {\n    long total = 0;\n    foreach (var entry in entries) {\n      var amount = entry.Amount * scale;\n      total += amount;\n    }\n    var result = total + scale;\n    return report.Send(result, total);\n  }\n\n",
    tail: "}\n",
    declares: "long total",
  },
  Language {
    name: "Starlark",
    grammar: Grammar::Starlark,
    head: "load(\":report.bzl\", \"send\")\n\n",
    function: "def handle{n}(entries, scale):\n    total = 0\n    for entry in entries:\n        amount = entry.amount * scale\n        total += amount\n    result = total + scale\n    return send(result, total)\n\n",
    tail: "",
    declares: "total = 0",
  },
  Language {
    name: "Rust",
    grammar: Grammar::Rust,
    head: "use crate::report::send;\n\n",
    function: "fn handle{n}(entries: &[Entry], scale: u64) -> u64 {\n  let mut total = 0;\n  for entry in entries {\n    let amount = entry.amount * scale;\n    total += amount;\n  }\n  let result = total + scale;\n  send(result, total)\n}\n\n",
    tail: "",
    declares: "let mut total",
  },
  Language {
    name: "TypeScript",
    grammar: Grammar::TypeScript,
    head: "import { send } from \"./report\"\n\n",
    function: "export function handle{n}(entries: Entry[], scale: number): number {\n  let total = 0\n  for (const entry of entries) {\n    const amount = entry.amount * scale\n    total += amount\n  }\n  const result = total + scale\n  return send(result, total)\n}\n\n",
    tail: "",
    declares: "let total",
  },
];

/// The file of a language, and the place of the local `total` of each of its first `PLACES` functions
fn source(language: &Language) -> (String, Vec<Position>) {
  let mut text = language.head.to_string();
  for index in 0..FUNCTIONS {
    write!(text, "{}", language.function.replace("{n}", &index.to_string())).unwrap();
  }
  text.push_str(language.tail);
  let declarations = text.lines().enumerate().filter(|it| it.1.contains(language.declares));
  let places = declarations.take(PLACES).map(|(line, text)| Position {
    line: u32::try_from(line).unwrap(),
    column: u32::try_from(text.find("total").unwrap()).unwrap(),
  });
  let places: Vec<Position> = places.collect();
  assert_eq!(places.len(), PLACES, "{}", language.name);
  (text, places)
}

fn bench_names(c: &mut Criterion) {
  for language in &LANGUAGES {
    let (text, places) = source(language);
    let questions: Vec<NameQuestion<'_>> = places
      .iter()
      .map(|&position| NameQuestion {
        position,
        renamed_to: Some("sum"),
      })
      .collect();
    // every place is a local of its own function, declared, written and read twice, and no function has a `sum`
    let found = names_at(language.grammar, &text, &questions);
    assert_eq!(found.names.len(), PLACES, "{}", language.name);
    assert!(
      found
        .names
        .iter()
        .all(|it| it.local.is_some() && it.uses.len() == 4 && it.clashes.as_ref().is_some_and(Vec::is_empty)),
      "{}",
      language.name
    );

    let mut group = c.benchmark_group(format!("names_at, {} places of {} lines of {}", PLACES, text.lines().count(), language.name));
    group.sample_size(10);
    group.bench_function("together", |b| b.iter(|| names_at(language.grammar, black_box(&text), black_box(&questions))));
    group.bench_function("one by one", |b| {
      b.iter(|| {
        for question in &questions {
          black_box(names_at(language.grammar, black_box(&text), std::slice::from_ref(question)));
        }
      });
    });
    group.finish();
  }
}

criterion_group!(benches, bench_names);
criterion_main!(benches);

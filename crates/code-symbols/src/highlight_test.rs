//! The highlighting: golden files of every grammar (`testdata/highlight/<file>.highlight`, each line with its coloured
//! pieces; `CODE_SYMBOLS_UPDATE=1 cargo nextest run -p code-symbols` rewrites them, review the diff), the lines and
//! offsets of the view, and the roles of the view.

use crate::highlight::lines::LineRuns;
use crate::highlight::{BOLD, Grammar, ITALIC, ROLE_BITS, Role, UNDERLINE, grammar_of, highlight};
use std::fmt::Write as _;
use std::path::PathBuf;

fn testdata() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata").join("highlight")
}

/// The name of a style: its role and its font styles
fn style_name(style: u32) -> String {
  let role = style & ((1 << ROLE_BITS) - 1);
  let mut name = if role == 0 { "plain".to_string() } else { Role::NAMES[role as usize - 1].to_string() };
  for (bit, font) in [(ITALIC, "italic"), (BOLD, "bold"), (UNDERLINE, "underline")] {
    if style & bit != 0 {
      name.push('+');
      name.push_str(font);
    }
  }
  name
}

/// The lines of a text as the view splits them
fn view_lines(text: &str) -> Vec<&str> {
  let mut lines = Vec::new();
  let mut start = 0;
  let bytes = text.as_bytes();
  let mut index = 0;
  while index < bytes.len() {
    if bytes[index] == b'\n' || bytes[index] == b'\r' {
      lines.push(&text[start..index]);
      index += if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') { 2 } else { 1 };
      start = index;
    } else {
      index += 1;
    }
  }
  lines.push(&text[start..]);
  lines
}

/// Each line with the pieces in a style other than the default foreground, as `style "text"`
fn render(text: &str, runs: &LineRuns) -> String {
  let mut out = String::new();
  for (number, line) in view_lines(text).into_iter().enumerate() {
    let _ = writeln!(out, "{:>3}: {line}", number + 1);
    let units: Vec<u16> = line.encode_utf16().collect();
    let pairs = runs.line(number);
    let pieces: Vec<String> = pairs
      .chunks(2)
      .enumerate()
      .filter(|(_, pair)| pair[1] != 0)
      .map(|(index, pair)| {
        let end = pairs.get(index * 2 + 2).map_or(units.len(), |it| *it as usize);
        format!("{} {:?}", style_name(pair[1]), String::from_utf16_lossy(&units[pair[0] as usize..end]))
      })
      .collect();
    if !pieces.is_empty() {
      let _ = writeln!(out, "     {}", pieces.join(" · "));
    }
  }
  out
}

#[test]
fn every_fixture_matches_its_golden_file() {
  let mut checked = 0;
  for entry in std::fs::read_dir(testdata()).unwrap() {
    let path = entry.unwrap().path();
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    let Some(grammar) = grammar_of(&name) else { continue };
    let text = std::fs::read_to_string(&path).unwrap();
    let runs = highlight(grammar, &text).unwrap_or_else(|| panic!("{name}: no highlighting"));
    assert_eq!(runs.line_count(), view_lines(&text).len(), "{name}: the lines of the view");
    let actual = render(&text, &runs);
    let golden = testdata().join(format!("{name}.highlight"));
    if std::env::var_os("CODE_SYMBOLS_UPDATE").is_some() {
      std::fs::write(&golden, &actual).unwrap();
    } else {
      let expected = std::fs::read_to_string(&golden).unwrap_or_default();
      pretty_assertions::assert_eq!(expected, actual, "{name}: rerun with CODE_SYMBOLS_UPDATE=1 to rewrite the golden file");
    }
    checked += 1;
  }
  assert_eq!(checked, 5, "a fixture of every grammar");
}

#[test]
fn splits_lines_and_counts_utf16_offsets_as_the_view() {
  // `\r\n`, a lone `\r`, a surrogate pair before a string, and the empty line after a final line end
  let text = "val a = 1\r\nval b = 2\rval 𝔸 = \"x\"\n";
  let runs = highlight(Grammar::Kotlin, text).unwrap();
  assert_eq!(runs.line_count(), 4);
  let third: Vec<u32> = runs.line(2).to_vec();
  // `val` at 0, `"x"` after `𝔸 = ` (𝔸 is two UTF-16 units): offset 4 + 2 + 3 = 9
  assert_eq!(third, [0, Role::Keyword.style(), 3, 0, 9, Role::String.style()]);
  assert_eq!(runs.line(3), &[] as &[u32]);
}

#[test]
fn has_no_highlighting_over_the_limit() {
  let text = "a".repeat(crate::highlight::MAX_BYTES + 1);
  assert_eq!(highlight(Grammar::Java, &text), None);
}

#[test]
fn chooses_the_typescript_grammar_for_ts_and_tsx_for_the_rest() {
  let grammars: Vec<Option<Grammar>> = ["a.ts", "a.mts", "a.tsx", "a.js", "a.jsx", "a.mjs", "a.kts", "a.rs", "a.java", "a.md"]
    .into_iter()
    .map(grammar_of)
    .collect();
  assert_eq!(
    grammars,
    [
      Some(Grammar::TypeScript),
      Some(Grammar::TypeScript),
      Some(Grammar::Tsx),
      Some(Grammar::Tsx),
      Some(Grammar::Tsx),
      Some(Grammar::Tsx),
      Some(Grammar::Kotlin),
      Some(Grammar::Rust),
      Some(Grammar::Java),
      None
    ]
  );
}

#[test]
fn roles_match_the_view() {
  let themes = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src/lib/diff/islands-themes.ts")).unwrap();
  let list = themes
    .split("export const SYNTAX_ROLES = [")
    .nth(1)
    .and_then(|rest| rest.split(']').next())
    .expect("SYNTAX_ROLES in islands-themes.ts");
  let names: Vec<&str> = list.split(',').map(|it| it.trim().trim_matches('"')).filter(|it| !it.is_empty()).collect();
  assert_eq!(names, Role::NAMES[..names.len()], "the roles of the view are the roles of the highlighting, in their order");
}

#[test]
fn colours_a_kotlin_name_that_is_a_soft_keyword_as_a_name() {
  let text = "fun set(value: Int, get: String) = value + get.length\n";
  let runs = highlight(Grammar::Kotlin, text).unwrap();
  let rendered = render(text, &runs);
  // `fun` is a keyword, `set` the declared function; `value` and `get` are parameters, not the soft keywords
  assert!(rendered.contains(r#"keyword "fun" · functionDeclaration "set" · field "length""#), "{rendered}");
}

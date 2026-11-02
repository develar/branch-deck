use crate::code_spans::code_spans;
use pretty_assertions::assert_eq;

/// The spans of `line` as the text they cover, with whether the author marked them
fn spans(line: &str) -> Vec<(String, bool)> {
  let units: Vec<u16> = line.encode_utf16().collect();
  code_spans(line)
    .into_iter()
    .map(|span| (String::from_utf16(&units[span.start as usize..span.end as usize]).unwrap(), span.explicit))
    .collect()
}

#[test]
fn a_span_in_backticks_is_explicit_code_its_backticks_included() {
  assert_eq!(spans("Use `pool size` here"), [("`pool size`".to_string(), true)]);
  // an unclosed backtick is prose
  assert_eq!(spans("a ` alone"), []);
}

#[test]
fn the_grammar_finds_paths_names_flags_and_calls() {
  for token in [
    "./tools/bt.cmd",
    "../shared/config",
    "~/projects/app",
    "//plugins/air/...",
    "src/lib/commit.ts",
    "Foo.generated.kt",
    "intellij.air.backend.tests",
    "com.intellij.air.backend.workflows.sessionFinished",
    "--lane",
    "-p",
    "--level=debug",
    "run()",
    "Foo.bar()",
    "Foo::bar",
    "Foo#bar",
    "POOL_RETRY_LIMIT",
    "snake_case",
    "afterLock",
    "SessionFolderStore",
    "SplashScreen",
    "Frame.paint",
    "SplashScreen.hide",
  ] {
    assert_eq!(spans(&format!("see {token} now")), [(token.to_string(), false)], "{token}");
  }
}

#[test]
fn a_word_of_prose_stays_prose() {
  for token in [
    "HTTP",
    "aB",
    "GenUI",
    "AssertJ",
    "example.com",
    "e.g.",
    "JavaScript",
    "macOS",
    "Node.js",
    "Vue.js",
    "Next.js",
    "TeamCity",
    "GoLand",
    "PhpStorm",
    "RubyMine",
    "DataGrip",
    "DataSpell",
    "RustRover",
    "ReSharper",
    "AppCode",
    "dotTrace",
    "WordPress",
    "SharePoint",
    "CocoaPods",
    "LastPass",
    "follow-up",
    "50",
    "U.S.",
  ] {
    assert_eq!(spans(&format!("see {token} now")), [], "{token}");
  }
  // a token in double quotes
  assert_eq!(spans("rename \"pool_size\" later"), []);
}

#[test]
fn the_punctuation_around_a_token_stays_prose_and_tokens_one_space_apart_are_one_span() {
  assert_eq!(spans("see (db/pool.js), then"), [("db/pool.js".to_string(), false)]);
  assert_eq!(spans("run bt.cmd --lane now"), [("bt.cmd --lane".to_string(), false)]);
  // a joined span says so: a chip, as one in backticks
  assert_eq!(code_spans("run bt.cmd --lane now").iter().map(|span| span.joined).collect::<Vec<_>>(), [true]);
  assert_eq!(code_spans("edit foo.ts, bar.ts").iter().map(|span| span.joined).collect::<Vec<_>>(), [false, false]);
  assert_eq!(
    spans("bt --module intellij.air.tests --filter com.intellij.air: 5 passed"),
    [("--module intellij.air.tests --filter com.intellij.air".to_string(), false)]
  );
  assert_eq!(spans("edit foo.ts, bar.ts"), [("foo.ts".to_string(), false), ("bar.ts".to_string(), false)]);
  assert_eq!(spans("call (run())."), [("run()".to_string(), false)]);
  assert_eq!(spans("see docs/JIRA-456/retry.md now"), [("docs/JIRA-456/retry.md".to_string(), false)]);
  // a possessive and the dash of a sentence are prose after the name
  assert_eq!(spans("SplashScreen's frame"), [("SplashScreen".to_string(), false)]);
  assert_eq!(spans("hides SplashScreen’s frame"), [("SplashScreen".to_string(), false)]);
  assert_eq!(spans("calls Frame.paint— twice"), [("Frame.paint".to_string(), false)]);
  assert_eq!(spans("as before—Frame.paint draws"), []);
  assert_eq!(spans("a dash —Frame.paint draws"), [("Frame.paint".to_string(), false)]);
  // a file of a library's name takes its directory
  assert_eq!(spans("edit web/Vue.js now"), [("web/Vue.js".to_string(), false)]);
}

#[test]
fn the_offsets_are_utf16_code_units() {
  let line = "naïve → `Frame.paint` and Manager.passes";
  let units: Vec<u16> = line.encode_utf16().collect();
  let found = code_spans(line);
  assert_eq!(found.len(), 2);
  assert_eq!(String::from_utf16(&units[found[0].start as usize..found[0].end as usize]).unwrap(), "`Frame.paint`");
  assert_eq!(String::from_utf16(&units[found[1].start as usize..found[1].end as usize]).unwrap(), "Manager.passes");
}

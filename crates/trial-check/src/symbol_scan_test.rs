use crate::diff_scan_test::commit_diffs;
use crate::symbol_scan::{TsAliases, crate_name, read_symbol_input, ts_aliases};
use object_cache::cache::{CacheConfig, ObjectCache};
use pretty_assertions::assert_eq;
use test_utils::git_test_utils::TestRepo;

#[test]
fn the_name_of_a_crate_and_the_aliases_of_a_tsconfig() {
  assert_eq!(
    crate_name(b"[workspace]\nname = \"no\"\n\n[package]\nversion = \"1\"\nname = \"avl-wire\"\n").as_deref(),
    Some("avl-wire")
  );
  assert_eq!(crate_name(b"[workspace]\nmembers = []\n"), None);
  // comments and trailing commas, which tsc accepts; a comma inside a string stays
  let config = "{\n  // the alias of the app\n  \"compilerOptions\": {\n    \"baseUrl\": \".\", /* the root */\n    \"paths\": { \"@/*\": [\"./src/*\",], \"~,}\": [\"lib/x,}\"] },\n  },\n}\n";
  assert_eq!(
    ts_aliases("web", config),
    TsAliases {
      paths: vec![("@/*".to_string(), vec!["web/src/*".to_string()]), ("~,}".to_string(), vec!["web/lib/x,}".to_string()])],
    }
  );
}

/// The summaries of both sides of each changed source file, the crate names and the aliases; a second scan reads every
/// summary from the cache and parses nothing
#[test]
fn reads_the_summaries_through_the_cache() {
  let repo = TestRepo::new();
  repo.create_commit_with_files(
    "base",
    &[
      ("crates/wire/Cargo.toml", "[package]\nname = \"avl-wire\"\n"),
      ("crates/wire/src/lib.rs", "pub fn one() {}\n"),
      ("tsconfig.json", "{ \"compilerOptions\": { \"paths\": { \"@/*\": [\"./src/*\"] } } }\n"),
    ],
  );
  let commit = repo.create_commit_with_files(
    "change",
    &[("crates/wire/src/lib.rs", "pub fn two() {}\n"), ("src/a.ts", "export const a = 1;\n"), ("notes.txt", "x\n")],
  );
  let diffs = commit_diffs(&repo, &commit);
  let dir = tempfile::tempdir().unwrap();
  let cache = ObjectCache::open_dir(dir.path(), &CacheConfig::default());

  let input = read_symbol_input(repo.git_executor(), repo.path_str(), &cache, &diffs).unwrap();
  // the old and the new lib.rs, and a.ts; not the text file
  assert_eq!((input.summaries.len(), input.stats.parsed, input.stats.cached), (3, 3, 0));
  assert_eq!(input.crates.get("crates/wire").map(String::as_str), Some("avl_wire"));
  assert_eq!(input.aliases[""].paths, [("@/*".to_string(), vec!["src/*".to_string()])]);

  let again = read_symbol_input(repo.git_executor(), repo.path_str(), &cache, &diffs).unwrap();
  assert_eq!((again.stats.parsed, again.stats.cached), (0, 3));
  assert_eq!(again.summaries.keys().collect::<std::collections::BTreeSet<_>>(), input.summaries.keys().collect());
}

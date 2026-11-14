use crate::diff_scan_test::commit_diffs;
use crate::symbol_scan::{crate_name, read_symbol_input};
use js_syntax::tsconfig::Aliases;
use pretty_assertions::assert_eq;
use test_utils::git_test_utils::TestRepo;
use test_utils::process_budget::{assert_processes, counting};

#[test]
fn the_name_of_a_crate() {
  assert_eq!(
    crate_name(b"[workspace]\nname = \"no\"\n\n[package]\nversion = \"1\"\nname = \"avl-wire\"\n").as_deref(),
    Some("avl-wire")
  );
  assert_eq!(crate_name(b"[workspace]\nmembers = []\n"), None);
}

/// `paths` and `baseUrl` of each `tsconfig*.json` above the changed files, with the configurations its `extends`
/// names applied as tsc applies them: a `tsconfig*.json` of a directory above the files is in the batch of the
/// summaries, its name without `.json` tried for nothing, so the chain costs no process; any other configuration is
/// read in one `cat-file` more, by its blob beside the files or by its path in another directory
#[test]
fn the_aliases_of_a_tsconfig_with_the_chain_it_extends() {
  let repo = TestRepo::new();
  repo.create_commit_with_files(
    "base",
    &[
      (
        "tsconfig.base.json",
        "{ \"compilerOptions\": { \"paths\": { \"@shared/*\": [\"packages/shared/src/*\"] } } }\n",
      ),
      // comments and trailing commas, which tsc accepts
      (
        "web/tsconfig.json",
        "{\n  // the aliases of the root\n  \"extends\": \"../tsconfig.base\",\n  \"compilerOptions\": { \"strict\": true, },\n}\n",
      ),
      (
        "config/tsconfig.paths.json",
        "{ \"compilerOptions\": { \"baseUrl\": \".\", \"paths\": { \"~/*\": [\"../app/src/*\"] } } }\n",
      ),
      ("app/tsconfig.json", "{ \"extends\": \"../config/tsconfig.paths.json\" }\n"),
      ("lib/base.json", "{ \"compilerOptions\": { \"paths\": { \"#/*\": [\"./src/*\"] } } }\n"),
      ("lib/tsconfig.json", "{ \"extends\": \"./base\" }\n"),
    ],
  );
  let web = repo.create_commit_with_files("web", &[("web/src/a.ts", "export const a = 1;\n")]);
  let app = repo.create_commit_with_files("app", &[("app/src/b.ts", "export const b = 1;\n")]);
  let lib = repo.create_commit_with_files("lib", &[("lib/src/c.ts", "export const c = 1;\n")]);
  let dir = tempfile::tempdir().unwrap();
  let cache = test_utils::process_budget::cache_in(dir.path());
  let shared = Aliases {
    base_url: None,
    paths: vec![("@shared/*".to_string(), vec!["packages/shared/src/*".to_string()])],
  };

  let git = counting();
  let input = read_symbol_input(&git, repo.path_str(), &cache, &commit_diffs(&repo, &web)).unwrap();
  assert_eq!(input.aliases["web"], std::slice::from_ref(&shared));
  assert_eq!(input.aliases[""], [shared]);
  assert_eq!(input.aliases["web"][0].candidates("@shared/diff/find"), ["packages/shared/src/diff/find"]);
  // the trees above the files, then the summary of a.ts and the two configurations
  assert_processes(&git, &[("cat-file", 2)]);

  let git = counting();
  let input = read_symbol_input(&git, repo.path_str(), &cache, &commit_diffs(&repo, &app)).unwrap();
  let paths = Aliases {
    base_url: Some("config".to_string()),
    paths: vec![("~/*".to_string(), vec!["app/src/*".to_string()])],
  };
  assert_eq!(input.aliases["app"], [paths]);
  assert_eq!(input.aliases["app"][0].candidates("~/x"), ["app/src/x", "config/~/x"]);
  // config/ is no directory above the files: its configuration in a batch of its own
  assert_processes(&git, &[("cat-file", 3)]);

  let git = counting();
  let input = read_symbol_input(&git, repo.path_str(), &cache, &commit_diffs(&repo, &lib)).unwrap();
  let own = Aliases {
    base_url: None,
    paths: vec![("#/*".to_string(), vec!["lib/src/*".to_string()])],
  };
  assert_eq!(input.aliases["lib"], [own]);
  // lib/ is above the files, but base.json is no `tsconfig*.json`: its blob in a batch of its own, `./base` tried first
  // for nothing
  assert_processes(&git, &[("cat-file", 3)]);
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
  let cache = test_utils::process_budget::cache_in(dir.path());

  let input = read_symbol_input(repo.git_executor(), repo.path_str(), &cache, &diffs).unwrap();
  // the old and the new lib.rs, and a.ts; not the text file
  assert_eq!((input.summaries.len(), input.stats.parsed, input.stats.cached), (3, 3, 0));
  assert_eq!(input.crates.get("crates/wire").map(String::as_str), Some("avl_wire"));
  assert_eq!(
    input.aliases[""],
    [Aliases {
      base_url: None,
      paths: vec![("@/*".to_string(), vec!["src/*".to_string()])],
    }]
  );

  let again = read_symbol_input(repo.git_executor(), repo.path_str(), &cache, &diffs).unwrap();
  assert_eq!((again.stats.parsed, again.stats.cached), (0, 3));
  assert_eq!(again.summaries.keys().collect::<std::collections::BTreeSet<_>>(), input.summaries.keys().collect());
}

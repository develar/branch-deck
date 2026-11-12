use crate::test_support::{PREFIX, range};
use crate::units::{OtherReason, UnitKind};
use crate::{PlanSources, ReviewPlan, review_plan_core};
use file_diffs::service::FileDiffs;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use std::fmt::Write as _;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use test_utils::process_budget::empty_cache;

/// The plan of `base..head` of `repo` with `cache` and a new memory of the diffs, as a new process of `brd` reads it
fn plan_with(repo: &TestRepo, cache: &ObjectCache, base: &str, head: &str) -> ReviewPlan {
  let file_diffs = FileDiffs::for_scans();
  let sources = PlanSources {
    cache,
    file_diffs: &file_diffs,
    index: None,
  };
  review_plan_core(repo.git_executor(), sources, PREFIX, &range(repo, base, head)).unwrap()
}

/// Writes `files` (path, bytes) into the working tree of `repo` and commits them as `message`
fn commit_bytes(repo: &TestRepo, message: &str, files: &[(&str, &[u8])]) -> String {
  for (path, bytes) in files {
    let file = repo.path().join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, bytes).unwrap();
    repo.git(&["add", "--", path]);
  }
  repo.commit_with(&["-m", message], &[])
}

/// A Java file in Windows-1252 (a dash as the byte 0x96 in a comment), one method changed
fn java(body: &str) -> Vec<u8> {
  let mut text = b"package a;\n\n/** Escapes \x96 the old way */\npublic class Escape {\n  public static String escape(String input) {\n".to_vec();
  text.extend_from_slice(format!("    return {body};\n  }}\n\n  public static int other() {{\n    return 1;\n  }}\n}}\n").as_bytes());
  text
}

/// A text file larger than a source file the summaries parse (`code_symbols::extract::MAX_BYTES`), one line of which
/// names the artifact `artifact`
fn large_build_file(artifact: &str) -> Vec<u8> {
  let mut text = String::new();
  for index in 0..24_000 {
    writeln!(text, "maven.install(name = \"filler_{index}\", artifacts = [\"org.example:filler-{index}:1.0\"])").unwrap();
  }
  writeln!(text, "maven.install(name = \"{artifact}\")").unwrap();
  text.into_bytes()
}

/// The plan of a range is the same whether its diffs come from git, their blobs then in the memory of this process, or
/// from the object cache, the texts then read with the summaries: a file not in UTF-8 is parsed both times, and a text
/// larger than a parsed source file is read both times
#[test]
fn a_plan_from_the_object_cache_is_the_plan_from_git() {
  let repo = TestRepo::new();
  assert!(large_build_file("zorblax_fetcher").len() > code_symbols::extract::MAX_BYTES);
  let base = commit_bytes(
    &repo,
    "Initial",
    &[
      ("src/a/Escape.java", &java("input")),
      ("lib/MODULE.bazel", &large_build_file("zorblax_fetcher")),
      ("deps.json", b"{\n  \"fetcher\": \"zorblax_fetcher\"\n}\n"),
    ],
  );
  commit_bytes(&repo, "Escape the input", &[("src/a/Escape.java", &java("input.replace(\"<\", \"&lt;\")"))]);
  let head = commit_bytes(
    &repo,
    "Drop the fetcher",
    &[
      ("lib/MODULE.bazel", &large_build_file("zorblax_fetcher_v2")),
      ("deps.json", b"{\n  \"fetcher\": \"zorblax_fetcher_v2\"\n}\n"),
    ],
  );

  let (_dir, cache) = empty_cache();
  let from_git = plan_with(&repo, &cache, &base, &head);
  let from_cache = plan_with(&repo, &cache, &base, &head);
  let java_units: Vec<(&UnitKind, Option<&str>)> = from_git
    .units
    .iter()
    .filter(|unit| unit.slices.iter().any(|slice| slice.path == "src/a/Escape.java"))
    .map(|unit| (&unit.kind, unit.name.as_deref()))
    .collect();
  assert!(
    !java_units.iter().any(|(kind, _)| **kind == UnitKind::Other { reason: OtherReason::Unparsed }),
    "{java_units:?}"
  );
  assert_eq!(from_cache, from_git);
}

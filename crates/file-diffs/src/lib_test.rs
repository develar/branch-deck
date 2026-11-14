//! The revision of psi-syntax (`PSI_SYNTAX_REVISION`) against the one cargo builds.

use crate::PSI_SYNTAX_REVISION;

#[test]
fn the_psi_syntax_revision_is_the_one_cargo_builds() {
  // `summaries_version` hashes the revision: a cached summary of another parser is made again. `code-tokens` parses
  // by the parsers of its own revision, and the service gives this one to `code_tokens::tokens_version`, so a stored
  // diff of another parser is made again too: every crate of psi-syntax that Cargo.lock holds is at this revision, but
  // `lmdb-env`, which the workspace pins apart. Cargo.lock alone: `--locked` holds it to the workspace's dependencies
  let lock = include_str!("../../../Cargo.lock");
  let source = "source = \"git+https://github.com/develar/psi-syntax?rev=";
  let locked: Vec<(&str, &str)> = lock
    .split("[[package]]\n")
    .filter_map(|package| {
      let name = package.strip_prefix("name = \"")?.split('"').next()?;
      let revision = package.split_once(source)?.1.split('#').next()?;
      Some((name, revision))
    })
    .collect();
  for (name, revision) in locked.iter().filter(|(name, _)| *name != "lmdb-env") {
    assert_eq!(*revision, PSI_SYNTAX_REVISION, "{name} is not at {PSI_SYNTAX_REVISION} in Cargo.lock");
  }
  for name in [
    "csharp-syntax",
    "java-syntax",
    "js-syntax",
    "kotlin-syntax",
    "psi-builder",
    "rust-syntax",
    "starlark-syntax",
    "code-tokens",
    "code-symbols",
    "line-diff",
    "diff-marks",
  ] {
    assert!(locked.iter().any(|(locked, _)| *locked == name), "{name} is not in Cargo.lock");
  }
}

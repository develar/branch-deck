//! Lock files and generated files: changes nobody writes by hand, left out of dependencies, overlaps and line counts by
//! default (`--include-generated` keeps them)

use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashSet;
use tracing::instrument;

/// File names of package manager lock files
const LOCK_FILES: [&str; 22] = [
  "Cargo.lock",
  "Gemfile.lock",
  "Package.resolved",
  "Pipfile.lock",
  "Podfile.lock",
  "bun.lock",
  "bun.lockb",
  "composer.lock",
  "deno.lock",
  "flake.lock",
  "go.sum",
  "go.work.sum",
  "gradle.lockfile",
  "mix.lock",
  "npm-shrinkwrap.json",
  "package-lock.json",
  "packages.lock.json",
  "pdm.lock",
  "pnpm-lock.yaml",
  "poetry.lock",
  "uv.lock",
  "yarn.lock",
];

pub fn is_lock_file(path: &str) -> bool {
  LOCK_FILES.contains(&file_name(path))
}

/// Files that changes in many unrelated areas touch: module and build registries, file lists and docs. They tell
/// nothing about where a commit belongs, so the areas leave them out. A pattern matches the file name, with `*` at its
/// start or end for any text; a pattern with a `/` matches the end of the path
const HUB_FILES: [&str; 5] = [".idea/modules.xml", "MODULE.bazel", "*-file-list.txt", "README*", "CHANGELOG*"];

pub fn is_hub_file(path: &str) -> bool {
  let name = file_name(path);
  HUB_FILES.iter().any(|pattern| {
    if let Some(suffix) = pattern.strip_prefix('*') {
      name.ends_with(suffix)
    } else if let Some(prefix) = pattern.strip_suffix('*') {
      name.starts_with(prefix)
    } else if pattern.contains('/') {
      path.strip_suffix(pattern).is_some_and(|rest| rest.is_empty() || rest.ends_with('/'))
    } else {
      name == *pattern
    }
  })
}

fn file_name(path: &str) -> &str {
  path.rsplit('/').next().unwrap_or(path)
}

/// The paths that are lock files, or that the attributes of HEAD mark `linguist-generated` or `-diff`, from one
/// `check-attr` process for all of them
#[instrument(skip(git, paths))]
pub fn generated_paths<'a>(git: &GitCommandExecutor, repository: &str, paths: impl IntoIterator<Item = &'a str>) -> Result<HashSet<String>> {
  let mut generated = HashSet::new();
  let mut input = String::new();
  for path in paths.into_iter().collect::<HashSet<_>>() {
    if is_lock_file(path) {
      generated.insert(path.to_string());
    } else {
      input.push_str(path);
      input.push('\0');
    }
  }
  if input.is_empty() {
    return Ok(generated);
  }

  let output = git.execute_command_with_input_raw(&["check-attr", "--source=HEAD", "--stdin", "-z", "linguist-generated", "diff"], repository, &input)?;
  // `<path>` NUL `<attribute>` NUL `<value>` NUL
  let fields: Vec<&[u8]> = output.split(|&b| b == 0).collect();
  for [path, attribute, value] in fields.as_chunks::<3>().0 {
    if matches!((*attribute, *value), (b"linguist-generated", b"set" | b"true") | (b"diff", b"unset")) {
      generated.insert(String::from_utf8_lossy(path).into_owned());
    }
  }
  Ok(generated)
}

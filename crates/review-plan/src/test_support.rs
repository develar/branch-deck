//! What the tests of the plan share: the plan of a test repository without a cache

use crate::mark_reviewed::{MarkReviewedParams, mark_reviewed_core};
use crate::{PlanSources, ReviewPlan, ReviewPlanParams, review_plan_core};
use file_diffs::accio::{AccioCli, DeclarationAt, DeclaredModule, FixtureIndex, ModuleBlock, ModulesAnswer, PathModule, References, SymbolHit, SymbolIndex, TestReach};
use file_diffs::service::FileDiffs;
use object_cache::cache::ObjectCache;
use std::ffi::OsStr;
use sync_utils::child_process::{ChildPriority, windowless_command_at};
use test_utils::env_vars::EnvVars;
use test_utils::git_test_utils::{TestRepo, git_command};

/// The branch prefix of the tests
pub(crate) const PREFIX: &str = "jane";

/// The plan of `params` in `repo`, with `index` for the fan-in
pub(crate) fn plan_of(repo: &TestRepo, index: Option<&dyn SymbolIndex>, params: &ReviewPlanParams) -> anyhow::Result<ReviewPlan> {
  let (cache, file_diffs) = (ObjectCache::disabled(), FileDiffs::for_scans());
  let sources = PlanSources {
    cache: &cache,
    file_diffs: &file_diffs,
    index,
  };
  review_plan_core(repo.git_executor(), sources, PREFIX, params)
}

/// The accio of this machine with an index of `repo` of its own, built of its head: its `ACCIO_HOME` in the git
/// directory of `repo`, set while the returned guard lives. `None`, and nothing indexed, on a machine without an accio
/// that answers `moves` and `modules` as the plan reads them (`AccioCli::answers_moves`, `AccioCli::answers_modules`)
fn indexed_by_accio(repo: &TestRepo) -> Option<(&'static AccioCli, EnvVars)> {
  let accio = AccioCli::discover().filter(|accio| accio.answers_moves() && accio.answers_modules())?;
  let home = repo.path().join(".git/accio-home");
  let env = EnvVars::set(&[("ACCIO_HOME", home.as_os_str()), ("ACCIO_NO_LIBS", OsStr::new("1"))]);
  let indexed = windowless_command_at(accio.executable(), ChildPriority::Normal)
    .args(["--project", repo.path_str(), "index", "--quiet"])
    .output()
    .unwrap();
  assert!(indexed.status.success(), "{}", String::from_utf8_lossy(&indexed.stderr));
  Some((accio, env))
}

/// The plan of `base..head` of `repo` with the accio of this machine and an index of `repo` of its own
/// (`indexed_by_accio`); `None`, and nothing asked, on a machine without one
pub(crate) fn plan_by_accio(repo: &TestRepo, base: &str, head: &str) -> Option<ReviewPlan> {
  let (accio, _env) = indexed_by_accio(repo)?;
  let canceled = || false;
  let search = accio.search(&canceled);
  Some(plan_of(repo, Some(&search), &range(repo, base, head)).unwrap())
}

/// The plan of `base..head` of `repo`, with an index that answers only the modules (`DeclaredModules`)
pub(crate) fn plan_of_range(repo: &TestRepo, base: &str, head: &str) -> ReviewPlan {
  plan_of(repo, Some(&DeclaredModules), &range(repo, base, head)).unwrap()
}

/// The modules of `paths` at `revision` as accio answers them for the trees of these tests (`accio modules --paths`):
/// a directory with an `*.iml`, a `*.csproj`, a Bazel, Cargo, npm, Maven or Gradle build file, a `go.mod` or a
/// `pyproject.toml` is a module root, named by its shortest `*.iml` stem, else by the `name` of its `Cargo.toml` or
/// `package.json`. `the_modules_of_a_review_are_those_accio_answers` holds it to the accio of this machine
pub(crate) fn declared_modules(repository_root: &str, paths: &[String], revision: &str) -> ModulesAnswer {
  let git = |args: &[&str]| {
    let output = git_command().arg("-C").arg(repository_root).args(args).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
  };
  let files = git(&["ls-tree", "-r", "-z", "--name-only", revision]);
  let mut declared: std::collections::BTreeMap<String, DeclaredModule> = std::collections::BTreeMap::new();
  for file in files.split('\0').filter(|file| !file.is_empty()) {
    let (root, name) = file.rsplit_once('/').unwrap_or(("", file));
    let block = |name: Option<String>| Some(ModuleBlock { name });
    let text = || git(&["show", &format!("{revision}:{file}")]);
    let quoted = |text: &str, key: &str| {
      text.lines().find_map(|line| {
        let rest = line.trim().strip_prefix(key)?.trim_start().strip_prefix([':', '='])?.trim().trim_end_matches(',');
        Some(rest.trim_matches('"').to_string())
      })
    };
    let module = || DeclaredModule {
      root: root.to_string(),
      ..DeclaredModule::default()
    };
    if let Some(stem) = name.strip_suffix(".iml") {
      let entry = declared.entry(root.to_string()).or_insert_with(module);
      if entry.jps.as_ref().and_then(|jps| jps.name.as_ref()).is_none_or(|known| stem.len() < known.len()) {
        entry.jps = block(Some(stem.to_string()));
      }
      continue;
    }
    if let Some(stem) = name.strip_suffix(".csproj") {
      declared.entry(root.to_string()).or_insert_with(module).msbuild = block(Some(stem.to_string()));
      continue;
    }
    match name {
      "BUILD.bazel" | "BUILD" => {
        declared.entry(root.to_string()).or_insert_with(module);
      }
      "Cargo.toml" => declared.entry(root.to_string()).or_insert_with(module).cargo = block(quoted(&text(), "name")),
      "package.json" => declared.entry(root.to_string()).or_insert_with(module).npm = block(quoted(&text(), "\"name\"")),
      "pom.xml" => declared.entry(root.to_string()).or_insert_with(module).maven = block(None),
      "build.gradle" | "build.gradle.kts" => declared.entry(root.to_string()).or_insert_with(module).gradle = block(Some(root.rsplit('/').next().unwrap_or(root).to_string())),
      "go.mod" => declared.entry(root.to_string()).or_insert_with(module).go = block(None),
      "pyproject.toml" => declared.entry(root.to_string()).or_insert_with(module).python = block(None),
      _ => {}
    }
  }
  let nearest = |path: &str| {
    declared
      .keys()
      .filter(|root| root.is_empty() || path.strip_prefix(root.as_str()).is_some_and(|rest| rest.starts_with('/')))
      .max_by_key(|root| root.len())
      .cloned()
  };
  let paths: Vec<PathModule> = paths
    .iter()
    .map(|path| PathModule {
      path: path.clone(),
      root: nearest(path),
    })
    .collect();
  let roots: std::collections::BTreeSet<&str> = paths.iter().filter_map(|path| path.root.as_deref()).collect();
  ModulesAnswer {
    complete: true,
    note: None,
    modules: declared.values().filter(|module| roots.contains(module.root.as_str())).cloned().collect(),
    paths,
  }
}

/// The `modules` of the fixture of the index (`FixtureIndex`) for `revisions` of `repo`: what each of their trees
/// declares, as `declared_modules` answers it
pub(crate) fn fixture_modules(repo: &TestRepo, revisions: &[&str]) -> serde_json::Value {
  let block = |block: &Option<ModuleBlock>| block.as_ref().map(|block| serde_json::json!({ "name": block.name }));
  revisions
    .iter()
    .map(|revision| {
      let files = git_command()
        .arg("-C")
        .arg(repo.path())
        .args(["ls-tree", "-r", "-z", "--name-only", revision])
        .output()
        .unwrap();
      let paths: Vec<String> = String::from_utf8(files.stdout)
        .unwrap()
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .collect();
      let modules: Vec<serde_json::Value> = declared_modules(repo.path_str(), &paths, revision)
        .modules
        .iter()
        .map(|module| serde_json::json!({ "root": module.root, "jps": block(&module.jps), "cargo": block(&module.cargo), "npm": block(&module.npm) }))
        .collect();
      ((*revision).to_string(), serde_json::Value::Array(modules))
    })
    .collect::<serde_json::Map<String, serde_json::Value>>()
    .into()
}

/// An index that answers the modules of the trees of a test repository (`declared_modules`), and the rest as the
/// fixture of its index does (`FixtureIndex`: nothing without one)
pub(crate) struct DeclaredModules;

impl SymbolIndex for DeclaredModules {
  fn symbols(&self, repository_root: &str, query: &str, revision: Option<&str>) -> anyhow::Result<Option<Vec<SymbolHit>>> {
    FixtureIndex.symbols(repository_root, query, revision)
  }

  fn references(&self, repository_root: &str, name: &str, revision: Option<&str>) -> anyhow::Result<Option<References>> {
    FixtureIndex.references(repository_root, name, revision)
  }

  fn references_at(&self, repository_root: &str, declarations: &[DeclarationAt], revision: &str) -> anyhow::Result<Option<Vec<References>>> {
    FixtureIndex.references_at(repository_root, declarations, revision)
  }

  fn tests(&self, repository_root: &str, declarations: &[DeclarationAt], given: &[String], revision: &str) -> anyhow::Result<Option<Vec<TestReach>>> {
    FixtureIndex.tests(repository_root, declarations, given, revision)
  }

  fn modules(&self, repository_root: &str, paths: &[String], revision: &str) -> anyhow::Result<Option<ModulesAnswer>> {
    Ok(Some(declared_modules(repository_root, paths, revision)))
  }
}

/// An index whose `modules` answers as its function does, and the rest as `DeclaredModules`
pub(crate) struct ModulesBy<F>(pub F);

impl<F: Fn(&str, &[String], &str) -> anyhow::Result<Option<ModulesAnswer>> + Send + Sync> SymbolIndex for ModulesBy<F> {
  fn symbols(&self, repository_root: &str, query: &str, revision: Option<&str>) -> anyhow::Result<Option<Vec<SymbolHit>>> {
    DeclaredModules.symbols(repository_root, query, revision)
  }

  fn references(&self, repository_root: &str, name: &str, revision: Option<&str>) -> anyhow::Result<Option<References>> {
    DeclaredModules.references(repository_root, name, revision)
  }

  fn references_at(&self, repository_root: &str, declarations: &[DeclarationAt], revision: &str) -> anyhow::Result<Option<Vec<References>>> {
    DeclaredModules.references_at(repository_root, declarations, revision)
  }

  fn tests(&self, repository_root: &str, declarations: &[DeclarationAt], given: &[String], revision: &str) -> anyhow::Result<Option<Vec<TestReach>>> {
    DeclaredModules.tests(repository_root, declarations, given, revision)
  }

  fn modules(&self, repository_root: &str, paths: &[String], revision: &str) -> anyhow::Result<Option<ModulesAnswer>> {
    (self.0)(repository_root, paths, revision)
  }
}

/// The module `gui` with a box, then a commit that adds the module `media` and gives the box a player of it: the base
/// and the head
pub(crate) fn add_media(repo: &TestRepo) -> (String, String) {
  let base = repo.create_commit_with_files("Add the GUI", &[("gui/BUILD.bazel", "# gui\n"), ("gui/src/Box.kt", "class Box\n")]);
  let head = repo.create_commit_with_files(
    "Play media in the GUI",
    &[
      ("gui/src/Box.kt", "class Box(val player: Player)\n"),
      ("media/BUILD.bazel", "# media\n"),
      ("media/src/Player.kt", "class Player\n"),
    ],
  );
  (base, head)
}

/// The params of the plan of `base..head`
pub(crate) fn range(repo: &TestRepo, base: &str, head: &str) -> ReviewPlanParams {
  ReviewPlanParams {
    repository_path: repo.path_str().to_string(),
    subject: crate::ReviewSubject::Range {
      base: base.to_string(),
      head: head.to_string(),
    },
    since_reviewed: false,
    concern_edges: crate::concerns::ConcernEdges::default(),
  }
}

/// The params of the plan of the branch `auth` on the baseline `master`, since its review or whole
pub(crate) fn branch(repo: &TestRepo, since_reviewed: bool) -> ReviewPlanParams {
  ReviewPlanParams {
    repository_path: repo.path_str().to_string(),
    subject: crate::ReviewSubject::Branch {
      name: "auth".to_string(),
      base: None,
      baseline_branch: Some("master".to_string()),
    },
    since_reviewed,
    concern_edges: crate::concerns::ConcernEdges::default(),
  }
}

/// Mark the branch `auth` of `repo` reviewed, on the baseline `master`
pub(crate) fn mark(repo: &TestRepo) {
  let params = MarkReviewedParams {
    repository_path: repo.path_str().to_string(),
    branch: "auth".to_string(),
    base: None,
    baseline_branch: Some("master".to_string()),
  };
  mark_reviewed_core(repo.git_executor(), PREFIX, &params).unwrap();
}

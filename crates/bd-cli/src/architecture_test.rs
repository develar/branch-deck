//! The direction of the dependencies between the crates of the workspace, the rules of "Crates и направление
//! зависимостей" in `docs/design/rust-domain-authority.md`, checked on `cargo metadata`. The rules are data; an edge
//! that breaks one today stands in `ALLOWED` with the stage that removes it, and an entry whose edge is gone or that
//! breaks no rule fails too, so the list only shrinks. A failure names the edge, or the path of a forbidden reach.
//!
//! `--no-deps` lists the dependencies of the workspace crates only: a reach goes through workspace crates and ends at
//! the first external crate (`reqwest` is seen, the `tokio` that `reqwest` pulls in is not). An optional dependency is
//! followed as a plain one unless only the test features (`TEST_FEATURES`) activate it: a `default` feature or a
//! feature a build of the app enables (`specta`, `schemars`) is what the crate is, the fake of a service that another
//! crate's tests enable through a dev-dependency is not. A test feature that a normal edge of the reach enables
//! (`patronus = { features = ["fake"] }` in `[dependencies]`, as the e2e server declares it) is in every build of the
//! dependent crate, so the reach carries the features each edge enables (the edge's `features` and what the enabled
//! features of the crate before it turn on with `x/f`) and follows the optional edges they activate. The workspace's feature unification (`.cargo/config.toml`) is why
//! `cargo tree -p <crate>` shows the fake's edge all the same.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::process::Command;

/// The app (`src-tauri`): nothing depends on it, and only it depends on Tauri
const APP: &str = "branch-deck";
/// The base of the CLI layer, which every family builds on
const BD_CORE: &str = "bd-core";
/// The CLI that collects the families
const BD_CLI: &str = "bd-cli";
/// The feature of `bd-cli` that a family's tests enable through a dev-dependency (a test cycle that cargo allows)
const FIXTURES: &str = "fixtures";
/// The features only the tests of other crates enable, through a dev-dependency: the in-process fake of a service
/// (`fake`) and the fixtures of bd (`fixtures`). An optional dependency that only they activate is not followed; one
/// that `default` or any other feature activates is
const TEST_FEATURES: &[&str] = &["fake", FIXTURES];
/// The crates named `bd-*` that are libraries below the CLI layer, not families (`bd-clustering` becomes
/// `commit-clustering` in `s3-crates`)
const BD_LIBRARIES: &[&str] = &["bd-clustering"];
/// Tauri and its plugins; a name that ends in `*` is a prefix
const TAURI: &str = "tauri*";

/// What `from` never reaches over its normal dependencies, also transitively
struct Unreachable {
  from: &'static str,
  forbidden: &'static [&'static str],
  why: &'static str,
}

const UNREACHABLE: &[Unreachable] = &[
  Unreachable {
    from: "domain-types",
    forbidden: &["git-ops", "git-executor", "object-cache", "reqwest", "tokio", TAURI, "clap"],
    why: "the crate of the types does no IO",
  },
  Unreachable {
    from: "patronus",
    forbidden: &["git-ops", "git-executor"],
    why: "the Patronus client runs no git",
  },
  Unreachable {
    from: "sync-core",
    forbidden: &["github", "patronus", "youtrack", "file-diffs", "code-symbols"],
    why: "the sync reads no remote service, no file diff and no symbol",
  },
  Unreachable {
    from: "update-core",
    forbidden: &["github", "patronus", "youtrack"],
    why: "Update Project, the pick and the resolve sessions read no remote service: the fetch is git's",
  },
  Unreachable {
    from: "trial-check",
    forbidden: &["github", "patronus", "youtrack"],
    why: "a trial check builds and reads the commits of the repository, never a remote service",
  },
  Unreachable {
    from: "repository-view",
    forbidden: &["trial-check"],
    why: "the model holds what the checks found, it runs no trial check",
  },
];

/// An edge that breaks a rule today, and the stage of the plan that removes it
struct Allowed {
  from: &'static str,
  to: &'static str,
  removed_in: &'static str,
}

const ALLOWED: &[Allowed] = &[
  Allowed {
    from: "sync-core",
    to: "github",
    removed_in: "s3-crates",
  },
  Allowed {
    from: "sync-core",
    to: "patronus",
    removed_in: "s3-crates",
  },
  Allowed {
    from: "sync-core",
    to: "youtrack",
    removed_in: "s3-crates",
  },
  Allowed {
    from: "repository-view",
    to: "trial-check",
    removed_in: "s7-session-facts",
  },
  Allowed {
    from: "bd-remote",
    to: "bd-rewrite",
    removed_in: "s3-crates",
  },
  Allowed {
    from: APP,
    to: "bd-lease",
    removed_in: "s3-crates",
  },
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
  Normal,
  Dev,
  Build,
}

/// One dependency of a workspace crate as its manifest declares it
struct Dependency {
  /// The package name
  name: String,
  /// The name the manifest gives it (`ops = { package = "git-ops" }`), which its feature entries use (`dep:ops`,
  /// `ops/f`); the package name without one
  rename: Option<String>,
  kind: Kind,
  /// Behind a feature of the crate (`optional = true`)
  optional: bool,
  /// The features the edge enables on the dependency
  features: Vec<String>,
}

impl Dependency {
  /// The name the crate's feature entries call it
  fn key(&self) -> &str {
    self.rename.as_deref().unwrap_or(&self.name)
  }
}

/// One workspace crate: its dependencies and its features, each with the entries that enable it (`dep:x`, `x/f`, another
/// feature), the implicit feature of an optional dependency included, as `cargo metadata` lists them
#[derive(Default)]
struct Package {
  dependencies: Vec<Dependency>,
  features: BTreeMap<String, Vec<String>>,
}

/// Every workspace crate, by name
type Graph = BTreeMap<String, Package>;

/// The graph of the workspace that `cargo metadata` reports, run by the cargo that runs the tests
fn workspace_graph() -> Graph {
  let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
  let cargo = std::env::var_os("CARGO").unwrap_or_else(|| env!("CARGO").into());
  let output = Command::new(cargo)
    .args(["metadata", "--format-version", "1", "--no-deps", "--offline", "--manifest-path"])
    .arg(&manifest)
    .output()
    .expect("cannot run cargo metadata");
  assert!(output.status.success(), "cargo metadata failed: {}", String::from_utf8_lossy(&output.stderr));
  parse_metadata(&serde_json::from_slice(&output.stdout).expect("cargo metadata printed no JSON"))
}

fn parse_metadata(metadata: &Value) -> Graph {
  let text = |value: &Value, what: &str| value.as_str().unwrap_or_else(|| panic!("no {what} in {value}")).to_owned();
  let packages = metadata["packages"].as_array().expect("no packages in cargo metadata");
  packages
    .iter()
    .map(|package| {
      let dependencies = package["dependencies"].as_array().expect("no dependencies in a package");
      let dependencies = dependencies
        .iter()
        .map(|dependency| Dependency {
          name: text(&dependency["name"], "name"),
          rename: dependency["rename"].as_str().map(str::to_owned),
          kind: match dependency["kind"].as_str() {
            None => Kind::Normal,
            Some("dev") => Kind::Dev,
            Some("build") => Kind::Build,
            Some(other) => panic!("unknown dependency kind {other}"),
          },
          optional: dependency["optional"].as_bool().expect("no optional in a dependency"),
          features: dependency["features"].as_array().into_iter().flatten().map(|feature| text(feature, "feature")).collect(),
        })
        .collect();
      let features = package["features"]
        .as_object()
        .expect("no features in a package")
        .iter()
        .map(|(feature, entries)| {
          (
            feature.clone(),
            entries.as_array().into_iter().flatten().map(|entry| text(entry, "feature entry")).collect(),
          )
        })
        .collect();
      (text(&package["name"], "name"), Package { dependencies, features })
    })
    .collect()
}

/// Whether `feature` of `package` activates the optional dependency `key` (its name in the feature entries): by `dep:x`,
/// by `x/f` (not `x?/f`), or through a feature it enables
fn activates<'p>(package: &'p Package, feature: &'p str, key: &str, seen: &mut BTreeSet<&'p str>) -> bool {
  seen.insert(feature)
    && package.features.get(feature).into_iter().flatten().any(|entry| {
      entry.strip_prefix("dep:") == Some(key)
        || entry.split_once('/').is_some_and(|(name, _)| name == key)
        || (package.features.contains_key(entry) && activates(package, entry, key, seen))
    })
}

/// The features of `package` that `enabled` and `default` turn on, themselves included: a feature enables the features
/// its entries name. `default` is on in every build of the crate (what the crate is), whatever the edges say
fn enabled_closure<'p>(package: &'p Package, enabled: impl IntoIterator<Item = &'p str>) -> BTreeSet<&'p str> {
  let mut closure = BTreeSet::new();
  let mut queue: Vec<&str> = enabled.into_iter().chain(["default"]).filter(|feature| package.features.contains_key(*feature)).collect();
  while let Some(feature) = queue.pop() {
    if closure.insert(feature) {
      queue.extend(
        package
          .features
          .get(feature)
          .into_iter()
          .flatten()
          .map(String::as_str)
          .filter(|entry| package.features.contains_key(*entry)),
      );
    }
  }
  closure
}

/// Whether a reach with `enabled` of `package` on (`enabled_closure`, `default` included) follows its `dependency`: a
/// plain dependency always, an optional one when a feature outside `TEST_FEATURES` or an enabled feature activates it
fn follows(package: &Package, enabled: &BTreeSet<&str>, dependency: &Dependency) -> bool {
  !dependency.optional
    || package
      .features
      .keys()
      .filter(|feature| activates(package, feature, dependency.key(), &mut BTreeSet::new()))
      .any(|feature| !TEST_FEATURES.contains(&feature.as_str()) || enabled.contains(feature.as_str()))
}

/// The features the edge `dependency` of `package` enables on its target when `enabled` of `package` is on: the edge's
/// own and the `x/f` and `x?/f` of the enabled features
fn enabled_on<'p>(package: &'p Package, enabled: &BTreeSet<&'p str>, dependency: &'p Dependency) -> BTreeSet<&'p str> {
  let mut features: BTreeSet<&str> = dependency.features.iter().map(String::as_str).collect();
  for feature in enabled {
    for entry in package.features.get(*feature).into_iter().flatten() {
      if let Some((name, of)) = entry.split_once('/')
        && name.strip_suffix('?').unwrap_or(name) == dependency.key()
      {
        features.insert(of);
      }
    }
  }
  features
}

/// Whether `name` is `pattern`, or starts with it when it ends in `*`
fn matches(pattern: &str, name: &str) -> bool {
  pattern.strip_suffix('*').map_or(pattern == name, |prefix| name.starts_with(prefix))
}

fn is_allowed(allowed: &[&Allowed], from: &str, to: &str) -> bool {
  allowed.iter().any(|entry| entry.from == from && entry.to == to)
}

/// A crate of the CLI layer: `bd-*`, except the libraries
fn is_layer(name: &str) -> bool {
  name.starts_with("bd-") && !BD_LIBRARIES.contains(&name)
}

/// A family of the CLI layer: `bd-lease`, `bd-remote`, `bd-rewrite`, `bd-update`, and any new `bd-*`
fn is_family(name: &str) -> bool {
  is_layer(name) && name != BD_CORE && name != BD_CLI
}

/// Every crate that `from` reaches over normal edges outside `allowed` (an optional edge as `follows` decides, with the
/// features the edges of the reach enable on each crate), with its first path from `from`
fn reach<'g>(graph: &'g Graph, allowed: &[&Allowed], from: &str) -> BTreeMap<String, Vec<String>> {
  let mut paths = BTreeMap::from([(from.to_owned(), vec![from.to_owned()])]);
  let mut enabled: BTreeMap<String, BTreeSet<&'g str>> = BTreeMap::new();
  let mut queue = VecDeque::from([from.to_owned()]);
  while let Some(name) = queue.pop_front() {
    let path = paths[&name].clone();
    let Some(package) = graph.get(&name) else {
      continue;
    };
    let closure = enabled_closure(package, enabled.get(&name).into_iter().flatten().copied());
    for dependency in &package.dependencies {
      if dependency.kind != Kind::Normal || !follows(package, &closure, dependency) || is_allowed(allowed, &name, &dependency.name) {
        continue;
      }
      let on = enabled_on(package, &closure, dependency);
      let known = enabled.entry(dependency.name.clone()).or_default();
      let grew = !on.is_subset(known);
      known.extend(on);
      if paths.contains_key(&dependency.name) {
        // a feature new to a crate already reached may turn on an optional edge of it: visit it again
        if grew {
          queue.push_back(dependency.name.clone());
        }
        continue;
      }
      let mut next = path.clone();
      next.push(dependency.name.clone());
      paths.insert(dependency.name.clone(), next);
      queue.push_back(dependency.name.clone());
    }
  }
  paths
}

/// The rule a direct edge breaks, if any
fn direct_rule(from: &str, dependency: &Dependency) -> Option<&'static str> {
  let to = dependency.name.as_str();
  if to == APP {
    return Some("nothing depends on the app");
  }
  if matches(TAURI, to) && from != APP {
    return Some("only the app depends on Tauri");
  }
  if !is_layer(to) {
    return None;
  }
  if !is_layer(from) {
    return (from != APP || (to != BD_CLI && to != BD_CORE)).then_some("outside the CLI layer only the app depends on it, and only on bd-cli and bd-core");
  }
  if from == BD_CORE {
    return Some("bd-core is the base of the CLI layer and depends on no other bd crate");
  }
  let fixtures = to == BD_CLI && dependency.kind == Kind::Dev && dependency.features.iter().any(|feature| feature == FIXTURES);
  (is_family(from) && to != BD_CORE && !fixtures)
    .then_some("a bd family depends on bd-core and the libraries, never on another family (a dev-dependency on bd-cli with `fixtures` is fine)")
}

/// Every broken rule of `graph` outside `allowed`, one line each
fn violations(graph: &Graph, allowed: &[&Allowed]) -> Vec<String> {
  let mut report = Vec::new();
  for rule in UNREACHABLE {
    for (name, path) in reach(graph, allowed, rule.from) {
      if rule.forbidden.iter().any(|pattern| matches(pattern, &name)) {
        report.push(format!("  {} reaches {name} ({}): {}", rule.from, path.join(" -> "), rule.why));
      }
    }
  }
  for (from, package) in graph {
    for dependency in package.dependencies.iter().filter(|dependency| !is_allowed(allowed, from, &dependency.name)) {
      if let Some(rule) = direct_rule(from, dependency) {
        report.push(format!("  {from} depends on {} ({:?}): {rule}", dependency.name, dependency.kind));
      }
    }
  }
  report
}

fn all_allowed() -> Vec<&'static Allowed> {
  ALLOWED.iter().collect()
}

#[test]
fn the_crates_depend_only_in_the_allowed_directions() {
  let report = violations(&workspace_graph(), &all_allowed());
  assert!(
    report.is_empty(),
    "A dependency breaks the direction of the crates:\n{}\nInstead: depend on a crate below, or move the code down \
     (docs/design/rust-domain-authority.md, \"Crates и направление зависимостей\"); ALLOWED only shrinks",
    report.join("\n")
  );
}

#[test]
fn every_allowed_edge_exists_and_breaks_a_rule() {
  let graph = workspace_graph();
  // what breaks with every entry; an entry is needed when leaving it out breaks more
  let base = violations(&graph, &all_allowed());
  let mut report = Vec::new();
  for entry in ALLOWED {
    let exists = graph
      .get(entry.from)
      .is_some_and(|package| package.dependencies.iter().any(|dependency| dependency.name == entry.to && dependency.kind == Kind::Normal));
    let others: Vec<&Allowed> = ALLOWED.iter().filter(|other| !std::ptr::eq(*other, entry)).collect();
    if !exists {
      report.push(format!("  {} -> {} (removed in {}) is gone: delete its entry", entry.from, entry.to, entry.removed_in));
    } else if violations(&graph, &others).iter().all(|violation| base.contains(violation)) {
      report.push(format!(
        "  {} -> {} (removed in {}) breaks no rule: delete its entry",
        entry.from, entry.to, entry.removed_in
      ));
    }
  }
  assert!(report.is_empty(), "ALLOWED holds an entry it no longer needs:\n{}", report.join("\n"));
}

#[test]
fn the_rules_of_the_architecture_name_crates_of_the_workspace() {
  let graph = workspace_graph();
  let named = UNREACHABLE.iter().map(|rule| rule.from).chain([APP, BD_CORE, BD_CLI]).chain(BD_LIBRARIES.iter().copied());
  let unknown: Vec<&str> = named.filter(|name| !graph.contains_key(*name)).collect();
  assert!(unknown.is_empty(), "the architecture rules name crates the workspace lacks (renamed?): {unknown:?}");
}

/// A graph of `(from, to, kind, features)` edges, none optional, each `from` a workspace crate without features
fn graph_of(edges: &[(&str, &str, Kind, &[&str])]) -> Graph {
  let mut graph = Graph::new();
  for (from, to, kind, features) in edges {
    graph.entry((*from).to_owned()).or_default().dependencies.push(Dependency {
      name: (*to).to_owned(),
      rename: None,
      kind: *kind,
      optional: false,
      features: features.iter().map(|feature| (*feature).to_owned()).collect(),
    });
  }
  graph
}

/// An optional normal dependency on `name`, as `rename` when the manifest renames it, enabling `features` of it
fn optional(name: &str, rename: Option<&str>, features: &[&str]) -> Dependency {
  Dependency {
    name: name.to_owned(),
    rename: rename.map(str::to_owned),
    kind: Kind::Normal,
    optional: true,
    features: features.iter().map(|feature| (*feature).to_owned()).collect(),
  }
}

/// The features of a package, as `cargo metadata` lists them
fn features_of(features: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
  features
    .iter()
    .map(|(feature, entries)| ((*feature).to_owned(), entries.iter().map(|entry| (*entry).to_owned()).collect()))
    .collect()
}

/// `patronus` with its plain edge to `sync-utils` and an optional one to `test-utils`, which reaches git; `features`
/// are the package's, as `cargo metadata` lists them
fn patronus_with_optional_test_utils(features: &[(&str, &[&str])]) -> Graph {
  let mut graph = graph_of(&[("test-utils", "git-executor", Kind::Normal, &[]), ("patronus", "sync-utils", Kind::Normal, &[])]);
  let patronus = graph.get_mut("patronus").unwrap();
  patronus.dependencies.push(optional("test-utils", None, &["http"]));
  patronus.features = features_of(features);
  graph
}

const PATRONUS_REACHES_GIT: &str = "  patronus reaches git-executor (patronus -> test-utils -> git-executor): the Patronus client runs no git";

#[test]
fn the_architecture_check_names_the_path_of_a_forbidden_reach() {
  let graph = graph_of(&[
    ("domain-types", "sync-utils", Kind::Normal, &[]),
    ("sync-utils", "git-executor", Kind::Normal, &[]),
    ("domain-types", "tokio", Kind::Dev, &[]),
    ("domain-types", "tauri-build", Kind::Build, &[]),
  ]);
  assert_eq!(
    violations(&graph, &[]),
    [
      "  domain-types reaches git-executor (domain-types -> sync-utils -> git-executor): the crate of the types does no IO",
      "  domain-types depends on tauri-build (Build): only the app depends on Tauri",
    ]
  );
  let allowed = Allowed {
    from: "sync-utils",
    to: "git-executor",
    removed_in: "s2-types",
  };
  assert_eq!(
    violations(&graph, &[&allowed]),
    ["  domain-types depends on tauri-build (Build): only the app depends on Tauri"]
  );
}

#[test]
fn the_architecture_check_follows_no_optional_edge_only_a_test_feature_enables() {
  // the fake of a service, behind its `fake` feature, reaches the test utilities and git; the client does not
  let mut graph = patronus_with_optional_test_utils(&[("fake", &["dep:axum", "dep:test-utils"])]);
  assert_eq!(violations(&graph, &[]), Vec::<String>::new());
  // the same edge as a plain dependency is the reach it hides
  graph.get_mut("patronus").unwrap().dependencies.last_mut().unwrap().optional = false;
  assert_eq!(violations(&graph, &[]), [PATRONUS_REACHES_GIT]);
}

#[test]
fn the_architecture_check_follows_an_optional_edge_a_default_feature_enables() {
  // every build of the crate enables `default`, so what it activates is what the crate is, a test feature included
  let graph = patronus_with_optional_test_utils(&[("default", &["fake"]), ("fake", &["dep:axum", "dep:test-utils"])]);
  assert_eq!(violations(&graph, &[]), [PATRONUS_REACHES_GIT]);
  // through a chain of features, and by a feature of the dependency (`x/f` activates the optional `x`)
  let graph = patronus_with_optional_test_utils(&[("default", &["full"]), ("full", &["fake"]), ("fake", &["test-utils/http"])]);
  assert_eq!(violations(&graph, &[]), [PATRONUS_REACHES_GIT]);
  // `x?/f` activates nothing; `other`, which nothing enables, is a reach all the same: any feature outside
  // `TEST_FEATURES` counts, enabled or not, as the build of the app may enable it
  let graph = patronus_with_optional_test_utils(&[("default", &["fake"]), ("fake", &["test-utils?/http"]), ("other", &["dep:test-utils"])]);
  let patronus = &graph["patronus"];
  assert!(!activates(patronus, "fake", "test-utils", &mut BTreeSet::new()));
  assert!(activates(patronus, "other", "test-utils", &mut BTreeSet::new()));
  assert_eq!(violations(&graph, &[]), [PATRONUS_REACHES_GIT]);
  // with only `x?/f`, cargo lists the implicit feature `x = ["dep:x"]`, which is no test feature: a reach
  let graph = patronus_with_optional_test_utils(&[("default", &[]), ("fake", &["test-utils?/http"]), ("test-utils", &["dep:test-utils"])]);
  assert_eq!(violations(&graph, &[]), [PATRONUS_REACHES_GIT]);
}

#[test]
fn the_architecture_check_follows_an_optional_edge_a_test_feature_a_normal_dependency_enables() {
  // every build of a crate whose `[dependencies]` enable `fake` of github links the fake, so the reach follows the
  // optional edge behind `fake` from there, though from github itself it does not
  let fake = [("fake", &["dep:test-utils"] as &[&str])];
  let mut graph = graph_of(&[("patronus", "github", Kind::Normal, &["fake"]), ("test-utils", "git-executor", Kind::Normal, &[])]);
  let github = graph.entry("github".to_owned()).or_default();
  github.dependencies.push(optional("test-utils", None, &[]));
  github.features = features_of(&fake);
  assert_eq!(
    violations(&graph, &[]),
    ["  patronus reaches git-executor (patronus -> github -> test-utils -> git-executor): the Patronus client runs no git"]
  );
  // the same edge without the feature: the fake stays off, the reach stops at github
  graph.get_mut("patronus").unwrap().dependencies[0].features.clear();
  assert_eq!(violations(&graph, &[]), Vec::<String>::new());
  // a feature of the dependent that enables it (`fixtures = ["patronus/fake"]` of bd-cli), itself enabled by the edge
  // before, and a crate reached first without the feature is visited again with it
  let mut graph = graph_of(&[
    ("domain-types", "bd-cli", Kind::Normal, &["fixtures"]),
    ("domain-types", "patronus", Kind::Normal, &[]),
    ("bd-cli", "patronus", Kind::Normal, &[]),
    ("test-utils", "git-executor", Kind::Normal, &[]),
  ]);
  graph.get_mut("bd-cli").unwrap().features = features_of(&[("fixtures", &["patronus/fake"])]);
  let patronus = graph.entry("patronus".to_owned()).or_default();
  patronus.dependencies.push(optional("test-utils", None, &[]));
  patronus.features = features_of(&fake);
  assert_eq!(
    violations(&graph, &[]),
    [
      "  domain-types reaches git-executor (domain-types -> patronus -> test-utils -> git-executor): the crate of the types does no IO",
      "  domain-types depends on bd-cli (Normal): outside the CLI layer only the app depends on it, and only on bd-cli and bd-core",
    ]
  );
}

#[test]
fn the_architecture_check_follows_an_optional_edge_by_the_name_the_manifest_gives_it() {
  // `ops = { package = "git-ops", optional = true }`: the feature entries say `dep:ops`, the package is git-ops
  let mut graph = graph_of(&[("domain-types", "serde", Kind::Normal, &[]), ("git-ops", "git-executor", Kind::Normal, &[])]);
  let types = graph.get_mut("domain-types").unwrap();
  types.dependencies.push(optional("git-ops", Some("ops"), &[]));
  types.features = features_of(&[("default", &["dep:ops"])]);
  assert_eq!(
    violations(&graph, &[]),
    [
      "  domain-types reaches git-executor (domain-types -> git-ops -> git-executor): the crate of the types does no IO",
      "  domain-types reaches git-ops (domain-types -> git-ops): the crate of the types does no IO",
    ]
  );
}

#[test]
fn the_architecture_check_follows_an_optional_edge_a_build_feature_enables() {
  // `schemars` of domain-types, which bd-core enables for `bd schema`: an optional git-ops behind it is a reach
  let mut graph = graph_of(&[("domain-types", "serde", Kind::Normal, &[]), ("git-ops", "git-executor", Kind::Normal, &[])]);
  let types = graph.get_mut("domain-types").unwrap();
  types.dependencies.push(optional("git-ops", None, &[]));
  types.features = features_of(&[("schemars", &["dep:schemars", "dep:git-ops"])]);
  assert_eq!(
    violations(&graph, &[]),
    [
      "  domain-types reaches git-executor (domain-types -> git-ops -> git-executor): the crate of the types does no IO",
      "  domain-types reaches git-ops (domain-types -> git-ops): the crate of the types does no IO",
    ]
  );
}

#[test]
fn the_architecture_check_keeps_the_bd_families_apart() {
  let graph = graph_of(&[
    ("bd-lease", "bd-core", Kind::Normal, &[]),
    ("bd-lease", "bd-clustering", Kind::Normal, &[]),
    ("bd-lease", "bd-cli", Kind::Dev, &[FIXTURES]),
    ("bd-update", "bd-cli", Kind::Dev, &[]),
    ("bd-update", "bd-lease", Kind::Normal, &[]),
    ("bd-core", "bd-update", Kind::Dev, &[]),
    (APP, "bd-cli", Kind::Normal, &[]),
    (APP, "bd-update", Kind::Normal, &[]),
    ("test-server", "bd-core", Kind::Normal, &[]),
    ("test-server", APP, Kind::Dev, &[]),
  ]);
  let family = "a bd family depends on bd-core and the libraries, never on another family (a dev-dependency on bd-cli with `fixtures` is fine)";
  let outside = "outside the CLI layer only the app depends on it, and only on bd-cli and bd-core";
  assert_eq!(
    violations(&graph, &[]),
    [
      "  bd-core depends on bd-update (Dev): bd-core is the base of the CLI layer and depends on no other bd crate".to_owned(),
      format!("  bd-update depends on bd-cli (Dev): {family}"),
      format!("  bd-update depends on bd-lease (Normal): {family}"),
      format!("  branch-deck depends on bd-update (Normal): {outside}"),
      format!("  test-server depends on bd-core (Normal): {outside}"),
      "  test-server depends on branch-deck (Dev): nothing depends on the app".to_owned(),
    ]
  );
}

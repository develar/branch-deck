//! The direction of the dependencies between the crates of the workspace, the rules of "Crates и направление
//! зависимостей" in `docs/design/rust-domain-authority.md`, checked on `cargo metadata`. The rules are data; an edge
//! that breaks one today stands in `ALLOWED` with the stage that removes it, and an entry whose edge is gone or that
//! breaks no rule fails too, so the list only shrinks. A failure names the edge, or the path of a forbidden reach.
//!
//! `--no-deps` lists the dependencies of the workspace crates only: a reach goes through workspace crates and ends at
//! the first external crate (`reqwest` is seen, the `tokio` that `reqwest` pulls in is not).

use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
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
    forbidden: &["git-ops"],
    why: "the Patronus client runs no git",
  },
  Unreachable {
    from: "sync-core",
    forbidden: &["github", "patronus", "youtrack", "file-diffs", "code-symbols"],
    why: "the sync reads no remote service, no file diff and no symbol",
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
    from: "patronus",
    to: "git-ops",
    removed_in: "s2-types",
  },
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
    from: "sync-core",
    to: "file-diffs",
    removed_in: "s3-crates",
  },
  Allowed {
    from: "sync-core",
    to: "code-symbols",
    removed_in: "s3-crates",
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
  name: String,
  kind: Kind,
  features: Vec<String>,
}

/// Every workspace crate and its dependencies, by name
type Graph = BTreeMap<String, Vec<Dependency>>;

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
          kind: match dependency["kind"].as_str() {
            None => Kind::Normal,
            Some("dev") => Kind::Dev,
            Some("build") => Kind::Build,
            Some(other) => panic!("unknown dependency kind {other}"),
          },
          features: dependency["features"].as_array().into_iter().flatten().map(|feature| text(feature, "feature")).collect(),
        })
        .collect();
      (text(&package["name"], "name"), dependencies)
    })
    .collect()
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

/// Every crate that `from` reaches over normal edges outside `allowed`, with its path from `from`
fn reach(graph: &Graph, allowed: &[&Allowed], from: &str) -> BTreeMap<String, Vec<String>> {
  let mut paths = BTreeMap::from([(from.to_owned(), vec![from.to_owned()])]);
  let mut queue = VecDeque::from([from.to_owned()]);
  while let Some(name) = queue.pop_front() {
    let path = paths[&name].clone();
    for dependency in graph.get(&name).into_iter().flatten() {
      if dependency.kind != Kind::Normal || is_allowed(allowed, &name, &dependency.name) || paths.contains_key(&dependency.name) {
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
  for (from, dependencies) in graph {
    for dependency in dependencies.iter().filter(|dependency| !is_allowed(allowed, from, &dependency.name)) {
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
      .is_some_and(|dependencies| dependencies.iter().any(|dependency| dependency.name == entry.to && dependency.kind == Kind::Normal));
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

/// A graph of `(from, to, kind, features)` edges, each `from` a workspace crate
fn graph_of(edges: &[(&str, &str, Kind, &[&str])]) -> Graph {
  let mut graph = Graph::new();
  for (from, to, kind, features) in edges {
    graph.entry((*from).to_owned()).or_default().push(Dependency {
      name: (*to).to_owned(),
      kind: *kind,
      features: features.iter().map(|feature| (*feature).to_owned()).collect(),
    });
  }
  graph
}

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

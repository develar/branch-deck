//! What the changes of each branch touch where it lands (`landing_footprints`): the required checks of Patronus and the
//! code owners of the tree of the baseline

use crate::check::CheckSession;
use crate::footprint::landing_footprints;
use domain_types::landing::BranchFootprint;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use test_utils::process_budget::repo_on_origin;

const PREFIX: &str = "test";

const CONFIG: &str = r#"version: "1.0"
checks:
- configurationId: "Air"
  name: "Air Plugin Tests"
  executeOnChanges:
  - "plugins/air/**"
  - "!/**/README.md"
- configurationId: "Build"
  name: "Build Scripts"
  executeOnChanges:
  - "build/**"
- configurationId: "AirOptional"
  name: "Air Optional"
  optional: true
  executeOnChanges:
  - "plugins/air/**"
- configurationId: "Inspections"
  name: "Zero Tolerance Inspections"
  executeOnChanges:
  - "*"
"#;

/// The baseline holds the config and the `OWNERSHIP` of the Air plugin and of the build; `air` changes the plugin and
/// its readme, `cache` the build, `docs` only a readme of the plugin
#[test]
fn each_branch_starts_the_required_checks_of_its_paths_and_has_their_owners() {
  let repo = repo_on_origin();
  let baseline = repo.create_commit_with_files(
    "Initial config",
    &[
      (".patronus/config.yaml", CONFIG),
      ("plugins/air/OWNERSHIP", "owner 'Air'\n"),
      ("build/OWNERSHIP", "owner 'Build'\n"),
    ],
  );
  repo.git(&["update-ref", "refs/remotes/origin/master", &baseline]);
  repo.create_commit_with_files(
    "(air) Watch the home",
    &[("plugins/air/core/Home.kt", "class Home\n"), ("plugins/air/README.md", "# Air\n")],
  );
  repo.create_commit_with_files("(cache) Pack the cache", &[("build/src/Cache.kt", "class Cache\n")]);
  repo.create_commit_with_files("(docs) Say how", &[("plugins/air/README.md", "# Air, how\n")]);
  let session = CheckSession::open(&GitCommandExecutor::new(), repo.path_str(), PREFIX, &ObjectCache::disabled()).unwrap();
  let footprints = landing_footprints(&session);
  let of = |checks: &[&str], owners: &[&str]| BranchFootprint {
    checks: checks.iter().map(|name| (*name).to_string()).collect(),
    owners: owners.iter().map(|name| (*name).to_string()).collect(),
  };
  assert_eq!(footprints.checks_unknown, None);
  assert_eq!(footprints.owners_unknown, None);
  assert_eq!(
    footprints.branches.into_iter().collect::<Vec<_>>(),
    [
      ("air".to_string(), of(&["Air Plugin Tests", "Zero Tolerance Inspections"], &["Air"])),
      ("cache".to_string(), of(&["Build Scripts", "Zero Tolerance Inspections"], &["Build"])),
      ("docs".to_string(), of(&["Zero Tolerance Inspections"], &["Air"])),
    ]
  );
}

/// A baseline without `.patronus/config.yaml`: the checks are unknown with the reason, the owners known
#[test]
fn without_the_config_the_checks_are_unknown() {
  let repo = repo_on_origin();
  repo.create_commit_with_files("(air) Watch the home", &[("plugins/air/Home.kt", "class Home\n")]);
  let session = CheckSession::open(&GitCommandExecutor::new(), repo.path_str(), PREFIX, &ObjectCache::disabled()).unwrap();
  let footprints = landing_footprints(&session);
  assert_eq!(footprints.checks_unknown.as_deref(), Some("the baseline has no .patronus/config.yaml"));
  assert_eq!(footprints.owners_unknown, None);
  assert_eq!(footprints.branches["air"], BranchFootprint::default());
}

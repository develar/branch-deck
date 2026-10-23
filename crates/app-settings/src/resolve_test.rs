use crate::resolve::{cli_branch_prefix, resolve_branch_prefix};
use domain_types::settings::{GitBranchPrefix, PrefixSource, ResolvedBranchPrefix};
use pretty_assertions::assert_eq;

fn git(prefix: &str, repository_scoped: bool) -> GitBranchPrefix {
  GitBranchPrefix {
    prefix: prefix.to_owned(),
    repository_scoped,
  }
}

fn resolved(prefix: &str, source: PrefixSource) -> ResolvedBranchPrefix {
  ResolvedBranchPrefix {
    prefix: prefix.to_owned(),
    source,
  }
}

#[test]
fn the_specific_prefix_wins() {
  let cases = [
    // the app's prefix only
    (git("", false), Some("app"), resolved("app", PrefixSource::AppSetting)),
    // the repository's own git prefix wins over the app's, `git -c` too (a repository-scoped one)
    (git("local", true), Some("app"), resolved("local", PrefixSource::RepositoryConfig)),
    (git("local", true), None, resolved("local", PrefixSource::RepositoryConfig)),
    // the app's prefix wins over the global git prefix
    (git("global", false), Some("app"), resolved("app", PrefixSource::AppSetting)),
    (git("global", false), None, resolved("global", PrefixSource::GlobalConfig)),
    (git("global", false), Some("  "), resolved("global", PrefixSource::GlobalConfig)),
    // a repository config that sets an empty prefix sets none
    (git("", true), Some("app"), resolved("app", PrefixSource::AppSetting)),
    (git("", true), None, ResolvedBranchPrefix::none()),
    (git("", false), None, ResolvedBranchPrefix::none()),
  ];
  for (git, app, expected) in cases {
    assert_eq!(resolve_branch_prefix(&git, app), expected, "{git:?} {app:?}");
  }
}

#[test]
fn the_prefix_is_named_as_the_branches_are_and_one_of_slashes_only_is_unset() {
  // no trailing slash, as `qualified_branch_name` names the branches (`normalizeBranchPrefix` of the page)
  assert_eq!(
    resolve_branch_prefix(&git("local/ ", true), Some("app/")),
    resolved("local", PrefixSource::RepositoryConfig)
  );
  assert_eq!(resolve_branch_prefix(&git("", false), Some(" app/")), resolved("app", PrefixSource::AppSetting));
  // nothing is left of `/`: the next prefix in the order
  assert_eq!(resolve_branch_prefix(&git("/", true), Some("app")), resolved("app", PrefixSource::AppSetting));
  assert_eq!(resolve_branch_prefix(&git("global", false), Some("/")), resolved("global", PrefixSource::GlobalConfig));
}

#[test]
fn the_prefix_of_the_command_line_is_taken_as_it_is_named() {
  assert_eq!(cli_branch_prefix(" jane/ "), resolved("jane", PrefixSource::Cli));
  assert_eq!(cli_branch_prefix("  "), ResolvedBranchPrefix::none());
}

#[test]
fn the_source_is_written_in_snake_case() {
  let json = serde_json::to_value(resolved("jane", PrefixSource::RepositoryConfig)).unwrap();
  assert_eq!(json, serde_json::json!({ "prefix": "jane", "source": "repository_config" }));
}

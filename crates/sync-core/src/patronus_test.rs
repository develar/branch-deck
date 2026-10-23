use crate::patronus::{SpaceRepository, attribute_paths_by_ownership, find_target_branch, needs_ownership_paths, parse_space_repository};
use domain_types::patronus::{PatronusReviewRequirement, PatronusReviewerGroup, ReviewPathsSource};
use git_ops::ownership::OwnershipRules;
use test_utils::git_test_utils::TestRepo;

#[test]
fn parses_space_remote_urls() {
  let ultimate = Some(SpaceRepository {
    project_key: "ij".to_string(),
    name: "ultimate".to_string(),
  });
  for url in [
    "ssh://git@git.jetbrains.team/ij/ultimate.git",
    "git@git.jetbrains.team:ij/ultimate.git",
    "https://git.jetbrains.team/ij/ultimate.git",
    "https://user:token@git.jetbrains.team/ij/ultimate",
    "ssh://git@Git.JetBrains.Team:22/ij/ultimate.git\n",
  ] {
    assert_eq!(parse_space_repository(url), ultimate, "{url}");
  }
}

#[test]
fn rejects_remote_urls_outside_space() {
  for url in [
    "git@github.com:JetBrains/intellij-community.git",
    "https://jetbrains.team/p/ij/repositories/ultimate",
    "ssh://git@git.jetbrains.team/ultimate.git",
    "/Users/me/ultimate",
    "",
  ] {
    assert_eq!(parse_space_repository(url), None, "{url}");
  }
}

#[test]
fn the_target_is_the_branch_of_origin_the_baseline_tracks() {
  let upstream = TestRepo::new();
  upstream.create_commit("Initial", "file.txt", "content");
  let clone = TestRepo::new_empty();
  clone.clone_from(upstream.path()).unwrap();
  let path = clone.path().to_str().unwrap();
  let git = clone.git_executor();
  let default_branch = git.execute_command(&["rev-parse", "--abbrev-ref", "HEAD"], path).unwrap();

  assert_eq!(find_target_branch(git, path, &format!("origin/{default_branch}")).unwrap(), Some(default_branch.clone()));
  // a local baseline has no target
  assert_eq!(find_target_branch(git, path, &default_branch).unwrap(), None);

  // a baseline on another remote: Patronus works on the push remote
  clone.add_remote("fork", upstream.path().to_str().unwrap()).unwrap();
  git.execute_command(&["fetch", "--quiet", "fork"], path).unwrap();
  assert_eq!(find_target_branch(git, path, &format!("fork/{default_branch}")).unwrap(), None);
}

fn group(name: &str, paths: &[&str], paths_source: ReviewPathsSource) -> PatronusReviewerGroup {
  PatronusReviewerGroup {
    name: name.to_string(),
    reviewers: Vec::new(),
    paths: paths.iter().map(ToString::to_string).collect(),
    branch_wide: false,
    paths_source,
    required_approvals: None,
    collected_approvals: None,
    approved: None,
    self_approval_allowed: None,
  }
}

#[test]
fn the_ownership_files_give_the_paths_to_the_groups_patronus_sent_without_them() {
  let rules = OwnershipRules::from_files([
    ("OWNERSHIP", "owner 'Monorepo'\n"),
    ("plugins/air/OWNERSHIP", "owner 'Air Plugin'\n"),
    ("build/OWNERSHIP", "owner 'IJ Build Scripts Maintainers'\n"),
  ]);
  let paths = ["build/a.kts", "plugins/air/A.kt", "plugins/air/B.kt", "README.md"].map(str::to_string);
  let mut requirement = PatronusReviewRequirement {
    required: true,
    merge_request: None,
    groups: vec![
      group(" air plugin", &[], ReviewPathsSource::Unknown),
      group("IJ Build Scripts Maintainers", &["build/a.kts"], ReviewPathsSource::Patronus),
      group("Kotlin Build Tools & Kotlin IntelliJ Plugin", &[], ReviewPathsSource::Unknown),
    ],
    skipped: false,
  };
  assert!(needs_ownership_paths(&requirement));
  attribute_paths_by_ownership(&mut requirement, &rules, &paths);
  assert_eq!(
    requirement.groups,
    vec![
      group(" air plugin", &["plugins/air/A.kt", "plugins/air/B.kt"], ReviewPathsSource::Ownership),
      group("IJ Build Scripts Maintainers", &["build/a.kts"], ReviewPathsSource::Patronus),
      // a reviewer group of another owner: only Patronus knows its paths
      group("Kotlin Build Tools & Kotlin IntelliJ Plugin", &[], ReviewPathsSource::Unknown),
    ]
  );
}

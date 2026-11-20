use crate::baseline::BaselineUpstream;
use crate::github_repository::GitHubRepository;
use crate::patronus::{SpaceRepository, attribute_paths_by_ownership, needs_ownership_paths, no_target_message, parse_space_repository, required_target_branch, target_branch_of};
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

fn upstream(remote: &str, branch: &str) -> BaselineUpstream {
  BaselineUpstream {
    remote: remote.to_string(),
    remote_ref: format!("refs/heads/{branch}"),
    tracking_ref: format!("refs/remotes/{remote}/{branch}"),
    name: format!("{remote}/{branch}"),
  }
}

#[test]
fn the_target_is_the_branch_of_origin_or_of_the_github_repository_of_the_branches() {
  let github = GitHubRepository::new("JetBrains", "ultimate");
  let master = Some("master".to_string());
  // origin, wherever it is
  let origin = upstream("origin", "master");
  assert_eq!(target_branch_of(Some(&origin), Some("ssh://git@git.jetbrains.team/ij/ultimate.git"), Some(&github)), master);
  assert_eq!(target_branch_of(Some(&origin), None, None), master);
  // the remote that fetches from the GitHub repository of the branches, which GitHub names in any case
  let on_github = upstream("github", "master");
  for url in ["https://github.com/JetBrains/ultimate", "git@github.com:jetbrains/Ultimate.git"] {
    assert_eq!(target_branch_of(Some(&on_github), Some(url), Some(&github)), master, "{url}");
  }
  // a fork, a remote of GitHub while no repository of the branches is known, a local baseline
  assert_eq!(
    target_branch_of(Some(&upstream("fork", "master")), Some("https://github.com/someone/ultimate"), Some(&github)),
    None
  );
  assert_eq!(target_branch_of(Some(&on_github), Some("https://github.com/JetBrains/ultimate"), None), None);
  assert_eq!(target_branch_of(None, None, Some(&github)), None);
}

#[test]
fn an_action_reads_the_target_with_the_urls_of_the_remotes() {
  let upstream = TestRepo::new();
  upstream.create_commit("Initial", "file.txt", "content");
  let clone = TestRepo::new_empty();
  clone.clone_from(upstream.path()).unwrap();
  let path = clone.path().to_str().unwrap();
  let git = clone.git_executor();
  let default_branch = git.execute_command(&["rev-parse", "--abbrev-ref", "HEAD"], path).unwrap();
  let github = GitHubRepository::new("owner", "repository");
  clone.add_remote("github", "https://github.com/owner/repository").unwrap();
  clone.add_remote("fork", "https://github.com/someone/repository").unwrap();

  assert_eq!(required_target_branch(git, path, &format!("origin/{default_branch}"), None).unwrap(), default_branch);
  assert_eq!(
    required_target_branch(git, path, &format!("github/{default_branch}"), Some(&github)).unwrap(),
    default_branch
  );
  for baseline in [default_branch.clone(), format!("fork/{default_branch}")] {
    let refused = required_target_branch(git, path, &baseline, Some(&github)).unwrap_err();
    assert_eq!(refused.to_string(), no_target_message(&baseline, Some(&github)));
  }
  assert_eq!(
    no_target_message("fork/master", Some(&github)),
    "The baseline fork/master is not a branch of origin or of owner/repository: a robot pushes into a branch of origin, and a pull request goes into it"
  );
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

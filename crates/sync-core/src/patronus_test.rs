use crate::patronus::{MAX_REVIEW_COMMITS, SpaceRepository, attribute_paths_by_ownership, find_target_branch, needs_ownership_paths, parse_space_repository};
use crate::patronus_review::{ReviewInput, read_review_inputs};
use git_ops::ownership::OwnershipRules;
use object_cache::cache::ObjectCache;
use std::fmt::Write;
use sync_types::patronus::{PatronusReviewRequirement, PatronusReviewerGroup, ReviewPathsSource};
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

/// A repository with a commit on the branch `baseline`
fn repo_with_baseline() -> TestRepo {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base");
  repo.git_executor().execute_command(&["branch", "baseline"], repo.path().to_str().unwrap()).unwrap();
  repo
}

fn read_inputs(repo: &TestRepo, baseline: &str, tips: &[&str]) -> Vec<Option<ReviewInput>> {
  let path = repo.path().to_str().unwrap();
  let tips: Vec<String> = tips.iter().map(|tip| repo.git_executor().execute_command(&["rev-parse", tip], path).unwrap()).collect();
  read_review_inputs(repo.git_executor(), path, &ObjectCache::disabled(), baseline, &tips).unwrap()
}

#[test]
fn reads_the_commits_of_the_branch_with_their_changed_paths() {
  let repo = repo_with_baseline();
  let path = repo.path().to_str().unwrap();
  let git = repo.git_executor();
  repo.create_commit_with_files("IJPL-1 first\n\nThe body", &[("a.txt", "a"), ("dir/b c.txt", "b")]);
  git.execute_command(&["rm", "--quiet", "a.txt"], path).unwrap();
  repo.create_commit("IJPL-1 second", "c.txt", "c");
  git.execute_command(&["mv", "dir/b c.txt", "moved.txt"], path).unwrap();
  git.execute_command(&["commit", "--quiet", "-m", "IJPL-1 rename"], path).unwrap();

  let [Some(input)] = <[Option<ReviewInput>; 1]>::try_from(read_inputs(&repo, "baseline", &["HEAD"])).ok().unwrap() else {
    panic!("the branch is short")
  };
  let commits = &input.commits;
  assert_eq!(
    commits.iter().map(|commit| commit.message.as_str()).collect::<Vec<_>>(),
    ["IJPL-1 rename", "IJPL-1 second", "IJPL-1 first\n\nThe body"]
  );
  // a rename is a delete and an add, as `--no-renames` lists it: the owners of both review it
  assert_eq!(commits[0].changed_paths, ["dir/b c.txt", "moved.txt"]);
  // a deleted path counts: its owners review the deletion
  assert_eq!(commits[1].changed_paths, ["a.txt", "c.txt"]);
  assert_eq!(commits[2].changed_paths, ["a.txt", "dir/b c.txt"]);
  assert_eq!(input.paths, ["a.txt", "c.txt", "dir/b c.txt", "moved.txt"]);
  assert!(!commits[0].is_merge_commit);
  assert!(commits[0].author_date.ends_with('Z'), "{}", commits[0].author_date);
  assert_ne!(commits[0].author_email, "");
}

/// Stacked branches share the commits of their parents; one log reads all of them, each branch gets the commits it reaches
#[test]
fn reads_several_branches_in_one_log() {
  let repo = repo_with_baseline();
  let path = repo.path().to_str().unwrap();
  let git = repo.git_executor();
  repo.create_commit("parent", "parent.txt", "p");
  git.execute_command(&["branch", "parent"], path).unwrap();
  repo.create_commit("child", "child.txt", "c");
  git.execute_command(&["branch", "child"], path).unwrap();
  git.execute_command(&["checkout", "--quiet", "-b", "side", "baseline"], path).unwrap();
  repo.create_commit("side", "side.txt", "s");

  let inputs = read_inputs(&repo, "baseline", &["parent", "child", "side", "baseline"]);
  let messages: Vec<Vec<&str>> = inputs
    .iter()
    .map(|input| input.as_ref().unwrap().commits.iter().map(|commit| commit.message.as_str()).collect())
    .collect();
  assert_eq!(messages, [vec!["parent"], vec!["child", "parent"], vec!["side"], vec![]]);
  assert_eq!(inputs[1].as_ref().unwrap().paths, ["child.txt", "parent.txt"]);
}

#[test]
fn does_not_read_a_branch_with_too_many_commits() {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base");
  let path = repo.path().to_str().unwrap();
  let git = repo.git_executor();
  let base = git.execute_command(&["rev-parse", "HEAD"], path).unwrap();
  // MAX_REVIEW_COMMITS + 1 commits without a work tree: one fast-import
  let mut stream = String::new();
  for index in 0..=MAX_REVIEW_COMMITS {
    let from = if index == 0 { format!("from {base}\n") } else { String::new() };
    write!(
      stream,
      "commit refs/heads/long\ncommitter Me <me@example.com> 1700000000 +0000\ndata 1\nc\n{from}M 644 inline f.txt\ndata {}\n{index}\n",
      index.to_string().len()
    )
    .unwrap();
  }
  git.execute_command_with_input(&["fast-import", "--quiet"], path, &stream).unwrap();
  let inputs = read_inputs(&repo, &base, &["refs/heads/long", "refs/heads/long~1"]);
  assert!(inputs[0].is_none());
  assert_eq!(inputs[1].as_ref().map(|input| input.commits.len()), Some(MAX_REVIEW_COMMITS));
  assert!(read_inputs(&repo, &base, &["refs/heads/long"])[0].is_none());
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

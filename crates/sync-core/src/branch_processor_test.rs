use crate::branch_processor::{BranchJob, BranchSyncContext, BuildOutcome, BuiltBranch, build_branch, finish_branch, process_single_branch};
use crate::commit_owners::CommitOwners;
use crate::sync::{SyncOptions, sync_branches};
use domain_types::commit::Commit;
use domain_types::{BranchError, BranchSyncStatus, CommitSyncStatus, SyncEvent};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::get_commit_list;
use git_ops::notes::write_commit_notes;
use git_ops::tree_id_cache::TreeIdCache;
use git_ops::word_resolve::ResolveMode;
use history_rewrite::commit_grouper::CommitGrouper;
use history_rewrite::move_commits::{MoveCommitsParams, move_commits_to_branch_core};
use history_rewrite::regroup::RegroupOptions;
use history_rewrite::reword::{CommitReword, RewordText, reword_core};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::collections::HashMap;
use test_utils::git_test_utils::TestRepo;
use test_utils::sync_fixtures::TestReporter;

fn repo_path(test_repo: &TestRepo) -> &str {
  test_repo.path().to_str().unwrap()
}

/// Syncs with the `test` branch prefix and returns the statuses of the copied commits, by branch, oldest first
async fn sync(test_repo: &TestRepo) -> BTreeMap<String, Vec<CommitSyncStatus>> {
  sync_with(test_repo, SyncOptions::default()).await
}

/// `sync` with `options`
async fn sync_with(test_repo: &TestRepo, options: SyncOptions) -> BTreeMap<String, Vec<CommitSyncStatus>> {
  let progress = TestReporter::new();
  sync_branches(test_repo.git_executor(), repo_path(test_repo), "test", progress.clone(), options)
    .await
    .unwrap();
  let mut statuses: BTreeMap<String, Vec<CommitSyncStatus>> = BTreeMap::new();
  for event in progress.get_events() {
    if let SyncEvent::CommitSynced { branch_name, status, .. } = event {
      statuses.entry(branch_name).or_default().push(status);
    }
  }
  statuses
}

/// The commits of the virtual branch, oldest first
fn virtual_commits(test_repo: &TestRepo, branch: &str) -> Vec<String> {
  test_repo
    .log(&["--reverse", "--format=%H", &format!("master..test/virtual/{branch}")])
    .unwrap()
    .lines()
    .map(str::to_string)
    .collect()
}

/// The commits of a branch as the sync groups them, oldest first
fn grouped_commits(test_repo: &TestRepo, branch: &str) -> Vec<Commit> {
  let mut grouper = CommitGrouper::new();
  for commit in get_commit_list(test_repo.git_executor(), repo_path(test_repo), "master").unwrap() {
    grouper.add_commit(commit);
  }
  grouper.finish().branches.swap_remove(branch).unwrap()
}

/// The context of a sync of the `test` branches against `master`, with the virtual branches as they are now
fn sync_context(test_repo: &TestRepo, progress: TestReporter) -> BranchSyncContext<TestReporter> {
  BranchSyncContext {
    repository_path: repo_path(test_repo).to_string(),
    branch_prefix: "test".to_string(),
    baseline_branch: "master".to_string(),
    total_branches: 1,
    progress,
    git_executor: GitCommandExecutor::new(),
    tree_id_cache: TreeIdCache::new(),
    remote_statuses: None,
    conflict_chains: None,
    conflict_tips: HashMap::new(),
    existing_commits: None,
    existing_tips: existing_tips(test_repo),
    ancestor_keys: HashMap::new(),
    verify_mapping_notes: false,
    owners: CommitOwners::default(),
    resolve: ResolveMode::On,
  }
}

/// The job that builds `branch` on `base`; `on_parent`: `base` is the new head of its parent
fn job(test_repo: &TestRepo, branch: &str, base: &str, on_parent: bool) -> BranchJob {
  BranchJob {
    branch_name: branch.to_string(),
    commits: grouped_commits(test_repo, branch),
    my_email: None,
    branch_idx: 0,
    base_commit: base.to_string(),
    on_parent,
  }
}

/// Rebuilds the virtual branch `branch` on `base`, as a sync does
fn rebuild_branch(test_repo: &TestRepo, branch: &str, base: &str) {
  let context = sync_context(test_repo, TestReporter::new());
  let notes = process_single_branch(&context, job(test_repo, branch, base, false)).unwrap();
  write_commit_notes(test_repo.git_executor(), repo_path(test_repo), &notes).unwrap();
}

/// Builds `branch` without moving its ref, as the first step of a sync does
fn build(context: &BranchSyncContext<TestReporter>, job: BranchJob) -> BuiltBranch {
  match build_branch(context, job).unwrap() {
    BuildOutcome::Built(built) => *built,
    BuildOutcome::Failed(reason, _) => panic!("the build failed: {reason:?}"),
  }
}

/// The unpushed commits of the remote status `progress` got for `branch`
fn unpushed_of(progress: &TestReporter, branch: &str) -> Vec<String> {
  progress
    .get_events()
    .into_iter()
    .find_map(|event| match event {
      SyncEvent::RemoteStatusUpdate(update) if update.branch_name == branch => Some(update.unpushed_commits),
      _ => None,
    })
    .unwrap_or_else(|| panic!("{branch} has a remote status"))
}

/// Builds auth on `base` and login on auth's new head, finishes login while auth isn't finished, and returns login's
/// unpushed commits with auth, built
fn finish_login_first(test_repo: &TestRepo, base: &str) -> (Vec<String>, BuiltBranch) {
  let progress = TestReporter::new();
  let context = sync_context(test_repo, progress.clone());
  let auth = build(&context, job(test_repo, "auth", base, false));
  let login = build(&context, job(test_repo, "login", &auth.head, true));
  finish_branch(&context, login).unwrap();
  (unpushed_of(&progress, "login"), auth)
}

/// A child can finish before its parent: the sync builds it on the parent's new head in memory, and the parent's
/// `finish_branch` moves the parent's ref later. The remote status of the child excludes the parent's commits by the head
/// it was built on, so it is right while the parent's ref is absent (a new stack) or at the old head (an existing one)
#[test]
fn a_child_finished_before_its_parent_excludes_the_parent_by_the_head_it_was_built_on() {
  let test_repo = TestRepo::new();
  let initial = test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_commit("(auth) Add auth", "auth.rs", "// auth");
  test_repo.create_commit("(login) Add form", "login.rs", "// login");

  // a new stack: auth has no ref yet
  let (unpushed, auth) = finish_login_first(&test_repo, &initial);
  assert!(!test_repo.branch_exists("test/virtual/auth"));
  assert_eq!(unpushed, [test_repo.rev_parse("test/virtual/login").unwrap()]);
  finish_branch(&sync_context(&test_repo, TestReporter::new()), auth).unwrap();

  // auth gets a commit: its ref is at the old head while login finishes, and the session commit is still auth's
  test_repo.create_commit("(auth) Add session", "session.rs", "// session");
  let (unpushed, auth) = finish_login_first(&test_repo, &initial);
  assert_ne!(test_repo.rev_parse("test/virtual/auth").unwrap(), auth.head, "the parent's ref is stale");
  assert_eq!(unpushed, [test_repo.rev_parse("test/virtual/login").unwrap()]);
}

/// The last status `progress` got for `branch`, with its error message
fn final_status(progress: &TestReporter, branch: &str) -> (BranchSyncStatus, Option<String>) {
  progress
    .get_events()
    .into_iter()
    .rev()
    .find_map(|event| match event {
      SyncEvent::BranchStatusUpdate { branch_name, status, error } if branch_name == branch => Some((
        status,
        error.map(|error| match error {
          BranchError::Generic(message) => message,
          BranchError::MergeConflict(info) => format!("conflict in {}", info.commit.hash),
        }),
      )),
      _ => None,
    })
    .unwrap_or_else(|| panic!("{branch} has a status"))
}

/// The sync moves a virtual branch only from the tip it listed (compare-and-swap): a branch another process moved since
/// then keeps that tip and reports an error, a branch another process moved to the same head (the copies are
/// deterministic) is written, and the other branches are written as usual
#[test]
fn a_branch_moved_since_the_listing_is_not_overwritten() {
  let test_repo = TestRepo::new();
  let initial = test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_commit("(alpha) Alpha", "alpha.rs", "// alpha");
  test_repo.create_commit("(beta) Beta", "beta.rs", "// beta");
  test_repo.create_commit("(gamma) Gamma", "gamma.rs", "// gamma");
  for branch in ["alpha", "beta", "gamma"] {
    rebuild_branch(&test_repo, branch, &initial);
  }
  test_repo.create_commit("(alpha) Alpha 2", "alpha-2.rs", "// alpha 2");
  test_repo.create_commit("(beta) Beta 2", "beta-2.rs", "// beta 2");
  test_repo.create_commit("(gamma) Gamma 2", "gamma-2.rs", "// gamma 2");

  let progress = TestReporter::new();
  let context = sync_context(&test_repo, progress.clone());
  let [alpha, beta, gamma] = ["alpha", "beta", "gamma"].map(|branch| build(&context, job(&test_repo, branch, &initial, false)));
  let (beta_head, gamma_head) = (beta.head.clone(), gamma.head.clone());
  // another process: alpha moved elsewhere, beta to the head this sync built
  git(&test_repo, &["update-ref", "refs/heads/test/virtual/alpha", &initial]);
  git(&test_repo, &["update-ref", "refs/heads/test/virtual/beta", &beta_head]);
  let moved = [alpha, beta, gamma].map(|built| finish_branch(&context, built).unwrap().moved_elsewhere);
  // the tip of alpha is no build of this sync: the report says it did not build alpha
  assert_eq!(moved, [true, false, false]);

  assert_eq!(test_repo.rev_parse("test/virtual/alpha").unwrap(), initial, "the other process's tip stays");
  let (status, error) = final_status(&progress, "alpha");
  assert_eq!(status, BranchSyncStatus::Error);
  let error = error.unwrap();
  assert!(error.contains("test/virtual/alpha") && error.contains("another process"), "{error}");
  assert_eq!(final_status(&progress, "beta"), (BranchSyncStatus::Updated, None));
  assert_eq!(test_repo.rev_parse("test/virtual/beta").unwrap(), beta_head);
  assert_eq!(final_status(&progress, "gamma"), (BranchSyncStatus::Updated, None));
  assert_eq!(test_repo.rev_parse("test/virtual/gamma").unwrap(), gamma_head);
}

/// A new branch is created only while no ref of its name exists
#[test]
fn a_new_branch_created_meanwhile_is_not_overwritten() {
  let test_repo = TestRepo::new();
  let initial = test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_commit("(alpha) Alpha", "alpha.rs", "// alpha");

  let progress = TestReporter::new();
  let context = sync_context(&test_repo, progress.clone());
  let alpha = build(&context, job(&test_repo, "alpha", &initial, false));
  git(&test_repo, &["update-ref", "refs/heads/test/virtual/alpha", &initial]);
  finish_branch(&context, alpha).unwrap();

  assert_eq!(test_repo.rev_parse("test/virtual/alpha").unwrap(), initial);
  assert_eq!(final_status(&progress, "alpha").0, BranchSyncStatus::Error);
}

/// The virtual branches and their tips, as a sync lists them
fn existing_tips(test_repo: &TestRepo) -> HashMap<String, String> {
  let lines = test_repo
    .git_executor()
    .execute_command_lines(
      &["for-each-ref", "--format=%(refname:lstrip=2) %(objectname)", "refs/heads/test/virtual/"],
      repo_path(test_repo),
    )
    .unwrap();
  lines
    .iter()
    .filter_map(|line| line.split_once(' '))
    .map(|(name, tip)| (name.to_string(), tip.to_string()))
    .collect()
}

#[test]
fn a_new_base_recopies_the_commits_whose_mapped_commits_still_exist() {
  let test_repo = TestRepo::new();
  let initial = test_repo.create_commit("Initial commit", "README.md", "# Test");
  let base = test_repo.create_commit("Update docs", "docs.md", "docs");
  test_repo.create_commit("(auth) Add login", "login.rs", "// login");
  test_repo.create_commit("(auth) Add logout", "logout.rs", "// logout");

  rebuild_branch(&test_repo, "auth", &initial);
  let on_initial = virtual_commits(&test_repo, "auth");
  assert_eq!(test_repo.rev_parse("test/virtual/auth~2").unwrap(), initial);

  // the notes still map both commits to the copies on `initial`, which are still in the branch
  rebuild_branch(&test_repo, "auth", &base);

  assert_eq!(test_repo.rev_parse("test/virtual/auth~2").unwrap(), base);
  let on_base = virtual_commits(&test_repo, "auth");
  assert_eq!(on_base.len(), 2);
  assert!(on_base.iter().all(|id| !on_initial.contains(id)), "{on_base:?} reuses a commit of {on_initial:?}");
}

#[tokio::test]
async fn a_rebuild_without_notes_writes_the_same_commits() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_commit("(auth) Add login", "login.rs", "// login");
  test_repo.create_commit("(cache) Add cache", "cache.rs", "// cache");
  test_repo.create_commit("(auth) Add logout\n\nClears the session.", "logout.rs", "// logout");

  sync(&test_repo).await;
  let auth = virtual_commits(&test_repo, "auth");
  let cache = virtual_commits(&test_repo, "cache");

  // a later second, so a committer date of "now" would change the ids
  std::thread::sleep(std::time::Duration::from_millis(1100));
  test_repo
    .git_executor()
    .execute_command(&["update-ref", "-d", "refs/notes/commits"], repo_path(&test_repo))
    .unwrap();
  let statuses = sync(&test_repo).await;
  assert_eq!(statuses["auth"], [CommitSyncStatus::Created, CommitSyncStatus::Created]);
  assert_eq!(virtual_commits(&test_repo, "auth"), auth);
  assert_eq!(virtual_commits(&test_repo, "cache"), cache);

  // also from scratch
  test_repo.delete_branch("test/virtual/auth").unwrap();
  test_repo.delete_branch("test/virtual/cache").unwrap();
  test_repo
    .git_executor()
    .execute_command(&["update-ref", "-d", "refs/notes/commits"], repo_path(&test_repo))
    .unwrap();
  sync(&test_repo).await;
  assert_eq!(virtual_commits(&test_repo, "auth"), auth);
  assert_eq!(virtual_commits(&test_repo, "cache"), cache);
}

#[tokio::test]
async fn assigning_an_older_commit_reuses_every_virtual_commit() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  let unassigned = test_repo.create_commit("Fix typo", "typo.txt", "typo");
  test_repo.create_commit("(auth) Add login", "login.rs", "// login");
  test_repo.create_commit("(cache) Add cache", "cache.rs", "// cache");
  test_repo.create_commit("(auth) Add logout", "logout.rs", "// logout");

  sync(&test_repo).await;
  let auth = virtual_commits(&test_repo, "auth");
  let cache = virtual_commits(&test_repo, "cache");

  // recreates every commit after it
  let params = MoveCommitsParams::new(repo_path(&test_repo), vec![unassigned], Some("docs"));
  move_commits_to_branch_core(test_repo.git_executor(), &params).unwrap();

  let statuses = sync(&test_repo).await;
  assert_eq!(statuses["auth"], [CommitSyncStatus::Unchanged, CommitSyncStatus::Unchanged]);
  assert_eq!(statuses["cache"], [CommitSyncStatus::Unchanged]);
  assert_eq!(statuses["docs"], [CommitSyncStatus::Created]);
  assert_eq!(virtual_commits(&test_repo, "auth"), auth);
  assert_eq!(virtual_commits(&test_repo, "cache"), cache);
}

#[tokio::test]
async fn moving_a_commit_to_another_branch_copies_it_there() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_commit("(auth) Add login", "login.rs", "// login");
  test_repo.create_commit("(cache) Add cache", "cache.rs", "// cache");
  let logout = test_repo.create_commit("(auth) Add logout", "logout.rs", "// logout");

  sync(&test_repo).await;
  let auth = virtual_commits(&test_repo, "auth");
  let cache = virtual_commits(&test_repo, "cache");

  // only the prefix changes: the note moves with the commit, but its virtual commit is in another branch
  let params = MoveCommitsParams::new(repo_path(&test_repo), vec![logout], Some("cache"));
  move_commits_to_branch_core(test_repo.git_executor(), &params).unwrap();

  let statuses = sync(&test_repo).await;
  assert_eq!(statuses["auth"], [CommitSyncStatus::Unchanged]);
  assert_eq!(statuses["cache"], [CommitSyncStatus::Unchanged, CommitSyncStatus::Created]);
  assert_eq!(virtual_commits(&test_repo, "auth"), auth[..1]);
  let moved = virtual_commits(&test_repo, "cache");
  assert_eq!(moved[0], cache[0]);
  assert_eq!(test_repo.log(&["-1", "--format=%s", &moved[1]]).unwrap(), "Add logout");
  assert!(test_repo.get_files_in_commit(&moved[1]).unwrap().contains(&"logout.rs".to_string()));
}

#[tokio::test]
async fn a_reworded_body_recopies_the_commit() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  let login = test_repo.create_commit("(auth) Add login", "login.rs", "// login");
  test_repo.create_commit("(cache) Add cache", "cache.rs", "// cache");

  sync(&test_repo).await;
  let cache = virtual_commits(&test_repo, "cache");

  let reword = CommitReword {
    commit: login,
    text: RewordText::Message("(auth) Add login\n\nWith a remember-me box.".to_string()),
  };
  reword_core(test_repo.git_executor(), repo_path(&test_repo), &[reword], &RegroupOptions::default()).unwrap();

  let statuses = sync(&test_repo).await;
  assert_eq!(statuses["auth"], [CommitSyncStatus::Created]);
  assert_eq!(statuses["cache"], [CommitSyncStatus::Unchanged]);
  assert_eq!(virtual_commits(&test_repo, "cache"), cache);
  assert_eq!(
    test_repo.log(&["-1", "--format=%B", "test/virtual/auth"]).unwrap().trim_end(),
    "Add login\n\nWith a remember-me box."
  );
}

#[tokio::test]
async fn a_commit_moved_into_the_branch_of_an_issue_is_copied_with_the_key() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_commit("(IJPL-1) docs plugins: audit the attributes\n\nThe rules of the audit.", "audit.md", "# Audit");
  test_repo.create_commit("[plugins] IJPL-1 Lift the attribute", "plugin.rs", "// plugin");

  sync(&test_repo).await;

  // the copies are what lands: the one of the commit without a key of its own names the issue of its branch
  assert_eq!(
    test_repo
      .log(&["--reverse", "--format=%B%x00", "master..test/virtual/IJPL-1"])
      .unwrap()
      .split('\0')
      .map(str::trim)
      .filter(|it| !it.is_empty())
      .collect::<Vec<_>>(),
    [
      "IJPL-1 docs plugins: audit the attributes\n\nThe rules of the audit.",
      "[plugins] IJPL-1 Lift the attribute"
    ]
  );
}

#[tokio::test]
async fn a_copy_keeps_the_timezones_of_the_original() {
  let test_repo = TestRepo::new();
  // older than the original, so the copy's committer date is the original's
  test_repo.create_commit_with_timestamp("Initial commit", "README.md", "# Test", Some(1_600_000_000));
  std::fs::write(test_repo.path().join("login.rs"), "// login").unwrap();
  test_repo.git_executor().execute_command(&["add", "login.rs"], repo_path(&test_repo)).unwrap();
  let env = [("GIT_AUTHOR_DATE", "1700000000 +0530"), ("GIT_COMMITTER_DATE", "1700000100 -0700")];
  test_repo
    .git_executor()
    .cmd(&["commit", "-m", "(auth) Add login"], repo_path(&test_repo))
    .env(&env)
    .text()
    .unwrap();

  sync(&test_repo).await;

  let dates = test_repo.log(&["-1", "--format=%ad|%cd", "--date=raw", "test/virtual/auth"]).unwrap();
  assert_eq!(dates.trim(), "1700000000 +0530|1700000100 -0700");
}

#[tokio::test]
async fn a_sync_writes_the_notes_of_all_branches_in_one_notes_commit() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  for branch in ["auth", "cache", "docs"] {
    test_repo.create_commit(&format!("({branch}) Add {branch}"), &format!("{branch}.rs"), branch);
  }

  sync(&test_repo).await;

  assert_eq!(test_repo.log(&["--format=%H", "refs/notes/commits"]).unwrap().lines().count(), 1);
  assert_eq!(test_repo.list_notes_with_ref("refs/notes/commits").unwrap().len(), 3);
}

fn git(test_repo: &TestRepo, args: &[&str]) -> String {
  test_repo.git_executor().execute_command(args, repo_path(test_repo)).unwrap()
}

fn git_with_env(test_repo: &TestRepo, args: &[&str], env: &[(&str, &str)]) -> String {
  test_repo.git_executor().cmd(args, repo_path(test_repo)).env(env).text().unwrap()
}

/// `commit` again with `tree` on `parent`, with its message and author, as plumbing outside Branch Deck writes it
fn recommit(test_repo: &TestRepo, commit: &str, tree: &str, parent: &str) -> String {
  let fields = git(test_repo, &["log", "-1", "--format=%an%x1f%ae%x1f%ad", "--date=raw", commit]);
  let fields: Vec<&str> = fields.split('\u{1f}').collect();
  let message = git(test_repo, &["log", "-1", "--format=%B", commit]);
  let env = [("GIT_AUTHOR_NAME", fields[0]), ("GIT_AUTHOR_EMAIL", fields[1]), ("GIT_AUTHOR_DATE", fields[2])];
  git_with_env(test_repo, &["commit-tree", tree, "-p", parent, "-m", &message], &env)
}

/// The tree of HEAD with `file` set to `content`, written through a temporary index
fn tree_with(test_repo: &TestRepo, base: &str, file: &str, content: &str) -> String {
  let index = test_repo.path().join(".git").join("tree-with.index");
  let index = index.to_str().unwrap();
  let env = [("GIT_INDEX_FILE", index)];
  git_with_env(test_repo, &["read-tree", base], &env);
  let blob = test_repo
    .git_executor()
    .cmd(&["hash-object", "-w", "--stdin"], repo_path(test_repo))
    .stdin(content.as_bytes())
    .text()
    .unwrap();
  git_with_env(test_repo, &["update-index", "--add", "--cacheinfo", &format!("100644,{blob},{file}")], &env);
  let tree = git_with_env(test_repo, &["write-tree"], &env);
  std::fs::remove_file(index).unwrap();
  tree
}

/// A hunk moved into a commit with plumbing, the note copied along: the note names the old copy, which lacks the hunk.
/// The sync sees that the commit's trees are not the ones the note was copied from, and copies it again
#[tokio::test]
async fn a_note_copied_to_a_commit_with_another_tree_is_stale() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_commit("(auth) Add login", "login.rs", "// login");
  let logout = test_repo.create_commit("(auth) Add logout", "logout.rs", "// logout");
  sync(&test_repo).await;
  let before = virtual_commits(&test_repo, "auth");

  let tree = tree_with(&test_repo, &logout, "logout.rs", "// logout\n// and the session\n");
  let changed = recommit(&test_repo, &logout, &tree, &format!("{logout}~1"));
  git(&test_repo, &["notes", "copy", &logout, &changed]);
  git(&test_repo, &["update-ref", "refs/heads/master", &changed]);

  let statuses = sync(&test_repo).await;
  assert_eq!(statuses["auth"], [CommitSyncStatus::Unchanged, CommitSyncStatus::Created]);
  let after = virtual_commits(&test_repo, "auth");
  assert_eq!(after[0], before[0]);
  assert_ne!(after[1], before[1]);
  assert_eq!(git(&test_repo, &["show", "test/virtual/auth:logout.rs"]), "// logout\n// and the session");
  // the note of the new commit now names the new copy and its source
  let note = test_repo.show_note(&changed).unwrap();
  assert_eq!(git_ops::notes::mapped_commit_id(&note), Some(after[1].as_str()));
  assert_eq!(git_ops::notes::mapped_source(&note).map(|source| source.tree), Some(tree));
}

/// `git commit --amend` with `notes.rewriteRef` carries the note to a commit with another message: the copy's message
/// is stale, though the trees are the same
#[tokio::test]
async fn an_amend_with_notes_rewrite_ref_rebuilds_the_copy() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_commit("(auth) Add login", "login.rs", "// login");
  sync(&test_repo).await;

  test_repo.set_config("notes.rewriteRef", "refs/notes/commits").unwrap();
  git(&test_repo, &["commit", "--amend", "-m", "(auth) Add login\n\nWith a remember-me box."]);
  assert!(test_repo.show_note("HEAD").is_ok(), "the amend copied the note");

  let statuses = sync(&test_repo).await;
  assert_eq!(statuses["auth"], [CommitSyncStatus::Created]);
  assert_eq!(git(&test_repo, &["log", "-1", "--format=%B", "test/virtual/auth"]), "Add login\n\nWith a remember-me box.");
}

/// A note written before the source line, to a copy written before the copies were deterministic (another committer
/// date): the sync checks the copy against its tree and keeps it, so the branch doesn't change, and records the source
#[tokio::test]
async fn a_note_without_a_source_keeps_an_older_copy_and_gets_one() {
  let test_repo = TestRepo::new();
  let initial = test_repo.create_commit("Initial commit", "README.md", "# Test");
  let login = test_repo.create_commit("(auth) Add login", "login.rs", "// login");
  sync(&test_repo).await;

  // the older copy: another committer date, a note with the mapping line only
  let copy = git(&test_repo, &["rev-parse", "test/virtual/auth"]);
  let tree = git(&test_repo, &["rev-parse", &format!("{copy}^{{tree}}")]);
  let fields = git(&test_repo, &["log", "-1", "--format=%an%x1f%ae%x1f%ad", "--date=raw", &copy]);
  let fields: Vec<&str> = fields.split('\u{1f}').collect();
  let env = [
    ("GIT_AUTHOR_NAME", fields[0]),
    ("GIT_AUTHOR_EMAIL", fields[1]),
    ("GIT_AUTHOR_DATE", fields[2]),
    ("GIT_COMMITTER_DATE", "1500000000 +0000"),
  ];
  let older = git_with_env(&test_repo, &["commit-tree", &tree, "-p", &initial, "-m", "Add login"], &env);
  assert_ne!(older, copy);
  git(&test_repo, &["update-ref", "refs/heads/test/virtual/auth", &older]);
  test_repo.add_note(&login, &format!("{}{older}", git_ops::notes::PREFIX)).unwrap();

  let statuses = sync(&test_repo).await;
  assert_eq!(statuses["auth"], [CommitSyncStatus::Unchanged]);
  assert_eq!(git(&test_repo, &["rev-parse", "test/virtual/auth"]), older);
  let note = test_repo.show_note(&login).unwrap();
  assert_eq!(git_ops::notes::mapped_commit_id(&note), Some(older.as_str()));
  assert!(git_ops::notes::mapped_source(&note).is_some(), "{note}");

  // and the next sync trusts the note without checking the tree again
  let statuses = sync(&test_repo).await;
  assert_eq!(statuses["auth"], [CommitSyncStatus::Unchanged]);
  assert_eq!(git(&test_repo, &["rev-parse", "test/virtual/auth"]), older);
}

/// A note whose source line looks right but whose copy has another tree: the sync trusts it, a sync that verifies the
/// notes (`bd --no-cache`) copies the commit again
#[tokio::test]
async fn a_verifying_sync_checks_the_tree_of_each_copy() {
  let test_repo = TestRepo::new();
  let initial = test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_commit("(auth) Add login", "login.rs", "// login");
  sync(&test_repo).await;

  let copy = git(&test_repo, &["rev-parse", "test/virtual/auth"]);
  let wrong_tree = tree_with(&test_repo, &copy, "login.rs", "// not the login\n");
  let wrong = recommit(&test_repo, &copy, &wrong_tree, &initial);
  git(&test_repo, &["update-ref", "refs/heads/test/virtual/auth", &wrong]);
  let note = test_repo.show_note("HEAD").unwrap();
  test_repo.add_note("HEAD", &note.replace(&copy, &wrong)).unwrap();

  assert_eq!(sync(&test_repo).await["auth"], [CommitSyncStatus::Unchanged]);
  assert_eq!(git(&test_repo, &["rev-parse", "test/virtual/auth"]), wrong);

  let verifying = SyncOptions {
    verify_mapping_notes: true,
    ..SyncOptions::default()
  };
  assert_eq!(sync_with(&test_repo, verifying).await["auth"], [CommitSyncStatus::Created]);
  assert_eq!(git(&test_repo, &["rev-parse", "test/virtual/auth"]), copy);
}

use bd_cli::fixture_test::assert_covered;
use bd_cli::fixture_test::{PREFIX, assert_error, branch, commit_all, git, next_steps, run_in, run_json, write};
use bd_cli::remote_fixture_test::{LANDED_SUBJECTS, Space, auth_remote_moved_back, land_auth_by_content, push_from_elsewhere_to, remote_tip, repo_with_remote, their_change};
use git_ops::revs::short_id;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use sync_core::push::{LeasedBranch, LeasedPush, PreparedPush, PushTarget};
use sync_core::push_lease::{PushOptions, prepare_push_expecting};
use tempfile::TempDir;
use test_utils::git_test_utils::{TestRepo, disable_background_maintenance, pre_receive_hook, refuse_every_push};

const AUTH: &str = "test-user/virtual/auth";

/// `bd push auth` has nothing to push, and its lease is the remote head its sync saw: the pushed commit
fn assert_pushes_up_to_date_on_the_seen_head(repo: &TestRepo) {
  let (exit_code, output) = run_in(repo, &["push", "auth"]);
  assert_pushed(&output, exit_code, "up_to_date", &git(repo, &["rev-parse", &format!("refs/heads/{AUTH}")]));
}

/// `bd push` pushed `auth` with `status`, leased on the remote head `lease` it saw
fn assert_pushed(output: &Value, exit_code: i32, status: &str, lease: &str) {
  assert_eq!(exit_code, 0, "{output}");
  let auth = branch(output, "auth");
  assert_eq!(
    (&auth["state"], &auth["lease"], &auth["lease_source"]),
    (&json!(status), &json!(short_id(lease)), &json!("expected"))
  );
}

#[test_log::test]
fn pushes_with_a_lease_and_prints_the_pull_request() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  run_in(&repo, &["describe", "auth", "--title", "Add login", "--body", "The login form."]);

  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let tip = git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]);
  assert_eq!(remote_tip(&remote, AUTH), tip);
  assert_eq!(output["remote"], json!("origin"));
  assert_eq!(
    branch(&output, "auth"),
    &json!({
      "name": "auth",
      "remote_branch": AUTH,
      "commit": short_id(&tip),
      "state": "new",
      "lease": null,
      "lease_source": "absent",
      "pull_request": {
        "base": "master",
        "head": AUTH,
        "title": "Add login",
        "body": "The login form.",
        "command": format!("gh pr create --base master --head {AUTH} --assignee @me --title 'Add login' --body-file -"),
      },
    })
  );

  let (_, schema) = run_in(&repo, &["schema", "--all"]);
  assert_covered(&schema["outputs"]["push"], &schema["outputs"]["push"], &output, "push");

  // nothing new: the lease is the remote head the sync saw
  assert_pushes_up_to_date_on_the_seen_head(&repo);
}

/// A GitHub repository that `origin` doesn't push to (`branchdeck.githubRepository`, a mirror) is named with `--repo`:
/// `gh` finds it among no remote. The sync read the setting; the push URL of `origin` tells the mirror apart
#[test_log::test]
fn the_pull_request_command_names_a_github_mirror() {
  let (repo, _remote) = repo_with_remote(&["(auth) Add login"]);
  repo.set_config("branchdeck.githubRepository", "acme/widgets").unwrap();
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    branch(&output, "auth")["pull_request"]["command"],
    json!(format!(
      "gh pr create --repo acme/widgets --base master --head {AUTH} --assignee @me --title 'Add login' --body-file -"
    ))
  );
}

/// Every pull request goes into the baseline: a stacked branch's holds the commits of the branches it is built on, so
/// pushed alone it carries them
#[test_log::test]
fn parents_go_first_and_a_child_alone_carries_its_parents_commits() {
  let (repo, remote) = repo_with_remote(&["(auth) Add auth", "(auth/login) Add login"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let (exit_code, output) = run_in(&repo, &["push", "--alone"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));

  let (exit_code, output) = run_in(&repo, &["push", "login", "--alone"]);
  assert_eq!(exit_code, 0, "{output}");
  let names: Vec<&Value> = output["branches"].as_array().unwrap().iter().map(|branch| &branch["name"]).collect();
  assert_eq!(names, [&json!("login")]);
  assert_eq!(remote_tip(&remote, AUTH), "");
  // the remote branch of login has the commit of auth under its own
  let login = remote_tip(&remote, "test-user/virtual/login");
  assert_eq!(git(&repo, &["log", "--format=%s", &format!("origin/master..{login}")]), "Add login\nAdd auth");
  // titled by its own commit, the text names the branch it is built on
  let pull_request = &branch(&output, "login")["pull_request"];
  assert_eq!(
    pull_request["command"],
    json!("gh pr create --base master --head test-user/virtual/login --assignee @me --title 'Add login' --body-file -")
  );
  assert_eq!(
    (&pull_request["title"], &pull_request["body"]),
    (&json!("Add login"), &json!("Built on auth: its commits are in this pull request too.\n\n- Add auth"))
  );

  // the branch it is built on goes along, first, and its pull request goes into the baseline too
  let (exit_code, output) = run_in(&repo, &["push", "login"]);
  assert_eq!(exit_code, 0, "{output}");
  let statuses: Vec<(&Value, &Value)> = output["branches"].as_array().unwrap().iter().map(|branch| (&branch["name"], &branch["state"])).collect();
  assert_eq!(statuses, [(&json!("auth"), &json!("new")), (&json!("login"), &json!("up_to_date"))]);
  assert_eq!(branch(&output, "auth")["pull_request"]["base"], json!("master"));
}

#[test_log::test]
fn a_parent_goes_along_only_with_commits_for_the_remote_and_brings_no_children() {
  let (repo, _remote) = repo_with_remote(&["(auth) Add auth", "(auth/session) Add session", "(auth/login) Add login"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");

  // auth is on the remote as it is: login goes alone; session, a child of auth, is not taken along by --with-children of login
  let (exit_code, output) = run_in(&repo, &["push", "login", "--with-children"]);
  assert_eq!(exit_code, 0, "{output}");
  let names: Vec<&Value> = output["branches"].as_array().unwrap().iter().map(|branch| &branch["name"]).collect();
  assert_eq!(names, [&json!("login")]);
}

#[test_log::test]
fn with_children_pushes_the_branches_stacked_on_the_named_ones() {
  let (repo, remote) = repo_with_remote(&["(auth) Add auth", "(auth/login) Add login", "(login/oauth) Add oauth", "(docs) Add docs"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();

  let (exit_code, output) = run_in(&repo, &["push", "--with-children"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--with-children", "--expect-remote", "HEAD"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));

  // the grandchild too, parents first; docs is not stacked on auth
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--with-children"]);
  assert_eq!(exit_code, 0, "{output}");
  let names: Vec<&Value> = output["branches"].as_array().unwrap().iter().map(|branch| &branch["name"]).collect();
  assert_eq!(names, [&json!("auth"), &json!("login"), &json!("oauth")]);
  assert_eq!(remote_tip(&remote, "test-user/virtual/docs"), "");
  // every pull request goes into the baseline, with the commits of the branches it is built on
  for name in ["auth", "login", "oauth"] {
    assert_eq!(branch(&output, name)["pull_request"]["base"], json!("master"), "{name}");
  }

  // a new commit of login rebuilds oauth on it: the push replaces oauth's remote copy
  write(&repo, "form.txt", "form\n");
  commit_all(&repo, "(auth/login) Add the login form", 1_700_000_900);
  let (exit_code, output) = run_in(&repo, &["push", "login", "--with-children"]);
  assert_eq!(exit_code, 0, "{output}");
  let statuses: Vec<(&Value, &Value)> = output["branches"].as_array().unwrap().iter().map(|branch| (&branch["name"], &branch["state"])).collect();
  assert_eq!(statuses, [(&json!("login"), &json!("updated")), (&json!("oauth"), &json!("forced"))]);
}

/// The root of a stack that forks takes its stack branch (`{prefix}/stack/<root>`) along with its children, in the same
/// atomic push, leased, as the app's Push with Children does: once it is on the remote (or its pull request is open,
/// `safe_push_test`), and only when this sync built it. Before that it is no branch to publish by itself; one on the
/// remote as it is stays out, and so does one that the remote pushed to doesn't compare with
#[test_log::test]
fn with_children_takes_the_stack_branch_along_once_it_is_on_the_remote() {
  let (repo, remote) = repo_with_remote(&["(auth) Add auth", "(auth/login) Add login", "(auth/session) Add session"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let stack = format!("{PREFIX}/stack/auth");

  // a child's push takes no stack branch along: only the root names the whole stack
  let (exit_code, output) = run_in(&repo, &["push", "login", "--with-children"]);
  assert_eq!((exit_code, output.get("stacks")), (0, None), "{output}");

  // not on the remote, no pull request: the branches go, the stack branch stays local, nothing to warn about
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--with-children"]);
  assert_eq!(exit_code, 0, "{output}");
  let names: Vec<&Value> = output["branches"].as_array().unwrap().iter().map(|branch| &branch["name"]).collect();
  assert_eq!(names, [&json!("auth"), &json!("login"), &json!("session")]);
  assert_eq!((output.get("stacks"), output.get("stack_warnings")), (None, None), "{output}");
  assert_eq!(remote_tip(&remote, &stack), "");

  // its pull request (or Safe Push) put it on the remote; a new commit of login rebuilds it, and it goes with login
  git(&repo, &["push", "-q", "origin", &format!("refs/heads/{stack}:refs/heads/{stack}")]);
  let pushed_before = remote_tip(&remote, &stack);
  write(&repo, "login-more.txt", "more\n");
  commit_all(&repo, "(auth/login) More login", 1_700_000_900);
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--with-children"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(branch(&output, "login")["state"], json!("updated"), "{output}");
  let pushed = &output["stacks"][0];
  let tip = git(&repo, &["rev-parse", &format!("refs/heads/{stack}")]);
  assert_ne!(tip, pushed_before);
  assert_eq!(
    (&pushed["name"], &pushed["remote_branch"], &pushed["lease"], &pushed["lease_source"], &pushed["commit"]),
    (&json!("auth"), &json!(stack), &json!(short_id(&pushed_before)), &json!("expected"), &json!(short_id(&tip)))
  );
  assert_eq!(
    (&pushed["pull_request"]["head"], &pushed["pull_request"]["base"], &pushed["pull_request"]["title"]),
    (&json!(stack), &json!("master"), &json!("Add auth, with the 2 branches built on it"))
  );
  assert_eq!(remote_tip(&remote, &stack), tip);
  let (_, schema) = run_in(&repo, &["schema", "--all"]);
  assert_covered(&schema["outputs"]["push"], &schema["outputs"]["push"], &output, "push");

  // on the remote as it is: nothing to push
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--with-children"]);
  assert_eq!((exit_code, output.get("stacks"), output.get("stack_warnings")), (0, None, None), "{output}");

  // its remote status is of origin: another remote gets the branches, not the stack branch
  let mirror = tempfile::tempdir().unwrap();
  git(&repo, &["init", "--bare", "-q", mirror.path().to_str().unwrap()]);
  disable_background_maintenance(mirror.path());
  git(&repo, &["remote", "add", "mirror", mirror.path().to_str().unwrap()]);
  git(&repo, &["update-ref", "refs/remotes/mirror/master", "refs/remotes/origin/master"]);
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--with-children", "--remote", "mirror"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (output.get("stacks"), &output["stack_warnings"]),
    (None, &json!([format!("{stack} was not pushed: its remote status is of origin, not of mirror")]))
  );
  assert_eq!(remote_tip(&mirror, &stack), "");
}

#[test_log::test]
fn with_children_refuses_a_blocked_child() {
  let (repo, remote) = repo_with_remote(&["(docs) Add docs", "(auth) Add auth"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  // login changes a file only docs has: it conflicts on auth, and oauth on it is blocked
  write(&repo, "f0.txt", "changed\n");
  commit_all(&repo, "(auth/login) Change docs", 1_700_000_900);
  write(&repo, "oauth.txt", "oauth\n");
  commit_all(&repo, "(login/oauth) Add oauth", 1_700_001_000);

  let (exit_code, output) = run_in(&repo, &["push", "auth", "--with-children"]);
  assert_error(&output, exit_code, (4, "push_refused"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.contains("login is not synced") && message.contains("oauth is not synced"), "{output}");
  assert_eq!(remote_tip(&remote, AUTH), "");
}

#[test_log::test]
fn all_needs_yes_and_unsynced_branches_are_refused() {
  let (repo, _remote) = repo_with_remote(&["(a) Add shared", "(b) Unrelated"]);
  let (exit_code, output) = run_in(&repo, &["push", "--all"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let (exit_code, output) = run_in(&repo, &["push"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let (exit_code, output) = run_in(&repo, &["push", "--all", "--yes"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["branches"].as_array().unwrap().len(), 2);

  // c changes a file only a has: it conflicts on the baseline and is not pushed
  write(&repo, "f0.txt", "changed\n");
  commit_all(&repo, "(c) Change shared", 1_700_000_900);
  let (exit_code, output) = run_in(&repo, &["push", "c"]);
  assert_error(&output, exit_code, (4, "push_refused"));
  assert!(
    output["error"]["message"]
      .as_str()
      .unwrap()
      .contains("c is not synced (conflict in f0.txt): fix it first, `bd sync` says what holds it"),
    "{output}"
  );
  assert_eq!(next_steps(&output["error"]), [("bd sync".to_string(), false)]);
}

/// A parked branch stays local: `--all` and `--with-children` leave it out (`skipped`), and the branches built on it, which
/// carry its commits; a named one is refused
#[test_log::test]
fn a_parked_branch_and_what_is_built_on_it_are_never_pushed() {
  let (repo, remote) = repo_with_remote(&["(auth) Add auth", "(auth/login) Add login", "(docs) Add docs"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let (exit_code, output) = run_in(&repo, &["park", "login", "--note", "IJPL-1"]);
  assert_eq!(exit_code, 0, "{output}");

  for args in [&["push", "login"][..], &["push", "login", "--alone"]] {
    let (exit_code, output) = run_in(&repo, args);
    assert_error(&output, exit_code, (4, "parked"));
    assert!(output["error"]["message"].as_str().unwrap().contains("login is parked: IJPL-1"), "{output}");
  }
  assert_eq!(remote_tip(&remote, "test-user/virtual/login"), "");

  let (exit_code, output) = run_in(&repo, &["push", "auth", "--with-children"]);
  assert_eq!(exit_code, 0, "{output}");
  let names: Vec<&Value> = output["branches"].as_array().unwrap().iter().map(|branch| &branch["name"]).collect();
  assert_eq!(names, [&json!("auth")]);
  assert_eq!(output["skipped"], json!([{"name": "login", "code": "parked", "message": "login is parked: IJPL-1"}]));
  let (_, schema) = run_in(&repo, &["schema", "--all"]);
  assert_covered(&schema["outputs"]["push"], &schema["outputs"]["push"], &output, "push");

  // a parked root keeps its child local too
  run_in(&repo, &["unpark", "login"]);
  run_in(&repo, &["park", "auth"]);
  let (exit_code, output) = run_in(&repo, &["push", "login"]);
  assert_error(&output, exit_code, (4, "parked"));
  assert!(output["error"]["message"].as_str().unwrap().contains("login is built on parked auth"), "{output}");
  let (exit_code, output) = run_in(&repo, &["push", "--all", "--yes"]);
  assert_eq!(exit_code, 0, "{output}");
  let names: Vec<&Value> = output["branches"].as_array().unwrap().iter().map(|branch| &branch["name"]).collect();
  assert_eq!(names, [&json!("docs")]);
  let skipped: Vec<&Value> = output["skipped"].as_array().unwrap().iter().map(|branch| &branch["name"]).collect();
  assert_eq!(skipped, [&json!("auth"), &json!("login")]);

  // nothing but parked branches left: refused
  run_in(&repo, &["park", "docs"]);
  let (exit_code, output) = run_in(&repo, &["push", "--all", "--yes"]);
  assert_error(&output, exit_code, (4, "parked"));
}

/// Someone else commits `subject` (`their_change`) onto the remote branch `AUTH` from another clone; returns the commit
fn push_from_elsewhere(remote: &TempDir, subject: &str) -> String {
  push_from_elsewhere_to(remote, AUTH, subject)
}

/// Someone else's fetched commit that the branch has by content (the user brought it in): the remote lacks nothing,
/// and a plain push goes through on the first try, although Branch Deck's last push is older
#[test_log::test]
fn a_plain_push_replaces_a_remote_commit_the_branch_has_by_content() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let theirs = push_from_elsewhere(&remote, "Their fix");
  git(&repo, &["fetch", "-q", "origin"]);
  let (file, content) = their_change("Their fix");
  write(&repo, &file, &content);
  commit_all(&repo, "(auth) Their fix", 1_700_000_500);

  let output = push_auth_to_its_tip(&repo, &remote);
  let auth = branch(&output, "auth");
  assert_eq!((&auth["lease"], &auth["lease_source"]), (&json!(short_id(&theirs)), &json!("expected")));
}

/// `bd push auth` went: the remote branch is at the local virtual branch. The output
fn push_auth_to_its_tip(repo: &TestRepo, remote: &TempDir) -> Value {
  let (exit_code, output) = run_in(repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(remote_tip(remote, AUTH), git(repo, &["rev-parse", &format!("refs/heads/{AUTH}")]));
  output
}

/// The subjects of the `dropped_commits` of a remote status in an output
fn dropped_subjects(remote: &Value) -> Vec<&str> {
  remote["dropped_commits"]
    .as_array()
    .unwrap()
    .iter()
    .map(|commit| commit["subject"].as_str().unwrap())
    .collect()
}

#[test_log::test]
fn a_moved_remote_branch_is_fetched_and_reported_and_force_replaces_it() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");

  // someone else adds a commit to the remote branch; the remote-tracking ref doesn't know
  let other = push_from_elsewhere(&remote, "Someone else");
  write(&repo, "more.txt", "more\n");
  commit_all(&repo, "(auth) Add logout", 1_700_000_500);

  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_error(&output, exit_code, (4, "lease_rejected"));
  assert_eq!(output["error"]["retryable"], json!(false));
  assert_eq!(remote_tip(&remote, AUTH), other, "the remote is unchanged");
  // bd fetched the branch and says what the remote has now
  assert_eq!(git(&repo, &["rev-parse", &format!("refs/remotes/origin/{AUTH}")]), other);
  let rejected = &output["error"]["branches"][0];
  assert_eq!(rejected["name"], json!("auth"));
  assert_eq!((&rejected["remote"]["head"], &rejected["remote"]["behind"]), (&json!(short_id(&other)), &json!(1)));
  assert_eq!(dropped_subjects(&rejected["remote"]), ["Someone else"]);
  assert_eq!(rejected["remote"]["dropped_commits"][0]["author"], json!("Other"));
  // the push that replaces it, leased on what bd fetched: the user's to approve
  let replace = format!("bd push auth --force --expect-remote {}", short_id(&other));
  assert_eq!(next_steps(&output["error"]), [(replace.clone(), true)]);
  assert!(
    output["error"]["message"].as_str().unwrap().contains("once the user approved, push again with --force"),
    "{output}"
  );
  let (_, schema) = run_in(&repo, &["schema", "--all"]);
  assert_covered(&schema["outputs"]["error"], &schema["outputs"]["error"], &output, "error");

  // the remote branch has a commit the branch lacks: a push needs --force, leased on what the sync saw
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_error(&output, exit_code, (4, "push_refused"));
  assert_eq!(dropped_subjects(&output["error"]["branches"][0]["remote"]), ["Someone else"]);
  assert!(
    output["error"]["message"].as_str().unwrap().contains("once the user approved, --force replaces them"),
    "{output}"
  );
  assert_eq!(next_steps(&output["error"]), [(replace, true)]);
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--force"]);
  assert_pushed(&output, exit_code, "forced", &other);
  assert_eq!(remote_tip(&remote, AUTH), git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]));
}

/// pushes again over the new remote tip when the remote has no commits the branch lacks: nothing is lost. Someone moved
/// the remote branch back to an older commit of the branch since the sync: the lease the sync saw is refused, bd fetches
/// it and pushes again, once, leased on what the remote has now, and says so (`pushed_over`), as the app's push does
#[test_log::test]
fn pushes_again_over_a_moved_remote_that_lacks_nothing_of_the_branch() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  let older = auth_remote_moved_back(&repo, &remote);

  let output = push_auth_to_its_tip(&repo, &remote);
  let auth = branch(&output, "auth");
  assert_eq!((&auth["lease"], &auth["pushed_over"]), (&json!(short_id(&older)), &json!(true)));
  let (_, schema) = run_in(&repo, &["schema", "--all"]);
  assert_covered(&schema["outputs"]["push"], &schema["outputs"]["push"], &output, "push");
}

/// The push of a stack is atomic: someone else's commit on the remote branch of `login` refuses `auth` too. bd fetched
/// both, and `lease_rejected` names `login` alone, with what its remote branch has now
#[test_log::test]
fn a_moved_lease_refuses_the_whole_stack_and_names_the_branch_that_moved() {
  let (repo, remote) = repo_with_remote(&["(auth) Add auth", "(auth/login) Add login"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let (exit_code, output) = run_in(&repo, &["push", "login"]);
  assert_eq!(exit_code, 0, "{output}");
  let login = "test-user/virtual/login";
  let pushed_auth = remote_tip(&remote, AUTH);
  push_from_elsewhere_to(&remote, login, "Someone else");
  write(&repo, "more.txt", "more\n");
  commit_all(&repo, "(auth) Add more", 1_700_000_500);

  let (exit_code, output) = run_in(&repo, &["push", "login"]);
  assert_error(&output, exit_code, (4, "lease_rejected"));
  assert_eq!(remote_tip(&remote, AUTH), pushed_auth, "nothing was pushed");
  let rejected = output["error"]["branches"].as_array().unwrap();
  let names: Vec<&Value> = rejected.iter().map(|branch| &branch["name"]).collect();
  assert_eq!(names, [&json!("login")]);
  assert_eq!(dropped_subjects(&rejected[0]["remote"]), ["Someone else"]);
}

/// `bd push` holds the repository lock while it syncs and decides, not while it pushes: the commits and the leases are
/// fixed under it, and the network push sends them after it, so another writer never waits for the network. The hook of
/// the remote, which runs while the push goes, finds no holder record of the lock (`branch-deck.lock.holder`)
#[test_log::test]
fn the_network_push_holds_no_repository_lock() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  let holder = repo.path().join(".git/branch-deck.lock.holder");
  let seen = remote.path().join("lock-seen");
  pre_receive_hook(
    remote.path(),
    &format!(
      "if [ -e '{}' ]; then echo held > '{}'; else echo free > '{}'; fi",
      holder.display(),
      seen.display(),
      seen.display()
    ),
  );

  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(std::fs::read_to_string(&seen).unwrap(), "free\n");
  assert_eq!(remote_tip(&remote, AUTH), git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]));
}

/// `bd push` asks Patronus for the robots that hold its branches before it takes the repository lock: while the list
/// waits at Patronus, another writer of the repository (`bd park`) takes the lock at once instead of failing as `locked`
/// after its wait, and the push syncs on what it left once Patronus answers
#[test_log::test]
fn the_robot_list_of_a_push_is_read_before_the_repository_lock() {
  let space = Space::new(&["(auth) Add login", "(docs) Add docs"]);
  let list = space.fake.hold("GET", "/app/rest/v1/robots");
  let environment = space.environment();
  let push = std::thread::spawn(move || run_json(&environment, &["--prefix", PREFIX, "push", "auth"]));
  space.runtime.block_on(list.arrived());

  let (exit_code, output) = run_json(&space.environment(), &["--prefix", PREFIX, "park", "docs"]);
  assert_eq!(exit_code, 0, "{output}");
  list.open();
  let (exit_code, output) = push.join().unwrap();
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(space.remote_tip(AUTH), git(&space.repo, &["rev-parse", &format!("refs/heads/{AUTH}")]));
}

/// A push whose commits add more than `branchdeck.pushSizeLimit` of files is `push_too_large` before bd asks the remote,
/// a dry run too: the message names the directory that holds most of it, `paths` it and the largest files, the remote
/// has nothing; an invalid limit refuses the push as a wrong argument
#[test_log::test]
fn a_push_over_the_size_limit_is_push_too_large_before_the_remote() {
  let (repo, remote) = repo_with_remote(&[]);
  write(&repo, "tool/target/debug/app", "x".repeat(3000));
  write(&repo, "tool/src/main.rs", "fn main() {}\n");
  commit_all(&repo, "(auth) Add the tool", 1_700_000_000);
  git(&repo, &["config", "branchdeck.pushSizeLimit", "2K"]);

  for args in [&["push", "auth", "--dry-run"][..], &["push", "auth"]] {
    let (exit_code, output) = run_in(&repo, args);
    assert_error(&output, exit_code, (4, "push_too_large"));
    let message = output["error"]["message"].as_str().unwrap();
    assert!(message.contains("2.9 KiB of it in tool/target/debug/ (1 file)"), "{output}");
    assert!(message.ends_with("never raise branchdeck.pushSizeLimit yourself"), "{output}");
    assert_eq!(
      output["error"]["paths"],
      json!(["tool/target/debug/", "tool/target/debug/app", "tool/src/main.rs"]),
      "{output}"
    );
  }
  assert_eq!(remote_tip(&remote, AUTH), "");

  git(&repo, &["config", "branchdeck.pushSizeLimit", "lots"]);
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
}

/// The leases held, yet the remote refused the push (a hook, a protected branch): `push_refused` with what git and the
/// remote said, nothing pushed, not `lease_rejected`
#[test_log::test]
fn a_push_the_remote_refuses_with_its_leases_holding_is_push_refused() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  refuse_every_push(remote.path());

  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_error(&output, exit_code, (4, "push_refused"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.contains("the remote refused the push"), "{output}");
  assert!(message.ends_with("Show the user what the remote answered: no flag of bd gets past it"), "{output}");
  assert!(message.contains("pre-receive hook declined). remote: protected branch"), "what the hook said: {output}");
  assert_eq!(remote_tip(&remote, AUTH), "");
}

/// A remote other than `origin`, which the sync compares with: bd compares with it the same way, so the lease, the
/// refusal of a branch behind it and the status of a rejected push are about that remote
#[test_log::test]
fn another_remote_is_compared_with_as_origin_is() {
  let (repo, origin) = repo_with_remote(&["(auth) Add login"]);
  let mirror = tempfile::tempdir().unwrap();
  git(&repo, &["init", "--bare", "-q", mirror.path().to_str().unwrap()]);
  disable_background_maintenance(mirror.path());
  git(&repo, &["remote", "add", "mirror", mirror.path().to_str().unwrap()]);
  // the baseline may be detected on either remote
  git(&repo, &["update-ref", "refs/remotes/mirror/master", "refs/remotes/origin/master"]);
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--remote", "nowhere"]);
  assert_error(&output, exit_code, (4, "no_remote"));

  let (exit_code, output) = run_in(&repo, &["push", "auth", "--remote", "mirror"]);
  assert_eq!(exit_code, 0, "{output}");
  let tip = git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]);
  assert_eq!((&output["remote"], &branch(&output, "auth")["lease_source"]), (&json!("mirror"), &json!("absent")));
  assert_eq!((remote_tip(&mirror, AUTH), remote_tip(&origin, AUTH)), (tip.clone(), String::new()));
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--remote", "mirror"]);
  assert_pushed(&output, exit_code, "up_to_date", &tip);

  // someone else pushes to the mirror: rejected, fetched and reported
  let other = push_from_elsewhere(&mirror, "Someone else");
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--remote", "mirror"]);
  assert_error(&output, exit_code, (4, "lease_rejected"));
  let rejected = &output["error"]["branches"][0]["remote"];
  assert_eq!((&rejected["head"], &rejected["behind"]), (&json!(short_id(&other)), &json!(1)));
  assert_eq!(dropped_subjects(rejected), ["Someone else"]);

  // fetched now: the branch is behind the mirror, and replacing it needs the user's approval
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--remote", "mirror"]);
  assert_error(&output, exit_code, (4, "push_refused"));
  assert_eq!(dropped_subjects(&output["error"]["branches"][0]["remote"]), ["Someone else"]);
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--remote", "mirror", "--force", "--expect-remote", &other]);
  assert_pushed(&output, exit_code, "forced", &other);
  assert_eq!(remote_tip(&mirror, AUTH), tip);
}

/// --force replaces the remote head the sync compared with, not a commit pushed after the last fetch;
/// --expect-remote pins the lease to the head the user saw
#[test_log::test]
fn force_drops_only_what_the_sync_saw_and_expect_remote_pins_the_lease() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  push_from_elsewhere(&remote, "Seen fix");
  git(&repo, &["fetch", "-q", "origin"]);
  let newer = push_from_elsewhere(&remote, "Unseen fix");

  let (exit_code, output) = run_in(&repo, &["push", "auth", "--force"]);
  assert_error(&output, exit_code, (4, "lease_rejected"));
  assert_eq!(remote_tip(&remote, AUTH), newer, "the unseen commit is not dropped");
  let rejected = &output["error"]["branches"][0]["remote"];
  assert_eq!(dropped_subjects(rejected), ["Unseen fix", "Seen fix"]);
  let head = rejected["head"].as_str().unwrap().to_string();
  assert_eq!(head, short_id(&newer));

  let (exit_code, output) = run_in(&repo, &["push", "auth", "--force", "--expect-remote", "f00d"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let (exit_code, output) = run_in(&repo, &["push", "auth", "other", "--expect-remote", &head]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));

  // the user saw the two dropped commits and approved
  let (exit_code, output) = run_in(&repo, &["push", "auth", "--force", "--expect-remote", &head]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    (&branch(&output, "auth")["lease"], &branch(&output, "auth")["lease_source"]),
    (&json!(head), &json!("expected"))
  );
  assert_eq!(remote_tip(&remote, AUTH), git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]));
}

/// The app's push of a branch, as the push button runs it: leased on the remote head its status showed
fn app_push(repo: &TestRepo, branch: &str) {
  let (remote_tip, exit_code) = repo
    .git_executor()
    .execute_command_with_status(
      &["rev-parse", "--verify", "--quiet", &format!("refs/remotes/origin/{PREFIX}/virtual/{branch}")],
      repo.path().to_str().unwrap(),
    )
    .unwrap();
  let path = repo.path().to_str().unwrap();
  let expected = (exit_code == 0).then_some(remote_tip);
  let [push] = prepare_push_expecting(repo.git_executor(), path, &[(format!("{PREFIX}/virtual/{branch}"), expected)])
    .unwrap()
    .try_into()
    .unwrap();
  let target = PushTarget {
    repository_path: path,
    remote: "origin",
    baseline: "origin/master",
    size_limit: None,
  };
  let leased = LeasedBranch {
    name: branch.to_string(),
    push,
    parent_ref: None,
    my_email: None,
  };
  let prepared = PreparedPush::leased(&target, vec![leased], PushOptions { atomic: false, dry_run: false });
  assert!(matches!(prepared.push_leased(repo.git_executor(), &mut |_| {}).unwrap(), LeasedPush::Pushed(_)));
}

#[test_log::test]
fn the_app_and_bd_share_the_lease() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");

  // the app pushes, then bd: the app's push moved the remote head bd's sync sees
  app_push(&repo, "auth");
  assert_pushes_up_to_date_on_the_seen_head(&repo);
  write(&repo, "more.txt", "more\n");
  commit_all(&repo, "(auth) Add logout", 1_700_000_500);
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(branch(&output, "auth")["state"], json!("updated"));

  // bd pushed, then the app
  write(&repo, "even-more.txt", "more\n");
  commit_all(&repo, "(auth) Add reset", 1_700_000_600);
  run_in(&repo, &["sync"]);
  app_push(&repo, "auth");
  let tip = git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]);
  assert_eq!(remote_tip(&remote, AUTH), tip);
  // bd again: the app's push moved the remote head, so this is not a stale lease
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(branch(&output, "auth")["state"], json!("up_to_date"));
}

/// A commit amended at the root of a stack: the root's remote branch has its older version (amended), the remote
/// branches built on it have it as the commit of an ancestor (restacked). Neither is someone's work a push drops: `bd
/// push` of the leaf pushes the stack without --force, also the restacked middle, which has no commit of its own to push
#[test_log::test]
fn amended_and_restacked_commits_push_without_force() {
  let subjects = ["(auth) Add auth", "(auth/login) Add login", "(login/oauth) Add oauth"];
  let (repo, remote) = repo_with_remote(&subjects);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let (exit_code, output) = run_in(&repo, &["push", "oauth"]);
  assert_eq!(exit_code, 0, "{output}");

  // the auth commit amended with its author time and subject; the others as they were
  git(&repo, &["reset", "--hard", "HEAD~3"]);
  write(&repo, "f0.txt", "(auth) Add auth, amended");
  commit_all(&repo, subjects[0], 1_700_000_000);
  for (index, subject) in subjects.iter().enumerate().skip(1) {
    write(&repo, &format!("f{index}.txt"), *subject);
    commit_all(&repo, subject, 1_700_000_000 + i64::try_from(index).unwrap() * 100);
  }

  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  let auth = &branch(&output, "auth")["remote"];
  assert_eq!((&auth["unpushed"], &auth["amended"], &auth["behind"]), (&json!(1), &json!(1), &json!(0)));
  for name in ["login", "oauth"] {
    let remote = &branch(&output, name)["remote"];
    assert_eq!((&remote["unpushed"], &remote["restacked"], &remote["behind"]), (&json!(0), &json!(1), &json!(0)), "{name}");
  }
  assert!(
    auth.get("restacked").is_none() && branch(&output, "login")["remote"].get("amended").is_none(),
    "a zero count is left out: {output}"
  );
  let (_, schema) = run_in(&repo, &["schema", "--all"]);
  assert_covered(&schema["outputs"]["sync"], &schema["outputs"]["sync"], &output, "sync");
  // their next step: a push without --force, also for the restacked branches with no commit of their own
  for name in ["auth", "login", "oauth"] {
    assert_eq!(branch(&output, name)["push"], json!(format!("bd push {name}")));
  }

  let (exit_code, output) = run_in(&repo, &["push", "oauth"]);
  assert_eq!(exit_code, 0, "{output}");
  let statuses: Vec<(&Value, &Value)> = output["branches"].as_array().unwrap().iter().map(|branch| (&branch["name"], &branch["state"])).collect();
  assert_eq!(
    statuses,
    [(&json!("auth"), &json!("forced")), (&json!("login"), &json!("forced")), (&json!("oauth"), &json!("forced"))]
  );
  assert_eq!(
    remote_tip(&remote, "test-user/virtual/login"),
    git(&repo, &["rev-parse", "refs/heads/test-user/virtual/login"])
  );
}

/// The remote branch of a member of a stack that landed whole has its commits as they were before the landing: older
/// versions of commits the baseline has, by identity, though one has another patch there. Nothing of them is lost, so
/// the sync counts them `landed`, not `behind`, and `bd push` replaces them without --force, naming them in
/// `replaced_landed`; the remote branch gets the baseline and the one new commit
#[test_log::test]
fn a_push_replaces_the_remote_commits_the_baseline_landed_without_force() {
  let (repo, remote) = repo_with_remote(&LANDED_SUBJECTS);
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let pushed = remote_tip(&remote, AUTH);
  let landed = land_auth_by_content(&repo);

  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  let auth = &branch(&output, "auth")["remote"];
  assert_eq!((&auth["unpushed"], &auth["landed"], &auth["behind"]), (&json!(1), &json!(1), &json!(0)), "{output}");
  // the copy of `Add auth` on the remote, below the one of `Add login`, which the baseline has as it is
  let pushed_auth = git(&repo, &["rev-parse", &format!("{pushed}~1")]);
  assert_eq!(
    auth["landed_commits"],
    json!([{"commit": short_id(&pushed_auth), "subject": "Add auth", "baseline_commit": short_id(&landed)}])
  );
  assert_eq!(branch(&output, "auth")["push"], json!("bd push auth"));
  let (_, schema) = run_in(&repo, &["schema", "--all"]);
  assert_covered(&schema["outputs"]["sync"], &schema["outputs"]["sync"], &output, "sync");

  let (exit_code, output) = run_in(&repo, &["push", "auth", "--dry-run"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(remote_tip(&remote, AUTH), pushed, "a dry run pushes nothing");
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_pushed(&output, exit_code, "forced", &pushed);
  assert_eq!(output["branches"][0]["replaced_landed"], auth["landed_commits"]);
  assert_covered(&schema["outputs"]["push"], &schema["outputs"]["push"], &output, "push");
  let tip = git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]);
  assert_eq!(remote_tip(&remote, AUTH), tip);
  assert_eq!(
    git(&repo, &["rev-list", "--count", &format!("refs/remotes/origin/master..{tip}")]),
    "1",
    "the new commit on the baseline"
  );
}

/// The same landing, and someone else's commit on the remote branch, which the baseline lacks: the push stays refused
/// without --force, and `dropped_commits` names only that commit, the landed one counted apart
#[test_log::test]
fn a_remote_commit_the_baseline_lacks_still_refuses_the_push() {
  let (repo, remote) = repo_with_remote(&LANDED_SUBJECTS);
  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let theirs = push_from_elsewhere_to(&remote, AUTH, "Someone else");
  git(&repo, &["fetch", "-q", "origin"]);
  land_auth_by_content(&repo);

  let (exit_code, output) = run_in(&repo, &["push", "auth"]);
  assert_error(&output, exit_code, (4, "push_refused"));
  let refused = &output["error"]["branches"][0]["remote"];
  assert_eq!(
    (&refused["head"], &refused["behind"], &refused["landed"]),
    (&json!(short_id(&theirs)), &json!(1), &json!(1))
  );
  assert_eq!(dropped_subjects(refused), ["Someone else"]);
  assert_eq!(remote_tip(&remote, AUTH), theirs, "nothing was pushed");
}

/// `bd push` with the arguments of a `push` line of the sync output
fn run_push_line(repo: &TestRepo, line: &str) -> (i32, Value) {
  let args: Vec<&str> = line.strip_prefix("bd ").unwrap_or_else(|| panic!("{line}")).split(' ').collect();
  run_in(repo, &args)
}

/// The sync names the push each branch needs: `--force --expect-remote` for a branch whose push drops a commit someone
/// else pushed, `--force` alone when a parent it takes along has commits for the remote, and none when it's up to date
#[test_log::test]
fn sync_names_the_push_of_each_branch() {
  let (repo, remote) = repo_with_remote(&["(auth) Add login"]);
  let (_, output) = run_in(&repo, &["sync"]);
  assert_eq!(branch(&output, "auth")["push"], json!("bd push auth"));
  run_in(&repo, &["push", "auth"]);
  let (_, output) = run_in(&repo, &["sync"]);
  assert_eq!(branch(&output, "auth").get("push"), None, "up to date: {output}");

  let theirs = push_from_elsewhere(&remote, "Their fix");
  git(&repo, &["fetch", "-q", "origin"]);
  let (_, output) = run_in(&repo, &["sync"]);
  let line = format!("bd push auth --force --expect-remote {}", short_id(&theirs));
  assert_eq!(branch(&output, "auth")["push"], json!(line));
  let (exit_code, output) = run_push_line(&repo, &line);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(remote_tip(&remote, AUTH), git(&repo, &["rev-parse", &format!("refs/heads/{AUTH}")]));
}

#[test_log::test]
fn a_child_behind_its_remote_with_a_parent_to_push_forces_without_expect_remote() {
  let (repo, remote) = repo_with_remote(&["(auth) Add auth", "(auth/login) Add login"]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  run_in(&repo, &["push", "login"]);
  push_from_elsewhere_to(&remote, "test-user/virtual/login", "Their fix");
  git(&repo, &["fetch", "-q", "origin"]);
  write(&repo, "auth2.txt", "more auth\n");
  commit_all(&repo, "(auth) More auth", 1_700_000_900);

  let (_, output) = run_in(&repo, &["sync"]);
  assert_eq!(branch(&output, "auth")["push"], json!("bd push auth"));
  let line = branch(&output, "login")["push"].as_str().unwrap().to_string();
  assert_eq!(line, "bd push login --force");
  let (exit_code, output) = run_push_line(&repo, &line);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    remote_tip(&remote, "test-user/virtual/login"),
    git(&repo, &["rev-parse", "refs/heads/test-user/virtual/login"])
  );
}

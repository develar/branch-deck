use crate::fixture_test::{PREFIX, commit_all, commit_subjects, environment, git, numbered_lines, repo_with_baseline, repository, run_in, run_json, with_header, write};
use git_ops::revs::short_id;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use test_utils::git_test_utils::TestRepo;

/// Two prefixed commits, an issue-key commit and an unassigned one, at fixed times
fn grouped_repo() -> (TestRepo, Vec<String>) {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let mut ids = Vec::new();
  write(&repo, "auth.rs", "fn login() {}\n");
  ids.push(commit_all(&repo, "(auth) Add login", 1_700_000_100));
  write(&repo, "crash.rs", "fix\n");
  ids.push(commit_all(&repo, "ABC-12 Fix crash", 1_700_000_200));
  write(&repo, "misc.txt", "misc\n");
  ids.push(commit_all(&repo, "Tidy up", 1_700_000_300));
  write(&repo, "auth.rs", "fn login() {}\nfn logout() {}\n");
  ids.push(commit_all(&repo, "(auth) Add logout", 1_700_000_400));
  (repo, ids)
}

/// With stacked branches on: a parent and its child, a branch with conflicting parents and one with a missing parent
fn stacked_repo() -> (TestRepo, Vec<String>) {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let ids = commit_subjects(&repo, &["(auth) Add auth", "(auth/login) Add login", "(x/c) One", "(y/c) Two", "(ghost/oauth) Add oauth"]);
  (repo, ids)
}

#[test_log::test]
fn status_shape() {
  let (repo, ids) = grouped_repo();
  let (exit_code, output) = run_in(&repo, &["status"]);
  assert_eq!(exit_code, 0, "{output}");

  assert_eq!(
    output,
    with_header(
      &repo,
      json!({
      "head": { "id": short_id(&ids[3]), "branch": "master" },
      "baseline": "origin/master",
      "branch_count": 2,
      "unassigned_count": 1,
      "branches": [
        {
          "name": "auth",
          "origin": "prefix",
          "ref": "refs/heads/test-user/virtual/auth",
          "latest_time": 1_700_000_400,
          "commit_count": 2,
          "tip": null,
          "commits": [
            { "id": short_id(&ids[3]), "subject": "Add logout" },
            { "id": short_id(&ids[0]), "subject": "Add login" },
          ],
        },
        {
          "name": "ABC-12",
          "origin": "issue",
          "ref": "refs/heads/test-user/virtual/ABC-12",
          "issue_key": "ABC-12",
          "latest_time": 1_700_000_200,
          "commit_count": 1,
          "tip": null,
          "commits": [{ "id": short_id(&ids[1]), "subject": "ABC-12 Fix crash" }],
        },
      ],
      "unassigned": [{ "id": short_id(&ids[2]), "subject": "Tidy up" }],
      })
    )
  );
}

#[test_log::test]
fn status_filters_and_pages() {
  let (repo, ids) = grouped_repo();
  git(&repo, &["update-ref", "refs/heads/test-user/virtual/auth", &ids[0]]);

  let (exit_code, output) = run_in(&repo, &["status", "--branch", "auth", "--limit", "1", "--offset", "1"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output.get("unassigned"), None);
  let branches = output["branches"].as_array().unwrap();
  assert_eq!(branches.len(), 1);
  assert_eq!(branches[0]["tip"], json!(short_id(&ids[0])));
  assert_eq!(branches[0]["commit_count"], json!(2));
  assert_eq!(branches[0]["commits"], json!([{ "id": short_id(&ids[0]), "subject": "Add login" }]));

  let (exit_code, output) = run_in(&repo, &["status", "--unassigned"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output.get("branches"), None);
  assert_eq!(output["unassigned"], json!([{ "id": short_id(&ids[2]), "subject": "Tidy up" }]));

  let (exit_code, output) = run_in(&repo, &["status", "--limit", "1", "--offset", "1"]);
  assert_eq!(exit_code, 0, "{output}");
  let names: Vec<&Value> = output["branches"].as_array().unwrap().iter().map(|branch| &branch["name"]).collect();
  assert_eq!(names, [&json!("ABC-12")]);

  let (exit_code, output) = run_in(&repo, &["status", "--branch", "nope"]);
  assert_eq!(exit_code, 2);
  assert_eq!(output["error"]["code"], json!("unknown_branch"));
}

#[test_log::test]
fn full_ids() {
  let (repo, ids) = grouped_repo();
  let (_, output) = run_in(&repo, &["status", "--full-ids", "--unassigned"]);
  assert_eq!(output["head"]["id"], json!(ids[3]));
  assert_eq!(output["unassigned"][0]["id"], json!(ids[2]));
}

#[test_log::test]
fn prefix_from_app_settings() {
  let (repo, _) = grouped_repo();
  let settings = tempfile::NamedTempFile::new().unwrap();
  std::fs::write(settings.path(), r#"{"appSettings": {"globalUserBranchPrefix": "from-app"}}"#).unwrap();
  let mut environment = environment(repo.path());
  environment.settings_file = Some(settings.path().to_path_buf());

  let (exit_code, output) = run_json(&environment, &["status", "--unassigned"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["prefix"], json!("from-app"));

  // --prefix wins over the app setting
  let (_, output) = run_json(&environment, &["--prefix", "explicit", "status", "--unassigned"]);
  assert_eq!(output["prefix"], json!("explicit"));

  // the repository's own git prefix wins over the app setting
  repo.set_config("branchdeck.branchPrefix", "from-git").unwrap();
  let (_, output) = run_json(&environment, &["status", "--unassigned"]);
  assert_eq!(output["prefix"], json!("from-git"));
}

#[test_log::test]
fn prefix_from_git_config() {
  let (repo, _) = grouped_repo();
  repo.set_config("branchdeck.branchPrefix", "from-git").unwrap();
  let (exit_code, output) = run_json(&environment(repo.path()), &["status", "--unassigned"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["prefix"], json!("from-git"));
}

fn assert_error(output: &Value, code: &str) {
  assert_eq!(output["schema"], json!(2));
  assert_eq!(output["error"]["code"], json!(code), "{output}");
  assert!(output["error"]["message"].as_str().is_some_and(|message| !message.is_empty()));
  assert_eq!(output["error"]["commits"], json!([]));
  assert_eq!(output["error"]["retryable"], json!(false));
}

#[test_log::test]
fn empty_prefix_is_an_error() {
  let (repo, _) = grouped_repo();
  let (exit_code, output) = run_json(&environment(repo.path()), &["--prefix", "", "status"]);
  assert_eq!(exit_code, 2);
  assert_error(&output, "no_branch_prefix");

  // a repository-level empty value overrides any global one
  repo.set_config("branchdeck.branchPrefix", "").unwrap();
  let (exit_code, output) = run_json(&environment(repo.path()), &["status"]);
  assert_eq!(exit_code, 2);
  assert_error(&output, "no_branch_prefix");
}

#[test_log::test]
fn no_baseline_is_refused() {
  let repo = TestRepo::new();
  repo.create_commit("Initial", "README.md", "readme\n");
  repo.rename_branch("master", "trunk").unwrap();
  let (exit_code, output) = run_in(&repo, &["status"]);
  assert_eq!(exit_code, 4);
  assert_error(&output, "no_baseline");
  assert_eq!(
    output["error"]["message"],
    json!("Cannot find the baseline branch: the repository has no remote and no master or main branch")
  );
}

/// `git init` without a commit: every command that reads HEAD or the baseline says so, whichever it reads first
#[test_log::test]
fn no_commits_is_no_baseline() {
  let repo = TestRepo::new();
  let plan = repo.path().join("plan.json");
  std::fs::write(&plan, r#"{"branches":{}}"#).unwrap();
  let plan = plan.to_str().unwrap();
  let commands: [&[&str]; 8] = [
    &["status"],
    &["status", "--brief"],
    &["commits"],
    &["suggest"],
    &["check", "--plan", plan],
    &["sync"],
    &["fold"],
    &["update", "--dry-run"],
  ];
  for args in commands {
    let (exit_code, output) = run_in(&repo, args);
    assert_eq!(exit_code, 4, "{args:?}: {output}");
    assert_error(&output, "no_baseline");
    assert_eq!(
      output["error"]["message"],
      json!("The repository has no commits yet: commit to the main branch, then sync"),
      "{args:?}"
    );
  }
}

#[test_log::test]
fn not_a_repository() {
  let directory = tempfile::tempdir().unwrap();
  let (exit_code, output) = run_json(&environment(directory.path()), &["--prefix", PREFIX, "status"]);
  assert_eq!(exit_code, 2);
  assert_error(&output, "not_a_repository");

  let missing = directory.path().join("missing");
  let (exit_code, output) = run_json(&environment(directory.path()), &["-C", missing.to_str().unwrap(), "--prefix", PREFIX, "commits"]);
  assert_eq!(exit_code, 2);
  assert_error(&output, "not_a_repository");
}

#[test_log::test]
fn invalid_arguments() {
  let directory = tempfile::tempdir().unwrap();
  let (exit_code, output) = run_json(&environment(directory.path()), &["status", "--no-such-flag"]);
  assert_eq!(exit_code, 2);
  assert_error(&output, "invalid_arguments");
}

#[test_log::test]
fn subdirectory_resolves_to_top_level() {
  let (repo, _) = grouped_repo();
  write(&repo, "nested/dir/file.txt", "x\n");
  let (exit_code, output) = run_json(&environment(&repo.path().join("nested")), &["-C", "dir", "--prefix", PREFIX, "status", "--unassigned"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["repository"], json!(repository(&repo)));
}

#[test_log::test]
fn stats_per_branch() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let base = repo.head();
  write(&repo, "src/api/handler.rs", "a\nb\nc\n");
  write(&repo, "Cargo.lock", numbered_lines(10));
  commit_all(&repo, "(api) Add handler", 1_700_000_100);
  write(&repo, "src/api/routes.rs", "r1\nr2\n");
  write(&repo, "docs/api.md", "doc\n");
  commit_all(&repo, "(api) PROJ-7 Add routes", 1_700_000_200);
  write(&repo, "src/api/handler.rs", "a\nB\nc\n");
  let tip = commit_all(&repo, "(api) Fix handler", 1_700_000_300);
  git(&repo, &["update-ref", "refs/heads/test-user/virtual/api", &tip]);

  let (exit_code, output) = run_in(&repo, &["status", "--stats"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    output["branches"][0]["stats"],
    json!({
      "files": 4,
      "generated_files": 1,
      "additions": 7,
      "deletions": 1,
      "top_dirs": [{ "dir": "src/api", "lines": 7 }, { "dir": "docs", "lines": 1 }],
      "authors": [{ "author": "Test User <test@example.com>", "commits": 3 }],
      "first_time": 1_700_000_100,
      "last_time": 1_700_000_300,
      "issue_keys": ["PROJ-7"],
      "diffstat": { "base": short_id(&base), "files": 4, "additions": 6, "deletions": 0 },
    })
  );

  let (_, output) = run_in(&repo, &["status", "--stats", "--include-generated"]);
  let stats = &output["branches"][0]["stats"];
  assert_eq!((&stats["additions"], &stats["diffstat"]["additions"]), (&json!(17), &json!(16)));
  assert_eq!(stats["top_dirs"][0], json!({ "dir": ".", "lines": 10 }));

  // never synced: no diffstat
  git(&repo, &["update-ref", "-d", "refs/heads/test-user/virtual/api"]);
  let (_, output) = run_in(&repo, &["status", "--stats"]);
  assert_eq!(output["branches"][0]["stats"]["diffstat"], json!(null));

  // without --stats there are none
  let (_, output) = run_in(&repo, &["status"]);
  assert_eq!(output["branches"][0].get("stats"), None);
}

#[test_log::test]
fn stacks_in_status() {
  let (repo, ids) = stacked_repo();
  let (exit_code, output) = run_in(&repo, &["status"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["forest"], json!(["auth", "login", "c", "oauth"]));
  let branch = |name: &str| output["branches"].as_array().unwrap().iter().find(|branch| branch["name"] == json!(name)).unwrap().clone();
  assert_eq!((&branch("login")["parent"], &branch("login")["parent_state"]), (&json!("auth"), &json!("active")));
  assert_eq!(branch("auth").get("parent"), None);
  assert_eq!(branch("oauth")["parent_state"], json!("missing"));
  // a parent without commits is passed: built on the baseline
  assert_eq!((branch("oauth").get("blocked_by"), branch("oauth").get("effective_base")), (None, None));
  assert_eq!(branch("oauth")["pr_base_changed"], json!(true));
  assert_eq!(branch("login").get("pr_base_changed"), None);
  assert_eq!(branch("login")["effective_base"], json!("auth"));
  assert_eq!(
    branch("c")["problems"],
    json!([{ "message": "c has conflicting parents x, y", "commits": [short_id(&ids[2]), short_id(&ids[3])] }])
  );

  // `bd commits` groups by the leaf too
  let (_, commits) = run_in(&repo, &["commits"]);
  assert_eq!(commits["commits"][1]["group"], json!("login"));
}

#[test_log::test]
fn a_parent_integrated_since_the_last_sync_is_integrated_before_the_next_one() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  write(&repo, "auth.rs", "auth\n");
  let auth = commit_all(&repo, "(auth) Add auth", 1_700_000_000);
  // the last sync built `auth`; then upstream took its change, and the update dropped the commit
  git(&repo, &["update-ref", &format!("refs/heads/{PREFIX}/virtual/auth"), &auth]);
  git(&repo, &["reset", "-q", "--hard", "origin/master"]);
  write(&repo, "auth.rs", "auth\n");
  let upstream = commit_all(&repo, "Add auth", 1_700_000_100);
  git(&repo, &["update-ref", "refs/remotes/origin/master", &upstream]);
  write(&repo, "login.rs", "login\n");
  commit_all(&repo, "(auth/login) Add login", 1_700_000_200);

  let (exit_code, output) = run_in(&repo, &["status"]);
  assert_eq!(exit_code, 0, "{output}");
  let login = &output["branches"][0];
  assert_eq!((&login["name"], &login["parent_state"]), (&json!("login"), &json!("integrated")));
  assert_eq!((login.get("effective_base"), login.get("blocked_by")), (None, None));
  assert_eq!(git(&repo, &["notes", "--ref", "branch-deck/detection", "list"]), "", "status writes no note");
}

#[test_log::test]
fn brief_status() {
  let (repo, _) = grouped_repo();
  let (exit_code, output) = run_in(&repo, &["status", "--brief"]);
  assert_eq!(exit_code, 0, "{output}");
  // the fields of `bd status`, with the branches cut down
  let (_, full) = run_in(&repo, &["status"]);
  let mut expected = full;
  expected["branches"] = json!([
    { "name": "auth", "commit_count": 2, "newest": "Add logout" },
    // the issue key is the name
    { "name": "ABC-12", "commit_count": 1, "newest": "ABC-12 Fix crash" },
  ]);
  assert_eq!(output, expected);

  let (exit_code, output) = run_in(&repo, &["status", "--brief", "--branch", "auth"]);
  assert_eq!((exit_code, &output["error"]["code"]), (2, &json!("invalid_arguments")));
}

#[test_log::test]
fn brief_status_keeps_the_stack_state() {
  let (repo, ids) = stacked_repo();
  let (exit_code, output) = run_in(&repo, &["status", "--brief"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["forest"], json!(["auth", "login", "c", "oauth"]));
  assert_eq!(
    output["branches"],
    json!([
      { "name": "oauth", "commit_count": 1, "newest": "Add oauth", "parent": "ghost", "parent_source": "prefix", "pr_base_changed": true },
      {
        "name": "c",
        "commit_count": 2,
        "newest": "Two",
        "problems": [{ "message": "c has conflicting parents x, y", "commits": [short_id(&ids[2]), short_id(&ids[3])] }],
      },
      { "name": "login", "commit_count": 1, "newest": "Add login", "parent": "auth", "parent_source": "prefix" },
      { "name": "auth", "commit_count": 1, "newest": "Add auth" },
    ])
  );
}

#[test_log::test]
fn issue_groups_in_status() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  let ids = commit_subjects(&repo, &["ABC-1 Add parser", "ABC-2 Parse lists", "XYZ-3 Other"]);
  // the store as another tool may leave it: a stale key and a line that can't be used
  let (git_executor, path) = (repo.git_executor(), repo.path().to_str().unwrap());
  let blob = git_executor
    .cmd(&["hash-object", "-w", "--stdin"], path)
    .stdin(b"ABC-1\tparser\nABC-2\tparser\nABC-9\tparser\nnot a line\n")
    .text()
    .unwrap();
  let tree = git_executor
    .cmd(&["mktree"], path)
    .stdin(format!("100644 blob {}\tissue-groups\n", blob.trim()).as_bytes())
    .text()
    .unwrap();
  let commit = git(&repo, &["commit-tree", tree.trim(), "-m", "planted"]);
  git(&repo, &["update-ref", "refs/branch-deck/issue-groups", commit.trim()]);

  let (exit_code, output) = run_in(&repo, &["status"]);
  assert_eq!(exit_code, 0, "{output}");
  let branches: Vec<(&Value, &Value, Option<&Value>)> = output["branches"]
    .as_array()
    .unwrap()
    .iter()
    .map(|branch| (&branch["name"], &branch["origin"], branch.get("issue_keys")))
    .collect();
  assert_eq!(
    branches,
    [
      (&json!("XYZ-3"), &json!("issue"), None),
      (&json!("parser"), &json!("issue_group"), Some(&json!(["ABC-1", "ABC-2"]))),
    ]
  );
  assert_eq!(output["idle_issue_groups"], json!(["ABC-9"]));
  assert_eq!(output["issue_group_problems"][0]["line"], json!("not a line"));

  let (_, brief) = run_in(&repo, &["status", "--brief"]);
  assert_eq!(
    (&brief["branches"][1]["issue_keys"], &brief["idle_issue_groups"]),
    (&json!(["ABC-1", "ABC-2"]), &json!(["ABC-9"]))
  );
  assert_eq!(ids.len(), 3);
}

/// Point `refs/branch-deck/stack-parents` at a commit whose file `stack-parents` is `text`, as another tool may
fn plant_stack_parents(repo: &TestRepo, text: &str) {
  let (git_executor, path) = (repo.git_executor(), repo.path().to_str().unwrap());
  let blob = git_executor.cmd(&["hash-object", "-w", "--stdin"], path).stdin(text.as_bytes()).text().unwrap();
  let tree = git_executor
    .cmd(&["mktree"], path)
    .stdin(format!("100644 blob {}\tstack-parents\n", blob.trim()).as_bytes())
    .text()
    .unwrap();
  let commit = git(repo, &["commit-tree", tree.trim(), "-m", "planted"]);
  git(repo, &["update-ref", "refs/branch-deck/stack-parents", commit.trim()]);
}

#[test_log::test]
fn stored_parents_in_status() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  commit_subjects(&repo, &["(auth) Add auth", "ABC-1 Add login", "(auth/oauth) Add oauth"]);
  plant_stack_parents(&repo, "ABC-1\tauth\nghost\tauth\nnot a line\n");

  let (exit_code, output) = run_in(&repo, &["status"]);
  assert_eq!(exit_code, 0, "{output}");
  let parents: Vec<(&Value, Option<&Value>, Option<&Value>)> = output["branches"]
    .as_array()
    .unwrap()
    .iter()
    .map(|branch| (&branch["name"], branch.get("parent"), branch.get("parent_source")))
    .collect();
  assert_eq!(
    parents,
    [
      (&json!("oauth"), Some(&json!("auth")), Some(&json!("prefix"))),
      (&json!("ABC-1"), Some(&json!("auth")), Some(&json!("store"))),
      (&json!("auth"), None, None),
    ]
  );
  assert_eq!(output["idle_stack_parents"], json!(["ghost"]));
  assert_eq!(output["stack_parent_problems"][0]["line"], json!("not a line"));

  let (_, brief) = run_in(&repo, &["status", "--brief"]);
  assert_eq!((&brief["branches"][1]["parent_source"], &brief["idle_stack_parents"]), (&json!("store"), &json!(["ghost"])));

  // without stacked branches the map is not used
  repo.set_config("branchdeck.stacks", "false").unwrap();
  let (_, flat) = run_in(&repo, &["status"]);
  assert_eq!((flat.get("idle_stack_parents"), flat["branches"][1].get("parent")), (None, None));
}

#[test_log::test]
fn a_sync_drops_the_stack_parent_of_an_integrated_branch() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  write(&repo, "auth.rs", "auth\n");
  commit_all(&repo, "(auth) Add auth", 1_700_000_000);
  write(&repo, "login.rs", "login\n");
  commit_all(&repo, "(login) Add login", 1_700_000_100);
  let (exit_code, output) = run_in(&repo, &["restack", "login", "auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  // a line of another worktree, whose branch has no archive here
  let lines = git(&repo, &["cat-file", "-p", "refs/branch-deck/stack-parents:stack-parents"]);
  plant_stack_parents(&repo, &format!("{lines}\nghost\tauth\n"));

  // upstream took the change of login, and the update dropped its commit
  git(&repo, &["reset", "-q", "--hard", "origin/master"]);
  write(&repo, "login.rs", "login\n");
  let upstream = commit_all(&repo, "Add login", 1_700_000_200);
  git(&repo, &["update-ref", "refs/remotes/origin/master", &upstream]);
  write(&repo, "auth.rs", "auth\n");
  commit_all(&repo, "(auth) Add auth", 1_700_000_300);

  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(git(&repo, &["cat-file", "-p", "refs/branch-deck/stack-parents:stack-parents"]), "ghost\tauth");
  let (_, status) = run_in(&repo, &["status"]);
  assert_eq!(status["idle_stack_parents"], json!(["ghost"]));
}

/// A stack that forks has its stack branch: `bd sync` builds it, and `bd status` reads its state without a sync, from
/// the build the object cache remembers: `built` while the commits are those the ref was built of, `stale` after a new
/// commit, `blocked` while a member has a stack problem
#[test_log::test]
fn the_stack_branch_of_a_forking_stack_in_sync_and_status() {
  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  commit_subjects(&repo, &["(auth) Add auth", "(auth/login) Add login", "(docs) Add docs", "(auth/session) Add session"]);

  let (_, status) = run_in(&repo, &["status"]);
  let stale = json!({
    "root": "auth",
    "branch": format!("{PREFIX}/stack/auth"),
    "state": "stale",
    "tip": null,
    "error": "Not the build of these commits: `bd sync` builds it or says why not",
    "commits": 3,
    "members": ["auth", "login", "session"],
  });
  assert_eq!(status["stacks"], json!([stale]));

  let (exit_code, sync) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{sync}");
  let tip = repo.rev_parse(&format!("{PREFIX}/stack/auth")).unwrap();
  let stack = &sync["stacks"][0];
  assert_eq!((&stack["state"], &stack["tip"], &stack["commits"]), (&json!("built"), &json!(short_id(&tip)), &json!(3)));
  assert_eq!(stack["remote"], json!({ "exists": false, "unpushed": 3, "behind": 0, "head": null }));
  assert_eq!(
    git(&repo, &["log", "--format=%s", &format!("origin/master..{PREFIX}/stack/auth")]),
    "Add session\nAdd login\nAdd auth"
  );

  let (_, status) = run_in(&repo, &["status"]);
  let built = json!({
    "root": "auth",
    "branch": format!("{PREFIX}/stack/auth"),
    "state": "built",
    "tip": short_id(&tip),
    "commits": 3,
    "members": ["auth", "login", "session"],
  });
  assert_eq!(status["stacks"], json!([built]));
  let (_, brief) = run_in(&repo, &["status", "--brief"]);
  assert_eq!(brief["stacks"], json!([built]));
  let (_, branch) = run_in(&repo, &["status", "--branch", "auth"]);
  assert_eq!(branch.get("stacks"), None);

  commit_subjects(&repo, &["(login) Validate login"]);
  let (_, status) = run_in(&repo, &["status"]);
  assert_eq!((&status["stacks"][0]["state"], &status["stacks"][0]["tip"]), (&json!("stale"), &json!(short_id(&tip))));

  commit_subjects(&repo, &["(session/session) Loop"]);
  let (_, status) = run_in(&repo, &["status"]);
  let stack = &status["stacks"][0];
  assert_eq!(
    (&stack["state"], &stack["member"], &stack["reason"]),
    (&json!("blocked"), &json!("session"), &json!("error"))
  );
}

use crate::review_head::{Fixes, RemoteReview, ReviewExtension, delete_review, delete_review_at, extend_review, push_review, read_remote_review};
use git_ops::commit_signing::Signing;
use git_ops::fix_replay::{FixReplay, TakenFiles};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, push_to_bare_origin};

const HEAD: &str = "test/review/auth/head";
const BASE: &str = "test/review/auth/base";
/// A fix that applied as it is
const APPLIED: FixReplay = FixReplay::Applied { taken: Vec::new() };

/// What the bare `origin` has of the review of auth
fn on_origin(repo: &TestRepo) -> RemoteReview {
  read_remote_review(repo.git_executor(), repo.path().to_str().unwrap(), "origin", "test", "auth").unwrap()
}

/// A repository on a bare `origin` whose `master` it fetched, with auth committed on it: the fork point and the tip
fn repo_with_auth() -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  let fork = repo.create_commit("Initial commit", "README.md", "# Test Project\n");
  push_to_bare_origin(&repo);
  repo.git(&["fetch", "-q", "origin"]);
  let tip = repo.create_commit("(auth) Add auth", "auth.txt", "auth\n");
  (repo, fork, tip)
}

#[test]
fn the_head_and_its_fork_point_go_in_one_push_and_a_repeat_pushes_nothing() {
  let (repo, fork, tip) = repo_with_auth();
  let path = repo.path().to_str().unwrap();
  let git = repo.git_executor();
  assert_eq!(on_origin(&repo), RemoteReview::default());

  let pushed = push_review(git, path, "origin", "test", "auth", &tip, "refs/remotes/origin/master").unwrap();
  assert_eq!(
    (pushed.head.as_str(), pushed.base.as_str(), pushed.fork_point.as_str(), pushed.pushed),
    (HEAD, BASE, fork.as_str(), true)
  );
  assert_eq!(
    on_origin(&repo),
    RemoteReview {
      head: Some(tip.clone()),
      base: Some(fork)
    }
  );
  // both are where they go: no push
  let again = push_review(git, path, "origin", "test", "auth", &tip, "refs/remotes/origin/master").unwrap();
  assert!(!again.pushed);

  // the robot landed a newer commit: the head moves on, leased on the one the remote had
  let newer = repo.create_commit("(auth) Fix auth", "auth.txt", "auth 2\n");
  let moved = push_review(git, path, "origin", "test", "auth", &newer, "refs/remotes/origin/master").unwrap();
  assert!(moved.pushed);
  assert_eq!(on_origin(&repo).head, Some(newer));
}

#[test]
fn a_delete_takes_only_the_branches_the_remote_has() {
  let (repo, _, tip) = repo_with_auth();
  let path = repo.path().to_str().unwrap();
  let git = repo.git_executor();
  push_review(git, path, "origin", "test", "auth", &tip, "refs/remotes/origin/master").unwrap();
  // someone deleted the base already: a leased delete of it would fail the push whole
  repo.git(&["push", "-q", "origin", &format!(":refs/heads/{BASE}")]);

  assert_eq!(delete_review(git, path, "origin", "test", "auth").unwrap(), [HEAD]);
  assert_eq!(on_origin(&repo), RemoteReview::default());
  // none left: done
  assert_eq!(delete_review(git, path, "origin", "test", "auth").unwrap(), Vec::<String>::new());
}

#[test]
fn a_delete_of_a_review_closed_elsewhere_takes_only_the_branches_still_at_its_tips() {
  let (repo, fork, tip) = repo_with_auth();
  let path = repo.path().to_str().unwrap();
  let git = repo.git_executor();
  push_review(git, path, "origin", "test", "auth", &tip, "refs/remotes/origin/master").unwrap();
  let closed = RemoteReview {
    head: Some(tip),
    base: Some(fork.clone()),
  };
  // a newer review of auth took the head since: it stays, the base of the closed one goes
  let newer = repo.create_commit("(auth) Fix auth", "auth.txt", "auth 2\n");
  push_review(git, path, "origin", "test", "auth", &newer, "refs/remotes/origin/master").unwrap();

  assert_eq!(delete_review_at(git, path, "origin", "test", "auth", &closed).unwrap(), [BASE]);
  assert_eq!(
    on_origin(&repo),
    RemoteReview {
      head: Some(newer.clone()),
      base: None
    }
  );
  // nothing left at its tips: done, the newer head kept
  assert_eq!(delete_review_at(git, path, "origin", "test", "auth", &closed).unwrap(), Vec::<String>::new());
  assert_eq!(on_origin(&repo).head, Some(newer.clone()));

  // both still at its tips: both go in one push
  let both = RemoteReview {
    head: Some(newer),
    base: Some(fork),
  };
  push_review(git, path, "origin", "test", "auth", both.head.as_deref().unwrap(), "refs/remotes/origin/master").unwrap();
  assert_eq!(delete_review_at(git, path, "origin", "test", "auth", &both).unwrap(), [HEAD, BASE]);
  assert_eq!(on_origin(&repo), RemoteReview::default());
}

/// The target of the bare `origin` changes auth to `content` by a commit of someone else, which the repository fetches
fn change_target(repo: &TestRepo, content: &str) {
  repo.git(&["checkout", "-q", "-B", "landing", "origin/master"]);
  repo.create_commit("Rework auth", "auth.txt", content);
  repo.git(&["push", "-q", "origin", "HEAD:master"]);
  repo.git(&["fetch", "-q", "origin"]);
}

/// A fix of auth committed on the target as the repository fetched it, on a branch of its own: the commit its Safe Push
/// takes
fn fix_on_master(repo: &TestRepo, name: &str, content: &str) -> String {
  repo.git(&["checkout", "-q", "-B", name, "origin/master"]);
  repo.create_commit(&format!("(auth) {name}"), "auth.txt", content)
}

/// auth landed and its review shows what it landed (the tip `tip`)
fn landed_review() -> (TestRepo, String) {
  let (repo, _, tip) = repo_with_auth();
  push_review(
    repo.git_executor(),
    repo.path().to_str().unwrap(),
    "origin",
    "test",
    "auth",
    &tip,
    "refs/remotes/origin/master",
  )
  .unwrap();
  repo.land_copy_on_origin(&tip);
  (repo, tip)
}

fn extend(repo: &TestRepo, sources: &[String]) -> ReviewExtension {
  extend_review(repo.git_executor(), repo.path().to_str().unwrap(), "origin", "test", "auth", fixes(sources, Signing::Off)).unwrap()
}

/// The fixes `sources` past the target `origin/master`, signed per `signing`
fn fixes(sources: &[String], signing: Signing) -> Fixes<'_> {
  Fixes {
    sources,
    target_ref: "refs/remotes/origin/master",
    signing,
  }
}

#[test]
fn a_fix_that_landed_goes_onto_the_head_and_a_repeat_adds_nothing() {
  let (repo, tip) = landed_review();
  let fix = fix_on_master(&repo, "fix", "auth 2\n");
  repo.land_copy_on_origin(&fix);

  let extended = extend(&repo, std::slice::from_ref(&fix));
  assert_eq!((extended.old_head.as_str(), extended.fixes.as_slice()), (tip.as_str(), [APPLIED].as_slice()));
  assert_ne!(extended.head, tip);
  assert_eq!(on_origin(&repo).head, Some(extended.head.clone()));
  // the fix on the commits of the review, by its author, with its change only
  assert_eq!(repo.git(&["rev-parse", &format!("{}^", extended.head)]), tip);
  assert_eq!(repo.git(&["show", "-s", "--format=%s", &extended.head]), "(auth) fix");
  assert_eq!(repo.git(&["show", &format!("{}:auth.txt", extended.head)]), "auth 2");
  // the base stays the fork point
  assert_eq!(on_origin(&repo).base, Some(repo.git(&["rev-parse", &format!("{tip}^")])));

  // a repeat (a crash after the push): its change is on the head, nothing is pushed
  let again = extend(&repo, std::slice::from_ref(&fix));
  assert_eq!(
    (again.old_head.as_str(), again.head.as_str(), again.fixes.as_slice()),
    (extended.head.as_str(), extended.head.as_str(), [APPLIED].as_slice())
  );
}

/// A commit of the fixes on the target as the repository fetched it, on a branch `name` of its own, which deletes `file`
fn delete_on_master(repo: &TestRepo, name: &str, file: &str) -> String {
  repo.git(&["checkout", "-q", "-B", name, "origin/master"]);
  repo.git(&["rm", "-q", file]);
  repo.git(&["commit", "-q", "-m", &format!("(auth) {name}")]);
  repo.head()
}

/// The text of `file` at `rev`
fn file_at(repo: &TestRepo, rev: &str, file: &str) -> String {
  repo.git(&["show", &format!("{rev}:{file}")])
}

#[test]
fn a_fix_that_conflicts_takes_the_files_of_what_landed_it_the_next_fix_applies_and_a_repeat_adds_nothing() {
  let (repo, tip) = landed_review();
  // the target changed auth after the landing, the fix is of that change: its lines conflict with those of the review
  change_target(&repo, "reworked\n");
  let fix = fix_on_master(&repo, "fix", "reworked 2\n");
  let landed = repo.land_copy_on_origin(&fix);
  let next = fix_on_master(&repo, "next", "reworked 3\n");
  repo.land_copy_on_origin(&next);

  let extended = extend(&repo, &[fix.clone(), next.clone()]);
  assert_eq!(
    extended.fixes,
    [
      FixReplay::Applied {
        taken: vec![TakenFiles {
          commit: fix.clone(),
          landed,
          paths: vec!["auth.txt".to_owned()],
        }],
      },
      FixReplay::Applied { taken: Vec::new() },
    ]
  );
  assert_eq!(on_origin(&repo).head, Some(extended.head.clone()));
  // the fix with the file as master has it after it, by its author, then the next fix on top of it
  let taken = repo.git(&["rev-parse", &format!("{}^", extended.head)]);
  assert_eq!(repo.git(&["rev-parse", &format!("{taken}^")]), tip);
  assert_eq!(
    repo.git(&["show", "-s", "--format=%s|%an|%at", &taken]),
    repo.git(&["show", "-s", "--format=%s|%an|%at", &fix])
  );
  assert_eq!(file_at(&repo, &taken, "auth.txt"), "reworked 2");
  assert_eq!(file_at(&repo, &extended.head, "auth.txt"), "reworked 3");
  // no temporary ref is left
  assert_eq!(repo.git(&["for-each-ref", "refs/branch-deck/"]), "");

  // a repeat (a crash after the push): the head has both, nothing is pushed
  let again = extend(&repo, &[fix, next]);
  assert_eq!((again.old_head.as_str(), again.head.as_str()), (extended.head.as_str(), extended.head.as_str()));
  assert_eq!(again.fixes, [FixReplay::Applied { taken: Vec::new() }, FixReplay::Applied { taken: Vec::new() }]);
}

#[test]
fn a_fix_that_conflicts_finds_what_landed_it_after_the_last_fetch_by_a_fetch_of_the_target() {
  let (repo, tip) = landed_review();
  change_target(&repo, "reworked\n");
  let fix = fix_on_master(&repo, "fix", "reworked 2\n");
  let fetched = repo.git(&["rev-parse", "refs/remotes/origin/master"]);
  let landed = repo.land_copy_on_origin(&fix);
  // the robot merged, the repository has not fetched the target since
  repo.git(&["update-ref", "refs/remotes/origin/master", &fetched]);

  let extended = extend(&repo, std::slice::from_ref(&fix));
  assert_eq!(
    extended.fixes,
    [FixReplay::Applied {
      taken: vec![TakenFiles {
        commit: fix,
        landed: landed.clone(),
        paths: vec!["auth.txt".to_owned()],
      }],
    }]
  );
  assert_eq!(repo.git(&["rev-parse", "refs/remotes/origin/master"]), landed);
  assert_eq!(repo.git(&["rev-parse", &format!("{}^", extended.head)]), tip);
  assert_eq!(file_at(&repo, &extended.head, "auth.txt"), "reworked 2");
}

#[test]
fn a_fix_that_master_has_under_no_identity_of_its_commit_does_not_apply_and_the_next_fix_still_does() {
  let (repo, tip) = landed_review();
  change_target(&repo, "reworked\n");
  let fix = fix_on_master(&repo, "fix", "reworked 2\n");
  // it landed under another subject (an autosquash folded it, a reword renamed it)
  repo.git(&["checkout", "-q", "-B", "landing", "origin/master"]);
  repo.create_commit("Rework auth once more", "auth.txt", "reworked 2\n");
  repo.git(&["push", "-q", "origin", "HEAD:master"]);
  repo.git(&["fetch", "-q", "origin"]);
  repo.git(&["checkout", "-q", "-B", "next", "origin/master"]);
  let next = repo.create_commit("(auth) next", "login.txt", "login\n");
  repo.land_copy_on_origin(&next);

  let extended = extend(&repo, &[fix.clone(), next]);
  assert_eq!(extended.fixes, [FixReplay::NotApplied { commit: fix }, FixReplay::Applied { taken: Vec::new() }]);
  // the head stays where the fix found it, the next fix goes on from there
  assert_eq!(repo.git(&["rev-parse", &format!("{}^", extended.head)]), tip);
  assert_eq!(file_at(&repo, &extended.head, "auth.txt"), "auth");
  assert_eq!(file_at(&repo, &extended.head, "login.txt"), "login");
  assert_eq!(on_origin(&repo).head, Some(extended.head));

  // the only fix doesn't apply: nothing is pushed
  let (repo, tip) = landed_review();
  change_target(&repo, "reworked\n");
  let fix = fix_on_master(&repo, "fix", "reworked 2\n");
  repo.git(&["checkout", "-q", "-B", "landing", "origin/master"]);
  repo.create_commit("Rework auth once more", "auth.txt", "reworked 2\n");
  repo.git(&["push", "-q", "origin", "HEAD:master"]);
  repo.git(&["fetch", "-q", "origin"]);
  let extended = extend(&repo, std::slice::from_ref(&fix));
  assert_eq!(
    (extended.head.as_str(), extended.fixes.as_slice()),
    (tip.as_str(), [FixReplay::NotApplied { commit: fix }].as_slice())
  );
  assert_eq!(on_origin(&repo).head, Some(tip));
}

#[test]
fn a_conflicting_commit_takes_a_rename_and_a_delete_as_master_landed_them() {
  // the fix renames the file master changed after the landing: the review shows it renamed, as master has it
  let (repo, tip) = landed_review();
  let lines = "reworked\nline 2\nline 3\nline 4\nline 5\nline 6\n";
  change_target(&repo, lines);
  repo.git(&["checkout", "-q", "-B", "rename", "origin/master"]);
  repo.git(&["mv", "auth.txt", "login.txt"]);
  let rename = repo.create_commit("(auth) rename", "login.txt", &lines.replace("reworked", "reworked 2"));
  let landed = repo.land_copy_on_origin(&rename);
  let extended = extend(&repo, std::slice::from_ref(&rename));
  assert_eq!(
    extended.fixes,
    [FixReplay::Applied {
      taken: vec![TakenFiles {
        commit: rename,
        landed: landed.clone(),
        paths: vec!["login.txt".to_owned()],
      }],
    }]
  );
  assert_eq!(repo.git(&["rev-parse", &format!("{}^", extended.head)]), tip);
  assert_eq!(repo.git(&["ls-tree", "--name-only", &extended.head]), repo.git(&["ls-tree", "--name-only", &landed]));
  assert_eq!(file_at(&repo, &extended.head, "login.txt"), file_at(&repo, &landed, "login.txt"));

  // the fix deletes the file master changed after the landing: the review shows it deleted, and names no file
  let (repo, tip) = landed_review();
  change_target(&repo, "reworked\n");
  let delete = delete_on_master(&repo, "drop", "auth.txt");
  repo.land_copy_on_origin(&delete);
  let extended = extend(&repo, std::slice::from_ref(&delete));
  assert_eq!(extended.fixes, [FixReplay::Applied { taken: Vec::new() }]);
  assert_eq!(repo.git(&["rev-parse", &format!("{}^", extended.head)]), tip);
  assert_eq!(repo.git(&["ls-tree", "--name-only", &extended.head]), "README.md");
  assert_eq!(on_origin(&repo).head, Some(extended.head));
}

#[cfg(any(unix, windows))]
#[test]
fn a_fix_on_the_head_is_signed_as_commit_gpgsign_says() {
  #[cfg(windows)]
  if sync_utils::child_process::windowless_command("ssh-keygen").arg("-?").output().is_err() {
    return;
  }
  let (repo, _) = landed_review();
  let fix = fix_on_master(&repo, "fix", "auth 2\n");
  repo.land_copy_on_origin(&fix);
  repo.enable_ssh_signing();
  let path = repo.path().to_str().unwrap();
  let signing = git_ops::commit_signing::commit_signing(repo.git_executor(), path).unwrap();
  let extended = extend_review(repo.git_executor(), path, "origin", "test", "auth", fixes(&[fix], signing)).unwrap();
  assert_eq!(on_origin(&repo).head, Some(extended.head.clone()));
  assert!(repo.git(&["cat-file", "commit", &extended.head]).contains("gpgsig"));
  // no temporary ref is left
  assert_eq!(repo.git(&["for-each-ref", "refs/branch-deck/"]), "");
}

#[test]
fn a_head_the_repository_lacks_is_fetched_first() {
  let (repo, _) = landed_review();
  // a clone of the remote before the review took a fix: it never sees the head that took it
  let clone = TestRepo::new_empty();
  clone.clone_from(&repo.path().join(".git/origin.git")).unwrap();
  let fix = fix_on_master(&repo, "fix", "auth 2\n");
  repo.land_copy_on_origin(&fix);
  let head = extend(&repo, std::slice::from_ref(&fix)).head;

  // the next fix lands from the clone
  clone.git(&["fetch", "-q", "origin", "master"]);
  clone.git(&["checkout", "-q", "-B", "second", "origin/master"]);
  let second = clone.create_commit("(auth) second", "auth.txt", "auth 3\n");
  clone.land_copy_on_origin(&second);
  clone.rev_parse(&format!("{head}^{{commit}}")).unwrap_err();
  let extended = extend_review(
    clone.git_executor(),
    clone.path().to_str().unwrap(),
    "origin",
    "test",
    "auth",
    fixes(&[second], Signing::Off),
  )
  .unwrap();
  assert_eq!((extended.old_head.as_str(), extended.fixes.as_slice()), (head.as_str(), [APPLIED].as_slice()));
  assert_eq!(on_origin(&repo).head, Some(extended.head.clone()));
  assert_eq!(clone.git(&["rev-parse", &format!("{}^", extended.head)]), head);
}

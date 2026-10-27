use crate::autosquash::{
  AutosquashPlan, FixupKind, PlannedFixup, PlannedGroup, SquashFixupsParams, build_fixup_message, get_autosquash_plan_core, plan_autosquash, squash_fixup_commits_core,
};
use crate::commit_grouper::GroupingInputs;
use crate::repository_lock::RewriteOptions;
use crate::test_commit::commit;
use domain_types::commit::Commit;
use domain_types::rewrite::{AutosquashScope, OrphanFixup, SquashFixupsResult};
use git_ops::rewrite_guard::HeadMoved;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// Commits with the ids `c0`, `c1`, … oldest first
fn commits(messages: &[&str]) -> Vec<Commit> {
  messages.iter().enumerate().map(|(index, message)| commit(&format!("c{index}"), message)).collect()
}

/// `target <- fixup, fixup` per group, by id, then the orphans
fn summary(plan: &AutosquashPlan) -> (Vec<String>, Vec<&str>) {
  let groups = plan
    .groups
    .iter()
    .map(|group| {
      let folded: Vec<&str> = group.folded.iter().map(|fixup| fixup.commit_id.as_str()).collect();
      format!("{} <- {}", group.target, folded.join(", "))
    })
    .collect();
  let orphans = plan.orphans.iter().map(|orphan| orphan.commit_id.as_str()).collect();
  (groups, orphans)
}

fn plan(messages: &[&str]) -> (Vec<String>, Vec<String>) {
  let plan = plan_autosquash("c-head".to_string(), &commits(messages), &GroupingInputs::with_stacks(false));
  let (groups, orphans) = summary(&plan);
  (groups, orphans.into_iter().map(str::to_string).collect())
}

fn groups(list: &[&str]) -> Vec<String> {
  list.iter().map(|it| (*it).to_string()).collect()
}

#[test]
fn matches_the_exact_subject_and_keeps_the_details() {
  let plan = plan_autosquash(
    "head".to_string(),
    &commits(&[
      "(auth) Add login",
      "(docs) Document",
      "fixup! (auth) Add login",
      "squash! (docs) Document\n\nMore",
      "amend! (auth) Add login\n\n(auth) Log in",
    ]),
    &GroupingInputs::with_stacks(false),
  );
  assert_eq!(
    plan,
    AutosquashPlan {
      head: "head".to_string(),
      groups: vec![
        PlannedGroup {
          target: "c0".to_string(),
          target_subject: "(auth) Add login".to_string(),
          branch: Some("auth".to_string()),
          folded: vec![
            PlannedFixup {
              commit_id: "c2".to_string(),
              subject: "fixup! (auth) Add login".to_string(),
              kind: FixupKind::Fixup,
            },
            PlannedFixup {
              commit_id: "c4".to_string(),
              subject: "amend! (auth) Add login".to_string(),
              kind: FixupKind::Amend,
            },
          ],
        },
        PlannedGroup {
          target: "c1".to_string(),
          target_subject: "(docs) Document".to_string(),
          branch: Some("docs".to_string()),
          folded: vec![PlannedFixup {
            commit_id: "c3".to_string(),
            subject: "squash! (docs) Document".to_string(),
            kind: FixupKind::Squash,
          }],
        },
      ],
      orphans: vec![],
    }
  );
}

#[test]
fn matches_the_subject_without_the_prefix() {
  assert_eq!(plan(&["(auth) Add login", "fixup! Add login"]), (groups(&["c0 <- c1"]), vec![]));
  // the prefixes must agree when the fixup names one
  assert_eq!(plan(&["(auth) Add login", "fixup! (docs) Add login"]), (vec![], groups(&["c1"])));
  // an issue key is part of the subject, not a prefix
  assert_eq!(plan(&["ABC-1: Add login", "fixup! ABC-1: Add login"]), (groups(&["c0 <- c1"]), vec![]));
}

#[test]
fn matches_a_unique_hash_prefix() {
  let commits = vec![
    commit("abcd1234", "Add a"),
    commit("abcd5678", "Add b"),
    commit("ffff0000", "fixup! abcd5"),
    commit("ffff1111", "fixup! ABCD1"),
    // ambiguous among the commits of the range
    commit("ffff2222", "fixup! abcd"),
    // too short
    commit("ffff3333", "fixup! abc"),
  ];
  let plan = plan_autosquash("head".to_string(), &commits, &GroupingInputs::with_stacks(false));
  assert_eq!(summary(&plan), (groups(&["abcd1234 <- ffff1111", "abcd5678 <- ffff0000"]), vec!["ffff2222", "ffff3333"]));
}

#[test]
fn falls_back_to_a_subject_starting_with_the_text() {
  assert_eq!(
    plan(&["Add login form", "(auth) Add logout", "fixup! Add login", "fixup! Add log"]),
    (groups(&["c0 <- c2, c3"]), vec![])
  );
  assert_eq!(plan(&["(auth) Add logout", "fixup! (auth) Add log"]), (groups(&["c0 <- c1"]), vec![]));
}

#[test]
fn strips_every_marker_and_takes_the_kind_from_the_first() {
  let plan = plan_autosquash(
    "head".to_string(),
    &commits(&["Add a", "fixup! fixup! Add a", "squash!  amend! Add a"]),
    &GroupingInputs::with_stacks(false),
  );
  let kinds: Vec<FixupKind> = plan.groups[0].folded.iter().map(|fixup| fixup.kind).collect();
  assert_eq!(summary(&plan), (groups(&["c0 <- c1, c2"]), vec![]));
  assert_eq!(kinds, [FixupKind::Fixup, FixupKind::Squash]);
}

#[test]
fn resolves_a_fixup_of_a_fixup_to_the_final_target() {
  let commits = vec![commit("aaaa0000", "Add a"), commit("bbbb0000", "fixup! Add a"), commit("cccc0000", "fixup! bbbb0000")];
  assert_eq!(
    summary(&plan_autosquash("head".to_string(), &commits, &GroupingInputs::with_stacks(false))),
    (groups(&["aaaa0000 <- bbbb0000, cccc0000"]), vec![])
  );

  // a fixup of an orphan has no target either
  let commits = vec![commit("aaaa0000", "fixup! Gone"), commit("bbbb0000", "fixup! aaaa0000")];
  assert_eq!(
    summary(&plan_autosquash("head".to_string(), &commits, &GroupingInputs::with_stacks(false))),
    (vec![], vec!["aaaa0000", "bbbb0000"])
  );
}

#[test]
fn picks_the_oldest_of_duplicate_subjects() {
  assert_eq!(plan(&["Add a", "Add a", "fixup! Add a"]), (groups(&["c0 <- c2"]), vec![]));
}

#[test]
fn orphans_a_fixup_without_an_older_target() {
  // the target is newer than the fixup
  assert_eq!(plan(&["fixup! Add a", "Add a"]), (vec![], groups(&["c0"])));
  // the target is not in the range
  let outside = plan_autosquash("head".to_string(), &commits(&["Add b", "fixup! (auth) Add a"]), &GroupingInputs::with_stacks(false));
  assert_eq!(
    outside.orphans,
    [OrphanFixup {
      commit_id: "c1".to_string(),
      subject: "fixup! (auth) Add a".to_string(),
      branch: Some("auth".to_string()),
    }]
  );
  // an empty target text matches nothing
  assert_eq!(plan(&["Add a", "fixup! "]).1, groups(&["c1"]));
}

#[test]
fn needs_the_space_after_the_marker() {
  assert_eq!(plan(&["Add a", "fixup!Add a", "squash!Add a"]), (vec![], vec![]));
  // an ordinary subject that looks like a marker can be a target
  assert_eq!(plan(&["fixup!Add a", "fixup! fixup!Add a"]), (groups(&["c0 <- c1"]), vec![]));
}

#[test]
fn a_subject_without_a_marker_is_never_a_fixup() {
  // an issue key or a prefix is no marker, also when an older commit has the same subject or starts with it
  assert_eq!(
    plan(&["ABC-1 Fix x", "(auth) Add a", "ABC-1 Fix x", "(auth) Add a", "(auth) ABC-1 Fix x", "ABC-1 Fix x again"]),
    (vec![], vec![])
  );
  // a prefixed commit stays the target of a fixup that names it
  assert_eq!(plan(&["ABC-1 Fix x", "(auth) Add a", "fixup! (auth) Add a"]), (groups(&["c1 <- c2"]), vec![]));
}

#[test]
fn builds_the_message_like_git() {
  let target = "(auth) Add login\n\nThe form\n";
  assert_eq!(build_fixup_message(target, &[(FixupKind::Fixup, "fixup! (auth) Add login\n\nignored")]), None);
  // a squash appends its body, without the marker line
  assert_eq!(
    build_fixup_message(target, &[(FixupKind::Squash, "squash! (auth) Add login\n\n\nAlso validate\n")]),
    Some("(auth) Add login\n\nThe form\n\nAlso validate".to_string())
  );
  // an amend replaces the message with its body; a later squash appends to it
  assert_eq!(
    build_fixup_message(
      target,
      &[
        (FixupKind::Amend, "amend! (auth) Add login\n\n(auth) Log in\n\nWith a form"),
        (FixupKind::Fixup, "fixup! (auth) Add login"),
        (FixupKind::Squash, "squash! (auth) Add login\n\nValidate"),
      ]
    ),
    Some("(auth) Log in\n\nWith a form\n\nValidate".to_string())
  );
  // without a body, a squash or an amend keeps the message
  assert_eq!(build_fixup_message(target, &[(FixupKind::Squash, "squash! X"), (FixupKind::Amend, "amend! X\n\n")]), None);
}

// ---- on a repository ----

fn repo_path(test_repo: &TestRepo) -> String {
  test_repo.path().to_str().unwrap().to_string()
}

fn squash_fixups(test_repo: &TestRepo, expected_head: &str, scope: AutosquashScope) -> anyhow::Result<SquashFixupsResult> {
  let params = SquashFixupsParams {
    repository_path: repo_path(test_repo),
    expected_head: expected_head.to_string(),
    scope,
  };
  squash_fixup_commits_core(test_repo.git_executor(), &params, &RewriteOptions::default())
}

fn messages(test_repo: &TestRepo) -> String {
  test_repo.log(&["--format=%B--"]).unwrap().trim_end().to_string()
}

/// `Initial`, then `(a) Add a`, `(b) Add b` and their fixups interleaved
fn interleaved_repo() -> (TestRepo, String) {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial", "README.md", "readme");
  test_repo.create_commit("(a) Add a", "a.txt", "a");
  test_repo.create_commit("(b) Add b", "b.txt", "b");
  test_repo.create_commit("fixup! (a) Add a", "a.txt", "a fixed");
  test_repo.create_commit("squash! Add b\n\nAlso b2", "b2.txt", "b2");
  let head = test_repo.create_commit("fixup! (c) Gone", "c.txt", "c");
  (test_repo, head)
}

#[test]
fn squashes_interleaved_groups_in_one_rewrite() {
  let (test_repo, head) = interleaved_repo();
  let plan = get_autosquash_plan_core(test_repo.git_executor(), &repo_path(&test_repo)).unwrap();
  assert_eq!(plan.head, head);
  assert_eq!(plan.groups.len(), 2);

  let result = squash_fixups(&test_repo, &head, AutosquashScope::All).unwrap();

  assert_eq!(messages(&test_repo), "fixup! (c) Gone\n--\n(b) Add b\n\nAlso b2\n--\n(a) Add a\n--\nInitial\n--");
  assert_eq!(result.squashed_count, 2);
  assert_eq!(result.previous_head, head);
  assert_eq!(result.new_head, test_repo.head());
  assert_eq!(result.squashed.len(), 2);
  assert_eq!(result.squashed[0].new_id, test_repo.rev_parse("HEAD~2").unwrap());
  assert_eq!(result.squashed[1].new_id, test_repo.rev_parse("HEAD~1").unwrap());
  assert_eq!(result.orphans.iter().map(|orphan| orphan.subject.as_str()).collect::<Vec<_>>(), ["fixup! (c) Gone"]);
  assert_eq!(test_repo.rev_parse("HEAD~2:a.txt").unwrap(), test_repo.rev_parse(&format!("{head}:a.txt")).unwrap());
  let reflog = test_repo.log(&["-g", "-1", "--format=%gs"]).unwrap();
  assert_eq!(reflog.trim(), "branch-deck: squash fixups");
}

#[test]
fn squashes_only_the_fixups_of_one_branch_or_one_fixup() {
  let (test_repo, head) = interleaved_repo();
  let result = squash_fixups(&test_repo, &head, AutosquashScope::Branch("b".to_string())).unwrap();
  assert_eq!(result.squashed_count, 1);
  assert_eq!(result.orphans, vec![]);
  assert_eq!(
    test_repo.log(&["--format=%s"]).unwrap().trim_end(),
    "fixup! (c) Gone\nfixup! (a) Add a\n(b) Add b\n(a) Add a\nInitial"
  );

  // only that fixup, though `(a) Add a` has none other
  let fixup = test_repo.rev_parse("HEAD~1").unwrap();
  squash_fixups(&test_repo, &test_repo.head(), AutosquashScope::Fixup(fixup)).unwrap();
  assert_eq!(test_repo.log(&["--format=%s"]).unwrap().trim_end(), "fixup! (c) Gone\n(b) Add b\n(a) Add a\nInitial");

  // nothing left but an orphan
  let error = squash_fixups(&test_repo, &test_repo.head(), AutosquashScope::Branch("c".to_string())).expect_err("only an orphan");
  assert!(format!("{error:#}").contains("1 without a target"), "{error:#}");
  let error = squash_fixups(&test_repo, &test_repo.head(), AutosquashScope::Unassigned).expect_err("no unassigned fixups");
  assert!(format!("{error:#}").contains("No fixup commits"), "{error:#}");
}

#[test]
fn refuses_when_the_branch_moved_since_the_plan() {
  let (test_repo, head) = interleaved_repo();
  let later = test_repo.create_commit("Later", "later.txt", "later");
  let error = squash_fixups(&test_repo, &head, AutosquashScope::All).expect_err("the branch moved");
  let moved = error.downcast_ref::<HeadMoved>().unwrap_or_else(|| panic!("not HeadMoved: {error:#}"));
  assert_eq!((moved.expected.as_str(), moved.actual.as_str()), (head.as_str(), later.as_str()));
  assert_eq!(test_repo.head(), later);
}

#[test]
fn keeps_a_dirty_tree_untouched() {
  let (test_repo, head) = interleaved_repo();
  let path = test_repo.path();
  std::fs::write(path.join("a.txt"), "dirty a\n").unwrap();
  std::fs::write(path.join("new.txt"), "untracked\n").unwrap();
  std::fs::write(path.join("b.txt"), "staged b\n").unwrap();
  test_repo.git_executor().execute_command(&["add", "b.txt"], &repo_path(&test_repo)).unwrap();
  let status_before = test_repo.git_executor().execute_command(&["status", "--porcelain"], &repo_path(&test_repo)).unwrap();

  squash_fixups(&test_repo, &head, AutosquashScope::All).unwrap();

  let status_after = test_repo.git_executor().execute_command(&["status", "--porcelain"], &repo_path(&test_repo)).unwrap();
  assert_eq!(status_after, status_before);
  assert_eq!(std::fs::read_to_string(path.join("a.txt")).unwrap(), "dirty a\n");
  assert_eq!(std::fs::read_to_string(path.join("new.txt")).unwrap(), "untracked\n");
  let staged = test_repo.git_executor().execute_command(&["show", ":b.txt"], &repo_path(&test_repo)).unwrap();
  assert_eq!(staged.trim_end(), "staged b");
}

#[test]
fn refuses_to_combine_messages_in_another_encoding() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial", "README.md", "readme");
  test_repo.set_config("i18n.commitEncoding", "ISO-8859-1").unwrap();
  test_repo.create_commit("Add a", "a.txt", "a");
  test_repo.set_config("i18n.commitEncoding", "UTF-8").unwrap();
  let head = test_repo.create_commit("squash! Add a\n\nMore", "a.txt", "more");

  let error = squash_fixups(&test_repo, &head, AutosquashScope::All).expect_err("a Latin-1 message");
  assert!(format!("{error:#}").contains("encoding ISO-8859-1"), "{error:#}");
  assert_eq!(test_repo.head(), head);
}

fn stacked_plan(messages: &[&str]) -> AutosquashPlan {
  plan_autosquash("head".to_string(), &commits(messages), &GroupingInputs::with_stacks(true))
}

#[test]
fn with_stacks_a_fixup_names_the_leaf_of_its_target() {
  let plan = stacked_plan(&["(auth/login) Add form", "(docs) Add form", "fixup! (login) Add form", "fixup! (login) Add f"]);
  assert_eq!(summary(&plan), (groups(&["c0 <- c2, c3"]), vec![]));
  assert_eq!(plan.groups[0].branch.as_deref(), Some("login"));

  // the full stack prefix and a deeper declaration of the same leaf agree too
  assert_eq!(
    summary(&stacked_plan(&["(login) Add form", "fixup! (auth/login) Add form"])),
    (groups(&["c0 <- c1"]), vec![])
  );
  assert_eq!(
    summary(&stacked_plan(&["(auth/login/oauth) Add OAuth", "fixup! (oauth) Add OAuth"])),
    (groups(&["c0 <- c1"]), vec![])
  );
}

#[test]
fn with_stacks_another_leaf_does_not_agree() {
  let plan = stacked_plan(&["(auth/login) Add form", "fixup! (auth) Add form"]);
  assert_eq!(summary(&plan), (vec![], vec!["c1"]));
  assert_eq!(plan.orphans[0].branch.as_deref(), Some("auth"));
}

#[test]
fn without_stacks_prefixes_are_compared_as_text() {
  let plan = plan_autosquash(
    "head".to_string(),
    &commits(&["(auth/login) Add form", "fixup! (login) Add form"]),
    &GroupingInputs::with_stacks(false),
  );
  assert_eq!(summary(&plan), (vec![], vec!["c1"]));
  assert_eq!(plan.orphans[0].branch.as_deref(), Some("login"));
  let plan = plan_autosquash(
    "head".to_string(),
    &commits(&["(auth/login) Add form", "fixup! (auth/login) Add form"]),
    &GroupingInputs::with_stacks(false),
  );
  assert_eq!(plan.groups[0].branch.as_deref(), Some("auth/login"));
}

#[test]
fn the_plan_reads_the_stacks_setting_of_the_repository() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial", "README.md", "# Test");
  test_repo.create_commit("(auth/login) Add form", "form.txt", "form");
  test_repo.create_commit("fixup! (login) Add form", "form.txt", "fixed form");

  let plan = get_autosquash_plan_core(test_repo.git_executor(), &repo_path(&test_repo)).unwrap();
  assert_eq!(plan.groups, []);
  assert_eq!(plan.orphans.len(), 1);

  test_repo.set_config("branchdeck.stacks", "true").unwrap();
  let plan = get_autosquash_plan_core(test_repo.git_executor(), &repo_path(&test_repo)).unwrap();
  assert_eq!(plan.groups.len(), 1);
  assert_eq!(plan.groups[0].branch.as_deref(), Some("login"));
  assert_eq!(plan.orphans, []);
}

use crate::landed_by_lines::squashes_of_gone;
use crate::landed_pairing::{
  CandidateBy, CommitKey, Interdiff, LandedCandidate, LandedDrop, LandedPairing, LandedProof, LandedScope, PathLines, interdiffs, pair_after_conflict, pair_before_replay,
  pair_landed, pair_twins, read_landed, read_landed_with_paths, walk_commits,
};
use pretty_assertions::assert_eq;
use std::collections::{HashMap, HashSet};
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use test_utils::history::{HistoryCommit, numbered_lines};
use test_utils::process_budget::{assert_processes, counting, processes};

/// `(mark, id, author, key, subject)` of each commit
fn fields(repo: &TestRepo, options: &[&str], revs: &[&str]) -> Vec<(char, String, String, CommitKey, String)> {
  walk_commits(repo.git_executor(), repo.path_str(), options, revs)
    .unwrap()
    .into_iter()
    .map(|commit| (commit.mark, commit.id, commit.author, commit.key, commit.subject))
    .collect()
}

#[test]
fn a_walk_reads_the_mark_the_author_and_the_identity_of_each_commit() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_timestamp("Initial commit", "a.txt", "a\n", Some(1_700_000_000));
  repo.checkout_new_branch("feature").unwrap();
  let login = repo.create_commit_with_timestamp("(auth) Add login", "login.txt", "login\n", Some(1_700_000_100));
  repo.checkout("master").unwrap();
  repo.git(&["cherry-pick", &login]);
  let copy = repo.head();
  let key = CommitKey::from_subject("test@example.com", 1_700_000_100, "Add login");

  // without `--left-right` every commit is `>`
  assert_eq!(
    fields(&repo, &[], &[&format!("^{base}"), "feature"]),
    [('>', login.clone(), "Test User".to_string(), key, "(auth) Add login".to_string())]
  );
  // a copy on the other side is `=`
  let mut marked: Vec<(char, String)> = fields(&repo, &["--left-right", "--cherry-mark"], &["master...feature"])
    .into_iter()
    .map(|(mark, id, ..)| (mark, id))
    .collect();
  marked.sort();
  let mut expected = [('=', login), ('=', copy)];
  expected.sort();
  assert_eq!(marked, expected);

  // a rev that names a file too is a rev: the walk ends its revs with `--`
  repo.create_commit("Add a file named feature", "feature", "feature\n");
  assert_eq!(fields(&repo, &[], &[&format!("^{base}"), "feature"]).len(), 1);
}

/// A commit of `f.txt` with `changes` at `time`: two commits of one subject at one time share a `CommitKey`
fn at(message: &str, changes: &[(usize, &str)], time: i64) -> HistoryCommit {
  HistoryCommit::new(message, &[("f.txt", &numbered_lines(changes))]).at(time)
}

/// A repository whose `master` has `f.txt`, with the branches `upstream` and `local` at it, `local` checked out
fn sides() -> TestRepo {
  let (repo, _) = TestRepo::with_history(&[HistoryCommit::new("Initial", &[("f.txt", &numbered_lines(&[])), ("bin.dat", "\0one")])]);
  repo.git(&["branch", "upstream"]);
  repo.git(&["checkout", "-q", "-b", "local"]);
  repo
}

/// The pairing of `local` with `upstream`: all of it, after a conflict too (`pair_landed`), and the pairing before a
/// replay (`pair_before_replay`)
fn pairings(repo: &TestRepo) -> (LandedPairing, LandedPairing) {
  in_scope(repo, |scope| {
    let reads = read_landed(repo.git_executor(), repo.path_str(), scope).unwrap();
    let before = pair_before_replay(repo.git_executor(), repo.path_str(), &reads).unwrap();
    let all = pair_after_conflict(repo.git_executor(), repo.path_str(), scope, &reads, before.pairing.clone(), &before.paths).unwrap();
    assert_eq!(all, pair_landed(repo.git_executor(), repo.path_str(), scope).unwrap());
    (all, before.pairing)
  })
}

/// A commit of `f.txt` with `changes` on `upstream` by another author, `Other <other@example.com>`
fn commit_by_another_author(repo: &TestRepo, message: &str, changes: &[(usize, &str)]) -> String {
  let git = |args: &[&str], stdin: &str| repo.git_executor().cmd(args, repo.path_str()).stdin(stdin.as_bytes()).text().unwrap();
  let file = git(&["hash-object", "-w", "--stdin"], &numbered_lines(changes));
  let binary = git(&["rev-parse", "upstream:bin.dat"], "");
  let tree = git(&["mktree"], &format!("100644 blob {binary}\tbin.dat\n100644 blob {file}\tf.txt\n"));
  let env = [("GIT_AUTHOR_NAME", "Other"), ("GIT_AUTHOR_EMAIL", "other@example.com")];
  let commit = repo
    .git_executor()
    .cmd(&["commit-tree", &tree, "-p", "upstream", "-m", message], repo.path_str())
    .env(&env)
    .text()
    .unwrap();
  git(&["update-ref", "refs/heads/upstream", &commit], "");
  commit
}

/// `f` of the scope of `local` onto `upstream`
fn in_scope<R>(repo: &TestRepo, f: impl FnOnce(LandedScope<'_>) -> R) -> R {
  let head = repo.rev_parse("local").unwrap();
  let upstream_head = repo.rev_parse("upstream").unwrap();
  let base = repo.git(&["merge-base", &head, &upstream_head]);
  let scope = LandedScope {
    head: &head,
    upstream_head: &upstream_head,
    base: &base,
    excluded: &[],
  };
  f(scope)
}

fn pairing(repo: &TestRepo) -> LandedPairing {
  pairings(repo).0
}

/// The local commit whose twins the tests land
fn change_line_5() -> HistoryCommit {
  at("(auth) Change line 5", &[(5, "five")], 200)
}

/// The upstream twin of `change_line_5` as `subject`, after an upstream change of `line`: line 2 leaves the twin's
/// cherry-pick clean, line 4, next to it, makes it conflict. The twin also writes `files`. The ids of both commits
fn twin_after(repo: &TestRepo, line: (usize, &str), subject: &str, files: &[(&str, &str)]) -> Vec<String> {
  let mut twin = at(subject, &[line, (5, "five")], 200);
  twin.files.extend(files.iter().map(|(path, content)| ((*path).to_string(), (*content).to_string())));
  repo.commit_history("refs/heads/upstream", &[at("Other change", &[line], 100), twin])
}

fn drop_of(commit: &str, upstream: &str, proof: LandedProof) -> LandedDrop {
  LandedDrop {
    commit: commit.to_string(),
    upstream: upstream.to_string(),
    proof,
  }
}

/// The twin of a local commit landed after another upstream commit that changed a line of its context: the patch id
/// with context differs, the cherry-pick onto the twin's parent writes the twin's tree
#[test]
fn a_twin_with_other_context_is_proven_by_its_replay() {
  let repo = sides();
  let local = repo.commit_history("refs/heads/local", &[change_line_5()]);
  let upstream = twin_after(&repo, (2, "two"), "Change line 5", &[]);
  let pairing = pairing(&repo);
  assert_eq!(pairing.drops, [drop_of(&local[0], &upstream[1], LandedProof::Replayed)]);
  assert_eq!(pairing.candidates, []);
}

/// Two twins that landed in the other order on one file: each replays onto its own upstream parent
#[test]
fn twins_landed_in_the_other_order_are_both_proven_by_their_replay() {
  let repo = sides();
  let local = repo.commit_history(
    "refs/heads/local",
    &[at("(auth) Change line 2", &[(2, "two")], 100), at("(auth) Change line 5", &[(2, "two"), (5, "five")], 200)],
  );
  let upstream = repo.commit_history(
    "refs/heads/upstream",
    &[at("Change line 5", &[(5, "five")], 200), at("Change line 2", &[(2, "two"), (5, "five")], 100)],
  );
  assert_eq!(
    pairing(&repo).drops,
    [
      drop_of(&local[0], &upstream[1], LandedProof::Replayed),
      drop_of(&local[1], &upstream[0], LandedProof::Replayed),
    ]
  );
}

/// The upstream changed the line next to the twin's change before it landed: the cherry-pick conflicts, the changed
/// lines, paths and modes are the same
#[test]
fn a_twin_whose_cherry_pick_conflicts_is_proven_by_its_changed_lines() {
  let repo = sides();
  let local = repo.commit_history("refs/heads/local", &[change_line_5()]);
  let upstream = twin_after(&repo, (4, "four"), "Change line 5", &[]);
  assert_eq!(pairing(&repo).drops, [drop_of(&local[0], &upstream[1], LandedProof::ChangedLines)]);
}

/// Equal changed lines next to another change of a mode or of a binary file prove nothing: no text diff shows them
#[test]
fn equal_changed_lines_beside_a_chmod_or_a_binary_edit_prove_nothing() {
  // the local twin also makes the file executable
  let repo = sides();
  std::fs::write(repo.path().join("f.txt"), numbered_lines(&[(5, "five")])).unwrap();
  repo.git(&["add", "f.txt"]);
  repo.git(&["update-index", "--chmod=+x", "f.txt"]);
  let date = [("GIT_AUTHOR_DATE", "@200 +0000"), ("GIT_COMMITTER_DATE", "@200 +0000")];
  let local = repo.commit_with(&["-m", "(auth) Change line 5"], &date);
  let upstream = twin_after(&repo, (4, "four"), "Change line 5", &[]);
  let paired = pairing(&repo);
  assert_eq!(paired.drops, []);
  let candidates: Vec<(&str, &str)> = paired.candidates.iter().map(|c| (c.commit.as_str(), c.upstream.as_str())).collect();
  assert_eq!(candidates, [(local.as_str(), upstream[1].as_str())]);

  // the twins change the binary file to different content
  let repo = sides();
  let mut twin = change_line_5();
  twin.files.push(("bin.dat".to_string(), "\0local".to_string()));
  repo.commit_history("refs/heads/local", &[twin]);
  twin_after(&repo, (4, "four"), "Change line 5", &[("bin.dat", "\0upstream")]);
  let paired = pairing(&repo);
  assert_eq!((paired.drops.len(), paired.candidates.len()), (0, 1), "{paired:?}");
}

/// A, a revert of A and A again locally, one A upstream: one upstream commit proves one local commit, the oldest
#[test]
fn one_upstream_commit_proves_one_local_commit() {
  let repo = sides();
  let local = repo.commit_history(
    "refs/heads/local",
    &[
      at("(auth) Change line 5", &[(5, "five")], 100),
      at("(auth) Revert the change of line 5", &[], 200),
      at("(auth) Change line 5", &[(5, "five")], 300),
    ],
  );
  let upstream = repo.commit_history("refs/heads/upstream", &[at("Change line 5", &[(5, "five")], 100)]);
  assert_eq!(pairing(&repo).drops, [drop_of(&local[0], &upstream[0], LandedProof::PatchId)]);
}

/// A twin of the same patch is proven by its patch id before the replay too: the pairing of the twins compares the
/// patch ids of the pairs first
#[test]
fn a_twin_of_the_same_patch_is_proven_by_its_patch_id_before_the_replay() {
  let repo = sides();
  let local = repo.commit_history("refs/heads/local", &[change_line_5()]);
  let upstream = repo.commit_history("refs/heads/upstream", &[at("Change line 5", &[(5, "five")], 200)]);
  let (all, before) = pairings(&repo);
  assert_eq!(before.drops, [drop_of(&local[0], &upstream[0], LandedProof::PatchId)]);
  assert_eq!(all, before);
}

/// The same patch under another identity of the same author (the upstream reworded it, at another time) is no twin, but
/// its patch id proves it before the replay: the upstream commits of the author with its set of paths compare their
/// patch ids, one with the set of another file does not
#[test]
fn the_same_patch_under_another_identity_of_its_author_is_proven_before_the_replay() {
  let repo = sides();
  let local = repo.commit_history("refs/heads/local", &[change_line_5()]);
  let upstream = repo.commit_history(
    "refs/heads/upstream",
    &[HistoryCommit::new("Change bin", &[("bin.dat", "\0two")]).at(250), at("Fix line 5", &[(5, "five")], 300)],
  );
  let reads = in_scope(&repo, |scope| read_landed(repo.git_executor(), repo.path_str(), scope).unwrap());
  let twins = pair_twins(repo.git_executor(), repo.path_str(), &reads).unwrap();
  assert_eq!(twins, LandedPairing::unpaired(local.clone()));
  let (all, before) = pairings(&repo);
  assert_eq!(before.drops, [drop_of(&local[0], &upstream[1], LandedProof::PatchId)]);
  assert_eq!(all, before);
}

/// The same patch by another author is left to the walk of the patch ids of the whole upstream, after a conflict: the
/// pairing before the replay reads only the upstream commits of the local authors
#[test]
fn the_same_patch_by_another_author_is_proven_only_after_a_conflict() {
  let repo = sides();
  let local = repo.commit_history("refs/heads/local", &[change_line_5()]);
  let upstream = commit_by_another_author(&repo, "Fix line 5", &[(5, "five")]);
  let (all, before) = pairings(&repo);
  assert_eq!(before, LandedPairing::unpaired(local.clone()));
  assert_eq!(all.drops, [drop_of(&local[0], &upstream, LandedProof::PatchId)]);
}

/// Before the replay, a local commit compares its patch id only with the upstream commits of its author whose set of
/// paths it has: one `diff-tree --name-only` of both sides, no patch id for another set
#[test]
fn the_same_patch_of_the_author_is_looked_for_only_among_its_sets_of_paths() {
  let repo = sides();
  repo.commit_history("refs/heads/local", &[change_line_5()]);
  repo.commit_history("refs/heads/upstream", &[HistoryCommit::new("Change bin", &[("bin.dat", "\0two")]).at(250)]);
  let reads = in_scope(&repo, |scope| read_landed(repo.git_executor(), repo.path_str(), scope).unwrap());
  let git = counting();
  let before = pair_before_replay(&git, repo.path_str(), &reads).unwrap();
  assert_eq!(before.pairing, LandedPairing::unpaired(vec![repo.rev_parse("local").unwrap()]));
  assert_eq!(processes(&git), [("diff-tree".to_string(), 1)]);
}

/// The twins are paired before every replay: without a commit of the same identity, no process
#[test]
fn the_pairing_of_the_twins_runs_no_process_without_a_twin() {
  let repo = sides();
  repo.commit_history("refs/heads/local", &[change_line_5()]);
  repo.commit_history("refs/heads/upstream", &[at("Other change", &[(2, "two")], 100)]);
  let reads = in_scope(&repo, |scope| read_landed(repo.git_executor(), repo.path_str(), scope).unwrap());
  let git = counting();
  let twins = pair_twins(&git, repo.path_str(), &reads).unwrap();
  assert_eq!(twins, LandedPairing::unpaired(vec![repo.rev_parse("local").unwrap()]));
  assert_eq!(processes(&git), []);
}

/// The same changed lines under another subject are another commit before the replay; after a conflict they make a
/// candidate, never a drop
#[test]
fn the_same_lines_under_another_identity_are_no_proof_but_a_candidate_after_a_conflict() {
  let repo = sides();
  let local = repo.commit_history("refs/heads/local", &[change_line_5()]);
  let upstream = twin_after(&repo, (4, "four"), "Fix line 5", &[]);
  let (all, before) = pairings(&repo);
  assert_eq!(before, LandedPairing::unpaired(local.clone()));
  assert_eq!(all.drops, []);
  assert_eq!(all.candidates, [by_lines(&local[0], &[], &upstream[1])]);
}

/// A commit amended after its push keeps its identity, not its change: a candidate with its fixup, never a drop
#[test]
fn an_amend_after_the_push_is_a_candidate_with_its_fixups() {
  let repo = sides();
  let local = repo.commit_history(
    "refs/heads/local",
    &[
      at("(auth) Change line 5", &[(5, "five, amended")], 200),
      at("(auth) Add line 8", &[(5, "five, amended"), (8, "eight")], 250),
      at("fixup! (auth) Change line 5", &[(5, "five, fixed"), (8, "eight")], 300),
    ],
  );
  let upstream = repo.commit_history("refs/heads/upstream", &[at("Change line 5", &[(5, "five")], 200)]);
  let pairing = pairing(&repo);
  assert_eq!(pairing.drops, []);
  assert_eq!(
    pairing.candidates,
    [LandedCandidate {
      commit: local[0].clone(),
      upstream: upstream[0].clone(),
      landed_with: vec![local[2].clone()],
      by: CandidateBy::Key,
    }]
  );
  assert_eq!(pairing.likely_landed(), [local[0].clone(), local[2].clone()]);
  assert_eq!(pairing.candidate_sets(), [vec![local[0].clone(), local[2].clone()], vec![local[0].clone()]]);
}

/// A fixup made after a proven twin landed is no candidate: it replays onto the twin on its own
#[test]
fn a_fixup_of_a_proven_twin_stays_out_of_the_pairing() {
  let repo = sides();
  let local = repo.commit_history("refs/heads/local", &[change_line_5(), at("fixup! (auth) Change line 5", &[(5, "five, fixed")], 300)]);
  let upstream = twin_after(&repo, (2, "two"), "Change line 5", &[]);
  let pairing = pairing(&repo);
  assert_eq!(pairing.drops, [drop_of(&local[0], &upstream[1], LandedProof::Replayed)]);
  assert_eq!((pairing.candidates.len(), pairing.likely_landed().len()), (0, 0), "{pairing:?}");
}

/// The interdiff of a twin sums the local commit with its fixups: a line the fixup takes back cancels. What one side
/// changes and the other does not stays, also a binary file, which no line shows
#[test]
fn the_interdiff_of_a_twin_sums_the_fixups_and_names_a_binary_file() {
  let repo = sides();
  let local = repo.commit_history(
    "refs/heads/local",
    &[
      at("(auth) Change line 5", &[(5, "five, amended")], 200),
      HistoryCommit::new(
        "fixup! (auth) Change line 5",
        &[("f.txt", &numbered_lines(&[(5, "five, fixed"), (8, "eight")])), ("bin.dat", "\0two")],
      )
      .at(300),
    ],
  );
  let upstream = repo.commit_history("refs/heads/upstream", &[at("Change line 5", &[(5, "five, fixed"), (8, "eight"), (9, "nine")], 200)]);
  let diffs = interdiffs(repo.git_executor(), repo.path_str(), &[(local.iter().map(String::as_str).collect(), upstream[0].as_str())]).unwrap();
  let blob = repo.rev_parse(&format!("{}:bin.dat", local[1])).unwrap();
  assert_eq!(
    diffs,
    [Interdiff {
      local_only: vec![PathLines {
        path: "bin.dat".to_string(),
        lines: vec![format!("binary {blob}")],
      }],
      upstream_only: vec![PathLines {
        path: "f.txt".to_string(),
        lines: vec!["-9".to_string(), "+nine".to_string()],
      }],
    }]
  );
  // the commit alone has a line the twin lacks, and lacks the fixup's
  let diffs = interdiffs(repo.git_executor(), repo.path_str(), &[(vec![local[0].as_str()], upstream[0].as_str())]).unwrap();
  assert_eq!(
    diffs[0].local_only,
    [PathLines {
      path: "f.txt".to_string(),
      lines: vec!["+five, amended".to_string()],
    }]
  );
}

#[test]
fn interdiffs_compare_multiple_local_commits_to_the_same_upstream() {
  let repo = sides();
  let local = repo.commit_history(
    "refs/heads/local",
    &[
      HistoryCommit::new("fixup! (lane) X", &[("f.txt", &numbered_lines(&[(5, "first")]))]).at(200),
      HistoryCommit::new("fixup! (lane) X", &[("g.txt", "second\n")]).at(300),
    ],
  );
  let upstream = repo.commit_history(
    "refs/heads/upstream",
    &[HistoryCommit::new("(lane) X", &[("f.txt", &numbered_lines(&[(5, "first")])), ("g.txt", "second\n")]).at(400)],
  );
  let pairs = [(vec![local[0].as_str()], upstream[0].as_str()), (vec![local[1].as_str()], upstream[0].as_str())];
  let diffs = interdiffs(repo.git_executor(), repo.path_str(), &pairs).unwrap();
  assert_eq!(diffs.len(), 2);
  assert!(diffs.iter().all(|diff| diff.local_only.is_empty()), "{diffs:?}");
}

/// The log of the local commits of the pairing names every path they change, a rename as its two paths, from the same
/// process: the files of an update whose replay can lose a binding (`UpdatePlan::check_lost_bindings`)
#[test]
fn the_local_commits_come_with_the_paths_they_change() {
  let repo = TestRepo::new();
  let base = repo.create_commit("base", "old name.kt", "a\n");
  repo.git(&["mv", "old name.kt", "new name.kt"]);
  repo.create_commit_with_files("rename and add", &[("new name.kt", "a\n"), ("src/b.kt", "b\n")]);
  repo.git(&["commit", "--allow-empty", "-m", "nothing"]);
  repo.create_commit("change", "src/b.kt", "b2\n");
  let git = counting();
  let scope = LandedScope {
    head: &repo.head(),
    upstream_head: &base,
    base: &base,
    excluded: &[],
  };
  let (reads, paths) = read_landed_with_paths(&git, repo.path_str(), scope).unwrap();
  assert_eq!(
    reads.local.iter().map(|commit| commit.subject.as_str()).collect::<Vec<_>>(),
    ["rename and add", "nothing", "change"]
  );
  let mut paths: Vec<String> = paths.into_iter().collect();
  paths.sort();
  assert_eq!(paths, ["new name.kt", "old name.kt", "src/b.kt"]);
  assert_processes(&git, &[("log", 2)]);
}

/// A candidate by changed lines of `commit` with the commits it `landed_with` as `upstream`
fn by_lines(commit: &str, landed_with: &[&str], upstream: &str) -> LandedCandidate {
  LandedCandidate {
    commit: commit.to_string(),
    upstream: upstream.to_string(),
    landed_with: landed_with.iter().map(|commit| (*commit).to_string()).collect(),
    by: CandidateBy::ChangedLines,
  }
}

/// `(feature) Change line 5` to `B1` on `local`, then `rework`, which rewrites it to `B2`
fn change_and_rework(repo: &TestRepo, rework: &str) -> Vec<String> {
  repo.commit_history("refs/heads/local", &[at("(feature) Change line 5", &[(5, "B1")], 200), at(rework, &[(5, "B2")], 300)])
}

/// The squash of a pull request on `upstream`, as GitHub writes it: authored at the merge, ` (#1)` after the title
fn squash(repo: &TestRepo, commit: HistoryCommit) -> String {
  repo.commit_history("refs/heads/upstream", &[commit]).remove(0)
}

/// The issue of #265: a commit and the fixup that rewrote its line, squashed at the merge under another identity. Neither
/// is a twin nor of the squash's patch, so the pairing before the replay has none; after the conflict the two are one
/// candidate of the squash
#[test]
fn a_commit_and_its_fixup_squashed_under_another_identity_are_one_candidate_after_a_conflict() {
  let repo = sides();
  let local = change_and_rework(&repo, "(feature) fixup! Change line 5");
  let upstream = squash(&repo, at("Change line 5 (#1)", &[(5, "B2")], 900));
  let (all, before) = pairings(&repo);
  assert_eq!(before, LandedPairing::unpaired(local.clone()));
  assert_eq!(all.candidates, [by_lines(&local[0], &[&local[1]], &upstream)]);
  assert_eq!(all.likely_landed(), local);
  assert_eq!(all.candidate_sets(), [local]);
}

/// A pull request whose later commit, no fixup, rewrote the line of an earlier one: the three are one candidate
#[test]
fn a_pull_request_squashed_under_another_identity_is_one_candidate_of_its_commits() {
  let repo = sides();
  let mut local = change_and_rework(&repo, "(feature) Rework line 5");
  local.extend(repo.commit_history("refs/heads/local", &[at("(feature) Add line 8", &[(5, "B2"), (8, "eight")], 350)]));
  let upstream = squash(&repo, at("Feature (#1)", &[(5, "B2"), (8, "eight")], 900));
  assert_eq!(pairing(&repo).candidates, [by_lines(&local[0], &[&local[1], &local[2]], &upstream)]);
}

/// A twin by identity that lacks what a later commit of its pull request changed: the later commit completes it, and
/// the candidate is one by changed lines
#[test]
fn a_key_candidate_takes_the_later_commit_that_completes_its_squash() {
  let repo = sides();
  let local = change_and_rework(&repo, "(feature) Rework line 5");
  let upstream = squash(&repo, at("Change line 5", &[(5, "B2")], 200));
  let (all, before) = pairings(&repo);
  assert_eq!(
    before.candidates,
    [LandedCandidate {
      commit: local[0].clone(),
      upstream: upstream.clone(),
      landed_with: Vec::new(),
      by: CandidateBy::Key,
    }]
  );
  assert_eq!(all.candidates, [by_lines(&local[0], &[&local[1]], &upstream)]);
}

/// A change and its revert in the middle of the pull request sum to nothing: the smallest set leaves them out
#[test]
fn a_change_and_its_revert_stay_out_of_the_candidate_of_a_squash() {
  let repo = sides();
  let local = repo.commit_history(
    "refs/heads/local",
    &[
      at("(feature) Change line 5", &[(5, "B1")], 200),
      at("(feature) Add line 8", &[(5, "B1"), (8, "eight")], 250),
      at("(feature) Revert \"Add line 8\"", &[(5, "B1")], 260),
      at("(feature) fixup! Change line 5", &[(5, "B2")], 300),
    ],
  );
  let upstream = squash(&repo, at("Change line 5 (#1)", &[(5, "B2")], 900));
  assert_eq!(pairing(&repo).candidates, [by_lines(&local[0], &[&local[3]], &upstream)]);
}

/// A local commit made after the merge rewrites the squashed line again: it changes a line of the squash, but the
/// smallest set leaves it out, and a fixup made after the merge too (a unit is no set)
#[test]
fn a_commit_made_after_the_squash_stays_out_of_its_candidate() {
  let repo = sides();
  let local = repo.commit_history(
    "refs/heads/local",
    &[
      at("(feature) Change line 5", &[(5, "B1")], 200),
      at("(feature) fixup! Change line 5", &[(5, "B2")], 300),
      at("(feature) fixup! Change line 5", &[(5, "B3")], 950),
      at("(feature) Change line 5 again", &[(5, "B4")], 960),
    ],
  );
  let upstream = squash(&repo, at("Change line 5 (#1)", &[(5, "B2")], 900));
  assert_eq!(pairing(&repo).candidates, [by_lines(&local[0], &[&local[1]], &upstream)]);
}

/// A line that a commit adds and a later one rewrites is no line of the squash: the later commit brings it into the set
#[test]
fn a_pure_addition_rewritten_by_a_later_commit_is_in_the_candidate_of_its_squash() {
  let repo = sides();
  let local = repo.commit_history(
    "refs/heads/local",
    &[
      HistoryCommit::new("(feature) Call foo", &[("g.txt", "foo(1)\n")]).at(200),
      HistoryCommit::new("(feature) Call foo with 2", &[("g.txt", "foo(2)\n")]).at(300),
    ],
  );
  let upstream = squash(&repo, HistoryCommit::new("Call foo (#1)", &[("g.txt", "foo(2)\n")]).at(900));
  assert_eq!(pairing(&repo).candidates, [by_lines(&local[0], &[&local[1]], &upstream)]);
}

/// A file the pull request added and deleted again is no path of the squash
#[test]
fn a_pull_request_that_added_and_deleted_a_file_is_one_candidate() {
  let repo = sides();
  let local = repo.commit_history(
    "refs/heads/local",
    &[
      HistoryCommit::new("(feature) Change line 5", &[("f.txt", &numbered_lines(&[(5, "B1")])), ("tmp.txt", "draft\n")]).at(200),
      HistoryCommit::new("(feature) Rework line 5", &[("f.txt", &numbered_lines(&[(5, "B2")]))])
        .deleting(&["tmp.txt"])
        .at(300),
    ],
  );
  let upstream = squash(&repo, at("Change line 5 (#1)", &[(5, "B2")], 900));
  assert_eq!(pairing(&repo).candidates, [by_lines(&local[0], &[&local[1]], &upstream)]);
}

/// A binary file updated twice: the sum keeps its last version, as the squash has it
#[test]
fn a_pull_request_that_updated_a_binary_file_twice_is_one_candidate() {
  let repo = sides();
  let local = repo.commit_history(
    "refs/heads/local",
    &[
      HistoryCommit::new("(feature) Change line 5", &[("f.txt", &numbered_lines(&[(5, "B1")])), ("bin.dat", "\0two")]).at(200),
      HistoryCommit::new("(feature) Rework line 5", &[("f.txt", &numbered_lines(&[(5, "B2")])), ("bin.dat", "\0three")]).at(300),
    ],
  );
  let upstream = squash(
    &repo,
    HistoryCommit::new("Change line 5 (#1)", &[("f.txt", &numbered_lines(&[(5, "B2")])), ("bin.dat", "\0three")]).at(900),
  );
  assert_eq!(pairing(&repo).candidates, [by_lines(&local[0], &[&local[1]], &upstream)]);
}

/// A change made twice locally (A, its revert, A again) with one squash of it upstream: the local commits
fn change_made_twice_squashed_once(repo: &TestRepo) -> Vec<String> {
  let local = repo.commit_history(
    "refs/heads/local",
    &[
      at("(feature) Change line 5", &[(5, "B")], 200),
      at("(feature) Revert \"Change line 5\"", &[], 250),
      at("(feature) Change line 5 once more", &[(5, "B")], 300),
    ],
  );
  squash(repo, at("Change line 5 (#1)", &[(5, "B")], 900));
  local
}

/// `change_made_twice_squashed_once`: either A alone sums to the squash, so neither is its candidate
#[test]
fn two_sets_equal_to_one_squash_make_no_candidate() {
  let repo = sides();
  change_made_twice_squashed_once(&repo);
  assert_eq!(pairing(&repo).candidates, []);
}

/// A squash with a line that no local commit changes, e.g. a suggestion of the review applied on GitHub: no set
#[test]
fn an_upstream_commit_with_a_line_no_set_has_pairs_with_nothing() {
  let repo = sides();
  change_and_rework(&repo, "(feature) fixup! Change line 5");
  squash(&repo, at("Change line 5 (#1)", &[(5, "B2"), (7, "suggested")], 900));
  assert_eq!(pairing(&repo).candidates, []);
}

/// After a conflict the candidates by changed lines read the changed lines of the commits of the targets in one
/// `diff-tree -p -U0`, and nothing when no target's paths are the local commits' (after the one `rev-list
/// --cherry-mark` of the same patch by any author)
#[test]
fn the_candidates_by_changed_lines_read_one_diff_and_none_without_a_set_of_paths() {
  let count = |squashed: HistoryCommit| {
    let repo = sides();
    change_and_rework(&repo, "(feature) fixup! Change line 5");
    squash(&repo, squashed);
    in_scope(&repo, |scope| {
      let reads = read_landed(repo.git_executor(), repo.path_str(), scope).unwrap();
      let before = pair_before_replay(repo.git_executor(), repo.path_str(), &reads).unwrap();
      let git = counting();
      pair_after_conflict(&git, repo.path_str(), scope, &reads, before.pairing, &before.paths).unwrap();
      processes(&git)
    })
  };
  assert_eq!(
    count(at("Change line 5 (#1)", &[(5, "B2")], 900)),
    [("diff-tree".to_string(), 1), ("rev-list".to_string(), 1)]
  );
  assert_eq!(count(HistoryCommit::new("Other file (#2)", &[("g.txt", "g\n")]).at(900)), [("rev-list".to_string(), 1)]);
}

/// The baseline commit each of the `gone` local commits landed as (`squashes_of_gone`) when `named` holds the other
/// baseline commits: the same with the sets of paths the check read and without them; the processes without them
fn squashes_naming(repo: &TestRepo, gone: &[String], named: &[&str]) -> (HashMap<String, String>, Vec<(String, u32)>) {
  in_scope(repo, |scope| {
    let reads = read_landed(repo.git_executor(), repo.path_str(), scope).unwrap();
    let before = pair_before_replay(repo.git_executor(), repo.path_str(), &reads).unwrap();
    let gone: Vec<&str> = gone.iter().map(String::as_str).collect();
    let named: HashSet<&str> = named.iter().copied().collect();
    let with_paths = squashes_of_gone(repo.git_executor(), repo.path_str(), &reads, &gone, &named, Some(&before.paths)).unwrap();
    let git = counting();
    let found = squashes_of_gone(&git, repo.path_str(), &reads, &gone, &named, None).unwrap();
    assert_eq!(found, with_paths);
    (found, processes(&git))
  })
}

/// `squashes_naming` with nothing named
fn squashes(repo: &TestRepo, gone: &[String]) -> HashMap<String, String> {
  squashes_naming(repo, gone, &[]).0
}

/// Each of `commits` as landed in `squash`
fn all_in(commits: &[String], squash: &str) -> HashMap<String, String> {
  commits.iter().map(|commit| (commit.clone(), squash.to_string())).collect()
}

/// A pull request of two commits of two files went empty in a clean replay onto its squash under another identity: both
/// landed as the squash, read with one `diff-tree --name-only` and one `diff-tree -p -U0`; a squash that something else
/// names is none of theirs
#[test]
fn the_gone_commits_that_sum_to_an_upstream_commit_of_their_author_name_it() {
  let repo = sides();
  let local = repo.commit_history(
    "refs/heads/local",
    &[
      at("(feature) Change line 5", &[(5, "B")], 200),
      HistoryCommit::new("(feature) Add g", &[("g.txt", "g\n")]).at(300),
    ],
  );
  let upstream = squash(
    &repo,
    HistoryCommit::new("Feature (#1)", &[("f.txt", &numbered_lines(&[(5, "B")])), ("g.txt", "g\n")]).at(900),
  );
  let (found, processes) = squashes_naming(&repo, &local, &[]);
  assert_eq!(found, all_in(&local, &upstream));
  assert_eq!(processes, [("diff-tree".to_string(), 2)]);
  assert_eq!(squashes_naming(&repo, &local, &[&upstream]).0, HashMap::new());
}

/// `change_made_twice_squashed_once`, all gone: either A alone sums to the squash, so neither names it
#[test]
fn two_sets_of_gone_commits_equal_to_one_squash_name_nothing() {
  let repo = sides();
  let local = change_made_twice_squashed_once(&repo);
  assert_eq!(squashes(&repo, &local), HashMap::new());
}

/// The upstream made the change of a gone commit, reverted it and made it again: the commit sums to either, so it names
/// neither
#[test]
fn a_set_equal_to_two_upstream_commits_names_neither() {
  let repo = sides();
  let local = repo.commit_history("refs/heads/local", &[at("(feature) Change line 5", &[(5, "B")], 200)]);
  repo.commit_history(
    "refs/heads/upstream",
    &[
      at("Change line 5 (#1)", &[(5, "B")], 900),
      at("Revert \"Change line 5 (#1)\"", &[], 910),
      at("Change line 5 again (#3)", &[(5, "B")], 920),
    ],
  );
  assert_eq!(squashes(&repo, &local), HashMap::new());
}

/// Without a baseline commit of the author of the gone commits that nothing names the search runs no process; with one
/// of other paths only their `diff-tree --name-only`
#[test]
fn the_gone_commits_read_no_diff_without_a_baseline_commit_of_their_author() {
  let repo = sides();
  let local = repo.commit_history("refs/heads/local", &[change_line_5()]);
  commit_by_another_author(&repo, "Change line 5 (#1)", &[(5, "five")]);
  assert_eq!(squashes_naming(&repo, &local, &[]), (HashMap::new(), Vec::new()));
  squash(&repo, HistoryCommit::new("Other file (#2)", &[("g.txt", "g\n")]).at(900));
  assert_eq!(squashes_naming(&repo, &local, &[]), (HashMap::new(), vec![("diff-tree".to_string(), 1)]));
}

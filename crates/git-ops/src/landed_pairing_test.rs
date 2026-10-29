use crate::landed_pairing::{
  CommitKey, Interdiff, LandedCandidate, LandedDrop, LandedPairing, LandedProof, LandedScope, PathLines, interdiffs, pair_landed, pair_twins, read_landed, walk_commits,
  with_patch_identical,
};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use test_utils::history::{HistoryCommit, numbered_lines};
use test_utils::process_budget::{counting, processes};

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

/// The pairing of `local` with `upstream`: the twins, then the rest by their patch id (`pair_landed`), and the twins alone
/// (`pair_twins`), the pairing before a replay
fn pairings(repo: &TestRepo) -> (LandedPairing, LandedPairing) {
  in_scope(repo, |scope| {
    let reads = read_landed(repo.git_executor(), repo.path_str(), scope).unwrap();
    let twins = pair_twins(repo.git_executor(), repo.path_str(), &reads, false).unwrap();
    let all = with_patch_identical(repo.git_executor(), repo.path_str(), scope, &reads, twins.clone(), false).unwrap();
    assert_eq!(all, pair_landed(repo.git_executor(), repo.path_str(), scope, false).unwrap());
    (all, twins)
  })
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
  let (all, twins) = pairings(&repo);
  assert_eq!(twins.drops, [drop_of(&local[0], &upstream[0], LandedProof::PatchId)]);
  assert_eq!(all, twins);
}

/// The same patch under another identity (the upstream reworded it) is no twin: only the rung after a conflict proves it
#[test]
fn the_same_patch_under_another_identity_is_proven_only_after_a_conflict() {
  let repo = sides();
  let local = repo.commit_history("refs/heads/local", &[change_line_5()]);
  let upstream = repo.commit_history("refs/heads/upstream", &[at("Fix line 5", &[(5, "five")], 300)]);
  let (all, twins) = pairings(&repo);
  assert_eq!(twins, LandedPairing::unpaired(local.clone()));
  assert_eq!(all.drops, [drop_of(&local[0], &upstream[0], LandedProof::PatchId)]);
}

/// The twins are paired before every replay: without a commit of the same identity, no process
#[test]
fn the_pairing_of_the_twins_runs_no_process_without_a_twin() {
  let repo = sides();
  repo.commit_history("refs/heads/local", &[change_line_5()]);
  repo.commit_history("refs/heads/upstream", &[at("Other change", &[(2, "two")], 100)]);
  let reads = in_scope(&repo, |scope| read_landed(repo.git_executor(), repo.path_str(), scope).unwrap());
  let git = counting();
  let twins = pair_twins(&git, repo.path_str(), &reads, false).unwrap();
  assert_eq!(twins, LandedPairing::unpaired(vec![repo.rev_parse("local").unwrap()]));
  assert_eq!(processes(&git), []);
}

/// The same changed lines under another subject are another commit
#[test]
fn the_same_lines_under_another_identity_pair_with_nothing() {
  let repo = sides();
  repo.commit_history("refs/heads/local", &[change_line_5()]);
  twin_after(&repo, (4, "four"), "Fix line 5", &[]);
  let paired = pairing(&repo);
  assert_eq!(paired, LandedPairing::unpaired(paired.local.clone()));
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
      fixups: vec![local[2].clone()],
    }]
  );
  assert_eq!(pairing.with_fixups, [local[0].clone(), local[2].clone()]);
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
  assert_eq!((pairing.candidates.len(), pairing.with_fixups.len()), (0, 0), "{pairing:?}");
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

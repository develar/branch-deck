//! The replay of local commits with each conflict of git resolved by words: the resolved commit, the commits around it,
//! a commit that does not resolve, and one that the words leave empty.

use crate::commit_writer::current_committer;
use crate::model::ResolvedFileSummary;
use crate::word_replay::{UpdateResolvedCommit, WordsReplay, replay_resolving_words};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// `base` with `f.txt`, `upstream` on it with `upstream_f` in `f.txt`, and three local commits on `base`: `a.txt`, then
/// `local_f` in `f.txt`, then `b.txt`. Returns (upstream, local commits oldest first)
fn diverged(repo: &TestRepo, upstream_f: &str, local_f: &str) -> (String, Vec<String>) {
  let base = repo.create_commit_with_files("base", &[("f.txt", "alpha beta gamma\nend\n"), ("keep.txt", "keep\n")]);
  let upstream = repo.create_commit("Change f upstream", "f.txt", upstream_f);
  repo.reset_hard(&base).unwrap();
  let local = vec![
    repo.create_commit("Add a", "a.txt", "a\n"),
    repo.create_commit("Change f", "f.txt", local_f),
    repo.create_commit("Add b", "b.txt", "b\n"),
  ];
  (upstream, local)
}

/// `diverged` and the replay of its local commits onto the upstream
fn replay(repo: &TestRepo, upstream_f: &str, local_f: &str) -> (String, Vec<String>, Option<WordsReplay>) {
  let (upstream, local) = diverged(repo, upstream_f, local_f);
  let (git, path) = (repo.git_executor(), repo.path_str());
  let committer = current_committer(git, path).unwrap();
  let replay = replay_resolving_words(git, path, &upstream, &local, &[], &committer).unwrap();
  (upstream, local, replay)
}

fn temp_refs(repo: &TestRepo) -> String {
  repo.git(&["for-each-ref", "--format=%(refname)", "refs/branch-deck/"])
}

#[test]
fn a_conflict_that_resolves_by_words_is_committed_and_the_rest_replays_onto_it() {
  let repo = TestRepo::new();
  let (upstream, local, replay) = replay(&repo, "alpha beta GAMMA\nend\n", "ALPHA beta gamma\nend\n");
  let replay = replay.unwrap();

  assert_eq!(repo.git(&["show", &format!("{}:f.txt", replay.tip)]), "ALPHA beta GAMMA\nend");
  assert_eq!(repo.git(&["log", "--format=%s", &format!("{upstream}..{}", replay.tip)]), "Add b\nChange f\nAdd a");
  let resolved_id = repo.git(&["rev-parse", &format!("{}~1", replay.tip)]);
  assert_eq!(
    replay.resolved,
    vec![UpdateResolvedCommit {
      commit: local[1].clone(),
      subject: "Change f".to_string(),
      new_id: resolved_id.clone(),
      conflicts: 1,
      files: vec![ResolvedFileSummary {
        path: "f.txt".to_string(),
        conflicts: 1,
      }],
    }]
  );
  // the author and the message of the local commit, the committer of the replay
  let format = "--format=%an <%ae> %at%n%B";
  assert_eq!(repo.git(&["log", "-1", format, &resolved_id]), repo.git(&["log", "-1", format, &local[1]]));
  assert_eq!(
    replay.resolved[0].note(),
    format!("branch-deck resolve\ncommit: {}\nresolved-by-words: 1 1\nwords\tf.txt\n", local[1])
  );
  assert_eq!(temp_refs(&repo), "");
}

#[test]
fn a_commit_whose_words_conflict_too_stops_the_replay() {
  let repo = TestRepo::new();
  let (_, _, replay) = replay(&repo, "one beta gamma\nend\n", "two beta gamma\nend\n");
  assert_eq!(replay, None);
  assert_eq!(temp_refs(&repo), "");
}

#[test]
fn a_commit_that_the_words_leave_without_a_change_is_dropped() {
  let repo = TestRepo::new();
  // the upstream has the word change of the local commit and one more
  let (upstream, _, replay) = replay(&repo, "ALPHA beta GAMMA\nend\n", "ALPHA beta gamma\nend\n");
  let replay = replay.unwrap();
  assert_eq!(repo.git(&["log", "--format=%s", &format!("{upstream}..{}", replay.tip)]), "Add b\nAdd a");
  assert_eq!(replay.resolved, Vec::new());
  assert_eq!(temp_refs(&repo), "");
}

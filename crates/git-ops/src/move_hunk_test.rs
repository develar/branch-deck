use super::move_hunk::{MoveHunkError, MoveHunkMode, MoveHunkOutcome, MoveHunkRequest, MovedChange, move_hunk};
use super::notes::{MappingNote, NoteSource, write_commit_notes};
use super::rewrite_test_utils::{assert_no_temp_refs, log_format, repo_path, show, stage_file, worktree_snapshot, write_file};
use super::squash_commits::restore_branch_head;
use anyhow::Result;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use text_diff::apply::LineBlock;
use text_diff::model::{DiffBudget, diff_texts};
use text_diff::policy::WhitespacePolicy;

/// Lines `line 1` ..= `line n`, with `changes` (line number, text) replacing some of them
fn lines(n: usize, changes: &[(usize, &str)]) -> String {
  (1..=n)
    .map(|i| {
      changes
        .iter()
        .find(|(line, _)| *line == i)
        .map_or_else(|| format!("line {i}\n"), |(_, text)| format!("{text}\n"))
    })
    .collect()
}

fn git(repo: &TestRepo, args: &[&str]) -> String {
  repo.git_executor().execute_command(args, repo_path(repo)).unwrap()
}

/// Write `files` and commit them with an old author and committer date, so a new committer shows
fn commit(repo: &TestRepo, message: &str, files: &[(&str, &str)], timestamp: i64) -> String {
  for (path, content) in files {
    write_file(repo, path, content).unwrap();
    git(repo, &["add", path]);
  }
  let date = format!("{timestamp} +0200");
  repo
    .git_executor()
    .cmd(&["commit", "-m", message], repo_path(repo))
    .env(&[("GIT_AUTHOR_DATE", &date), ("GIT_COMMITTER_DATE", &date)])
    .text()
    .unwrap();
  repo.head()
}

/// The blocks of `path` in `commit`, as the diff views show them; a side without the file is empty
fn blocks_of(repo: &TestRepo, commit: &str, path: &str) -> Vec<LineBlock> {
  let text = |rev: String| String::from_utf8_lossy(&repo.git_executor().cmd(&["cat-file", "blob", &rev], repo_path(repo)).bytes().unwrap_or_default()).into_owned();
  let (old, new) = (text(format!("{commit}^:{path}")), text(format!("{commit}:{path}")));
  diff_texts(&old, &new, WhitespacePolicy::Default, &DiffBudget::NONE)
    .blocks
    .iter()
    .map(LineBlock::from)
    .collect()
}

fn request<'a>(repo: &TestRepo, from: &'a str, to: &'a str, path: &'a str, change: MovedChange<'a>) -> MoveHunkRequest<'a> {
  MoveHunkRequest {
    from,
    to,
    path,
    change,
    // read in the test before the request; leaked, as the request borrows it
    expected_head: Box::leak(repo.head().into_boxed_str()),
    baseline_branch: "master",
    mode: MoveHunkMode::Apply,
  }
}

fn run(repo: &TestRepo, request: &MoveHunkRequest<'_>) -> Result<MoveHunkOutcome, MoveHunkError> {
  move_hunk(repo.git_executor(), repo_path(repo), request)
}

/// The new id of `old` in `outcome`
fn new_id<'a>(outcome: &'a MoveHunkOutcome, old: &str) -> &'a str {
  &outcome.commits.iter().find(|(id, _)| id == old).unwrap().1
}

fn files_of(repo: &TestRepo, commit: &str) -> String {
  git(repo, &["diff-tree", "--no-commit-id", "--name-only", "-r", commit])
}

fn added_lines(repo: &TestRepo, from: &str, to: &str, path: &str) -> Vec<String> {
  git(repo, &["diff", "-U0", from, to, "--", path])
    .lines()
    .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
    .map(str::to_string)
    .collect()
}

/// A mapping note of `original` onto `mapped`, with the trees of `original` and its parent as the source
fn write_mapping_note(repo: &TestRepo, original: &str, mapped: &str) -> Result<()> {
  let tree_of = |rev: &str| git(repo, &["rev-parse", &format!("{rev}^{{tree}}")]);
  let note = MappingNote {
    original: original.to_string(),
    mapped: mapped.to_string(),
    source: NoteSource {
      parent_tree: tree_of(&format!("{original}^")),
      tree: tree_of(original),
    },
  };
  write_commit_notes(repo.git_executor(), repo_path(repo), &[note])
}

/// Base (`f.txt` of 20 lines), `A` changes line 2 and adds a.txt, `X` adds x.txt, `B` changes line 18 and adds b.txt
fn base_a_x_b() -> (TestRepo, [String; 3]) {
  let repo = TestRepo::new();
  commit(&repo, "Base", &[("f.txt", &lines(20, &[]))], 1_600_000_000);
  let a = commit(&repo, "(a) A", &[("f.txt", &lines(20, &[(2, "two")])), ("a.txt", "a\n")], 1_600_000_100);
  let x = commit(&repo, "(x) X", &[("x.txt", "x\n")], 1_600_000_200);
  let b = commit(&repo, "(b) B", &[("f.txt", &lines(20, &[(2, "two"), (18, "eighteen")])), ("b.txt", "b\n")], 1_600_000_300);
  (repo, [a, x, b])
}

#[test]
fn moves_a_file_change_into_an_older_commit() -> Result<()> {
  let (repo, [a, x, b]) = base_a_x_b();
  let tree = git(&repo, &["rev-parse", "HEAD^{tree}"]);
  let x_committer = log_format(&repo, &x, "%cn <%ce> %cd")?;

  let outcome = run(&repo, &request(&repo, &b, &a, "f.txt", MovedChange::WholeFile))?;

  assert_eq!(git(&repo, &["rev-parse", "HEAD^{tree}"]), tree, "HEAD keeps its tree");
  assert_eq!(outcome.new_head, repo.head());
  assert_eq!(outcome.commits.iter().map(|(old, _)| old.as_str()).collect::<Vec<_>>(), [&a, &x, &b]);
  assert_eq!(outcome.changed, [a.clone(), b.clone()], "X keeps its diff");
  let (new_a, new_x, new_b) = (new_id(&outcome, &a), new_id(&outcome, &x), new_id(&outcome, &b));
  assert_eq!(show(&repo, new_a, "f.txt")?, lines(20, &[(2, "two"), (18, "eighteen")]).trim_end());
  assert_eq!(files_of(&repo, new_a), "a.txt\nf.txt");
  assert_eq!(files_of(&repo, new_x), "x.txt");
  assert_eq!(files_of(&repo, new_b), "b.txt", "B holds only the rest");
  // X kept its diff: author and committer byte for byte; A and B got the current user as committer
  assert_eq!(log_format(&repo, new_x, "%cn <%ce> %cd")?, x_committer);
  assert_eq!(log_format(&repo, new_a, "%an <%ae> %ad")?, log_format(&repo, &a, "%an <%ae> %ad")?);
  assert_ne!(log_format(&repo, new_a, "%cd")?, log_format(&repo, &a, "%cd")?);
  assert_eq!(git(&repo, &["reflog", "-1", "--format=%gs", "master"]), "branch-deck: move hunk");
  assert_no_temp_refs(&repo)?;

  // an undo is a ref move back: the trees are equal
  restore_branch_head(repo.git_executor(), repo_path(&repo), &outcome.new_head, &outcome.previous_head, "undo")?;
  assert_eq!(repo.head(), b);
  Ok(())
}

#[test]
fn moves_a_file_change_into_a_newer_commit() -> Result<()> {
  let (repo, [a, x, b]) = base_a_x_b();
  let tree = git(&repo, &["rev-parse", "HEAD^{tree}"]);

  let outcome = run(&repo, &request(&repo, &a, &b, "f.txt", MovedChange::WholeFile))?;

  assert_eq!(git(&repo, &["rev-parse", "HEAD^{tree}"]), tree);
  assert_eq!(outcome.changed, [a.clone(), b.clone()]);
  let (new_a, new_x, new_b) = (new_id(&outcome, &a), new_id(&outcome, &x), new_id(&outcome, &b));
  assert_eq!(files_of(&repo, new_a), "a.txt");
  assert_eq!(show(&repo, new_x, "f.txt")?, lines(20, &[]).trim_end(), "the commits between lose the change");
  assert_eq!(added_lines(&repo, new_x, new_b, "f.txt"), ["+two", "+eighteen"]);
  assert_eq!(files_of(&repo, new_b), "b.txt\nf.txt");
  assert_no_temp_refs(&repo)
}

#[test]
fn moves_one_block_of_three() -> Result<()> {
  let repo = TestRepo::new();
  commit(&repo, "Base", &[("f.txt", &lines(40, &[]))], 1_600_000_000);
  let a = commit(&repo, "(a) A", &[("a.txt", "a\n")], 1_600_000_100);
  let b = commit(&repo, "(b) B", &[("f.txt", &lines(40, &[(2, "two"), (20, "twenty"), (38, "thirty-eight")]))], 1_600_000_200);
  let blocks = blocks_of(&repo, &b, "f.txt");
  assert_eq!(blocks.len(), 3);
  let tree = git(&repo, &["rev-parse", "HEAD^{tree}"]);

  let outcome = run(&repo, &request(&repo, &b, &a, "f.txt", MovedChange::Blocks(&blocks[1..2])))?;

  assert_eq!(git(&repo, &["rev-parse", "HEAD^{tree}"]), tree);
  let (new_a, new_b) = (new_id(&outcome, &a).to_string(), new_id(&outcome, &b).to_string());
  assert_eq!(show(&repo, &new_a, "f.txt")?, lines(40, &[(20, "twenty")]).trim_end());
  assert_eq!(added_lines(&repo, &new_a, &new_b, "f.txt"), ["+two", "+thirty-eight"]);

  // and back: the block out of A into B again, as A's diff has it
  let blocks = blocks_of(&repo, &new_a, "f.txt");
  assert_eq!(blocks.len(), 1);
  let back = run(&repo, &request(&repo, &new_a, &new_b, "f.txt", MovedChange::Blocks(&blocks)))?;
  assert_eq!(files_of(&repo, new_id(&back, &new_a)), "a.txt");
  assert_eq!(git(&repo, &["rev-parse", "HEAD^{tree}"]), tree);
  assert_no_temp_refs(&repo)
}

#[test]
fn a_commit_between_that_shifts_the_lines_still_applies() -> Result<()> {
  let repo = TestRepo::new();
  commit(&repo, "Base", &[("f.txt", &lines(30, &[]))], 1_600_000_000);
  let a = commit(&repo, "(a) A", &[("a.txt", "a\n")], 1_600_000_100);
  // five lines inserted at the top
  let top = "top\n".repeat(5);
  let x = commit(&repo, "(x) X", &[("f.txt", &format!("{top}{}", lines(30, &[])))], 1_600_000_200);
  let b = commit(
    &repo,
    "(b) B",
    &[("f.txt", &format!("{top}{}", lines(30, &[(25, "twenty-five")]))), ("b.txt", "b\n")],
    1_600_000_300,
  );
  let x_committer = log_format(&repo, &x, "%cn <%ce> %cd")?;
  // a mapping note of X with its source, which names the new trees afterwards
  write_mapping_note(&repo, &x, &a)?;
  let tree_of = |rev: &str| git(&repo, &["rev-parse", &format!("{rev}^{{tree}}")]);
  let tree = tree_of("HEAD");

  let outcome = run(&repo, &request(&repo, &b, &a, "f.txt", MovedChange::WholeFile))?;

  assert_eq!(tree_of("HEAD"), tree);
  let (new_a, new_x) = (new_id(&outcome, &a), new_id(&outcome, &x));
  assert_eq!(
    show(&repo, new_a, "f.txt")?,
    lines(30, &[(25, "twenty-five")]).trim_end(),
    "the change lands on line 25 of A's version"
  );
  assert_eq!(show(&repo, new_x, "f.txt")?, format!("{top}{}", lines(30, &[(25, "twenty-five")])).trim_end());
  assert_eq!(outcome.changed, [a.clone(), b], "X only shifted the change");
  assert_eq!(log_format(&repo, new_x, "%cn <%ce> %cd")?, x_committer);
  let expected_note = MappingNote {
    original: new_x.to_string(),
    mapped: a.clone(),
    source: NoteSource {
      parent_tree: tree_of(new_a),
      tree: tree_of(new_x),
    },
  };
  assert_eq!(git(&repo, &["notes", "show", new_x]), expected_note.content());
  assert_no_temp_refs(&repo)
}

#[test]
fn a_commit_between_that_changes_the_same_lines_conflicts() -> Result<()> {
  let repo = TestRepo::new();
  commit(&repo, "Base", &[("f.txt", &lines(20, &[]))], 1_600_000_000);
  let a = commit(&repo, "(a) A", &[("a.txt", "a\n")], 1_600_000_100);
  commit(&repo, "(x) X", &[("f.txt", &lines(20, &[(10, "ten")]))], 1_600_000_200);
  let b = commit(&repo, "(b) B", &[("f.txt", &lines(20, &[(10, "TEN")])), ("b.txt", "b\n")], 1_600_000_300);
  let refs = git(&repo, &["for-each-ref"]);

  let error = run(&repo, &request(&repo, &b, &a, "f.txt", MovedChange::WholeFile)).expect_err("X changes the same line");

  match &error {
    MoveHunkError::Conflict { commit, path } => assert_eq!((commit.as_str(), path.as_str()), (a.as_str(), "f.txt")),
    other => panic!("not a conflict: {other}"),
  }
  assert!(error.to_string().contains(&a[..8]), "{error}");
  assert_eq!(repo.head(), b);
  assert_eq!(git(&repo, &["for-each-ref"]), refs);
  assert_no_temp_refs(&repo)
}

#[test]
fn keeps_dirty_and_staged_changes_byte_for_byte() -> Result<()> {
  let (repo, [a, _, b]) = base_a_x_b();
  stage_file(&repo, "f.txt", "staged\n")?;
  write_file(&repo, "f.txt", "staged and dirty\n")?;
  stage_file(&repo, "new.txt", "new\n")?;
  write_file(&repo, "b.txt", "b dirty\n")?;
  write_file(&repo, "untracked.txt", "untracked\n")?;
  let before = worktree_snapshot(&repo)?;

  run(&repo, &request(&repo, &b, &a, "f.txt", MovedChange::WholeFile))?;

  assert_eq!(worktree_snapshot(&repo)?, before);
  assert_eq!(std::fs::read_to_string(repo.path().join("f.txt"))?, "staged and dirty\n");
  Ok(())
}

#[test]
fn a_dry_run_writes_no_ref_and_no_note() -> Result<()> {
  let (repo, [a, _, b]) = base_a_x_b();
  let refs = git(&repo, &["for-each-ref"]);

  let outcome = run(
    &repo,
    &MoveHunkRequest {
      mode: MoveHunkMode::DryRun,
      ..request(&repo, &b, &a, "f.txt", MovedChange::WholeFile)
    },
  )?;

  assert_eq!((outcome.new_head.as_str(), outcome.commits.len()), (b.as_str(), 0));
  assert_eq!(outcome.changed, [a, b]);
  assert_eq!(git(&repo, &["for-each-ref"]), refs);
  Ok(())
}

/// A trial writes the rewritten commits for a trial build and moves no ref: no branch, no note, no temporary ref
#[test]
fn a_trial_writes_the_commits_and_moves_no_ref() -> Result<()> {
  let (repo, [a, x, b]) = base_a_x_b();
  let refs = git(&repo, &["for-each-ref"]);

  let outcome = run(
    &repo,
    &MoveHunkRequest {
      mode: MoveHunkMode::Trial,
      ..request(&repo, &b, &a, "f.txt", MovedChange::WholeFile)
    },
  )?;

  assert_eq!(git(&repo, &["for-each-ref"]), refs);
  assert_eq!((outcome.previous_head.as_str(), repo.head()), (b.as_str(), b.clone()));
  assert_eq!(outcome.commits.iter().map(|(old, _)| old.as_str()).collect::<Vec<_>>(), [&a, &x, &b]);
  assert_eq!(outcome.new_head, new_id(&outcome, &b));
  for ((_, new), tree) in outcome.commits.iter().zip(&outcome.trees) {
    assert_eq!(&git(&repo, &["rev-parse", &format!("{new}^{{tree}}")]), tree);
  }
  assert_eq!(outcome.trees.last(), Some(&git(&repo, &["rev-parse", "HEAD^{tree}"])), "the trial keeps HEAD's tree too");
  assert_eq!(files_of(&repo, new_id(&outcome, &a)), "a.txt\nf.txt");
  assert_eq!(outcome.changed, [a, b]);
  Ok(())
}

#[test]
fn refuses_a_move_that_leaves_a_commit_empty_or_names_no_change() {
  let (repo, [a, x, b]) = base_a_x_b();
  let head = repo.head();
  let refused = |request: &MoveHunkRequest<'_>, expected: &str| {
    match run(&repo, request) {
      Err(MoveHunkError::Refused(message)) => assert!(message.contains(expected), "{message}"),
      other => panic!("not refused: {other:?}"),
    }
    assert_eq!(repo.head(), head);
    assert_no_temp_refs(&repo).unwrap();
  };
  // X changes only x.txt
  refused(&request(&repo, &x, &a, "x.txt", MovedChange::WholeFile), "squash it instead");
  refused(&request(&repo, &x, &a, "f.txt", MovedChange::WholeFile), "doesn't change f.txt");
  refused(&request(&repo, &a, &a, "f.txt", MovedChange::WholeFile), "two different commits");
  refused(&request(&repo, &b, &a, "b.txt", MovedChange::Blocks(&blocks_of(&repo, &b, "b.txt"))), "move the whole file");

  let error = run(
    &repo,
    &MoveHunkRequest {
      expected_head: &a,
      ..request(&repo, &b, &a, "f.txt", MovedChange::WholeFile)
    },
  )
  .expect_err("HEAD moved");
  assert!(matches!(error, MoveHunkError::HeadMoved(_)), "{error}");
}

/// A block moves with the line ends of its file: CRLF lines, and lone CR lines, which git does not count as lines
#[test]
fn moves_a_block_of_a_crlf_file_and_of_a_lone_cr_file() -> Result<()> {
  for end in ["\r\n", "\r"] {
    let repo = TestRepo::new();
    let text = |lines: &[&str]| lines.iter().flat_map(|line| [*line, end]).collect::<String>();
    commit(&repo, "Base", &[("f.txt", &text(&["a", "b", "c", "d", "e", "f", "g", "h", "i"]))], 1_600_000_000);
    let a = commit(&repo, "(a) A", &[("a.txt", "a\n")], 1_600_000_100);
    let b = commit(&repo, "(b) B", &[("f.txt", &text(&["a", "B", "c", "d", "e", "f", "g", "H", "i"]))], 1_600_000_200);
    let blocks = blocks_of(&repo, &b, "f.txt");
    assert_eq!(blocks.len(), 2, "{end:?}");

    let outcome = run(&repo, &request(&repo, &b, &a, "f.txt", MovedChange::Blocks(&blocks[1..])))?;

    let new_a = new_id(&outcome, &a);
    let blob = repo.git_executor().cmd(&["cat-file", "blob", &format!("{new_a}:f.txt")], repo_path(&repo)).bytes()?;
    assert_eq!(String::from_utf8(blob)?, text(&["a", "b", "c", "d", "e", "f", "g", "H", "i"]), "{end:?}");
    assert_no_temp_refs(&repo)?;
  }
  Ok(())
}

/// The processes of a move whose range has a mapping note, by subcommand: a fixed number whatever the range, one `var`
/// for the committer of the commits and of the notes commit
#[test]
fn a_move_runs_a_fixed_set_of_processes() -> Result<()> {
  let (repo, [a, x, b]) = base_a_x_b();
  write_mapping_note(&repo, &x, &a)?;
  let counting = repo.git_executor().with_process_stats("test");

  move_hunk(&counting, repo_path(&repo), &request(&repo, &b, &a, "f.txt", MovedChange::WholeFile))?;

  let processes: Vec<(String, u32)> = counting
    .process_stats()
    .unwrap()
    .by_command()
    .into_iter()
    .map(|(subcommand, stats)| (subcommand, stats.count))
    .collect();
  let expected = [
    ("cat-file", 1),
    ("diff-tree", 2),
    ("fast-import", 2),
    ("log", 1),
    ("merge-tree", 1),
    ("notes", 1),
    ("patch-id", 1),
    ("rev-parse", 2),
    ("show-ref", 1),
    ("symbolic-ref", 2),
    ("update-ref", 1),
    ("var", 1),
  ];
  assert_eq!(processes, expected.map(|(subcommand, count)| (subcommand.to_string(), count)));
  Ok(())
}

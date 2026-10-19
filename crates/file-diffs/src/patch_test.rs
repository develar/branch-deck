//! The blocks as `bd commits --patch` prints them: the context, the header with the scope label, the ids.

use crate::blocks::BlobPair;
use crate::fixture_test::{commit_all, process_counts, repo_path, write};
use crate::patch::{BlockPatch, block_patches};
use crate::service::FileDiffs;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use text_diff::apply::{BlockId, apply_blocks};
use text_diff::model::{BlockKind, DiffBudget, diff_texts};
use text_diff::policy::WhitespacePolicy;

fn patches(old: &str, new: &str) -> Vec<(String, String, String)> {
  let diff = diff_texts(old, new, WhitespacePolicy::Default, &DiffBudget::NONE);
  block_patches(old, new, &diff, &[], &[])
    .into_iter()
    .map(|patch| (patch.id.to_string(), patch.header, patch.text))
    .collect()
}

const OLD: &str = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";

/// Each block has up to three lines of context, cut at a neighbour block: two blocks four lines apart stay two blocks
#[test]
fn each_block_has_its_own_context() {
  let new = OLD.replace("2\n", "two\n").replace("7\n", "seven\n");
  let patches = patches(OLD, &new);
  let headers: Vec<&str> = patches.iter().map(|(_, header, _)| header.as_str()).collect();
  assert_eq!(headers, ["@@ -1,5 +1,5 @@", "@@ -4,7 +4,7 @@"]);
  assert_eq!(patches[0].2, " 1\n-2\n+two\n 3\n 4\n 5");
  assert_eq!(patches[1].2, " 4\n 5\n 6\n-7\n+seven\n 8\n 9\n 10");
  assert!(patches[0].0.starts_with("2,1:2,1@"), "{}", patches[0].0);
}

/// An insertion names the line before it; a final line end added is a block on the empty last line, and that empty
/// line is no context
#[test]
fn an_insertion_and_a_final_line_end() {
  let patches = patches("a\nb\nc", "a\nadded\nb\nc\n");
  assert_eq!(
    patches
      .iter()
      .map(|(id, header, text)| (id.split('@').next().unwrap(), header.as_str(), text.as_str()))
      .collect::<Vec<_>>(),
    [("1,0:2,1", "@@ -1,3 +1,4 @@", " a\n+added\n b\n c"), ("3,0:5,1", "@@ -2,2 +3,3 @@", " b\n c\n+")]
  );
}

/// The label of a block is the innermost scope its first line is in; a deletion takes the scope of the old text
#[test]
fn the_header_names_the_scope() {
  let old = "class A {\n  fun a() {\n    one()\n  }\n  fun b() = 2\n}\n";
  let new = "class A {\n  fun a() {\n    ONE()\n  }\n}\n";
  let diff = diff_texts(old, new, WhitespacePolicy::Default, &DiffBudget::NONE);
  let scope = |start_line: u32, end_line: u32, label: &str| text_diff::model::Scope {
    start_line,
    end_line,
    label: label.to_string(),
  };
  let old_scopes = [scope(0, 6, "A"), scope(1, 4, "A › a"), scope(4, 5, "A › b")];
  let new_scopes = [scope(0, 5, "A"), scope(1, 4, "A › a")];
  let headers: Vec<String> = block_patches(old, new, &diff, &old_scopes, &new_scopes).into_iter().map(|patch| patch.header).collect();
  assert_eq!(headers, ["@@ -1,4 +1,4 @@ A › a", "@@ -4,3 +4,2 @@ A › b"]);
}

/// The ids of `patches_of` parse and apply: one block moved alone gives a text between the two sides. A final line end
/// removed is a block of its own kind
#[test]
fn the_ids_of_a_commit_apply() {
  let repo = TestRepo::new();
  write(&repo, "A.kt", OLD.as_bytes());
  commit_all(&repo, "Base");
  let new = OLD.replace("2\n", "two\n").trim_end().to_string();
  write(&repo, "A.kt", new.as_bytes());
  let commit = commit_all(&repo, "Change");
  let id = |path: &str| repo.git_executor().execute_command(&["rev-parse", path], repo_path(&repo)).unwrap();
  let (old_id, new_id) = (id(&format!("{commit}^:A.kt")), id(&format!("{commit}:A.kt")));

  let counting = repo.git_executor().with_process_stats("test");
  let pair = BlobPair {
    path: "A.kt",
    old_path: None,
    old_id: Some(&old_id),
    new_id: Some(&new_id),
  };
  let patches: Vec<BlockPatch> = FileDiffs::default()
    .patches_of(&counting, repo_path(&repo), &ObjectCache::disabled(), &[pair])
    .unwrap()
    .remove(0)
    .unwrap();
  assert_eq!(process_counts(&counting), [("cat-file".to_string(), 1)]);
  assert_eq!(patches.iter().map(|patch| patch.kind).collect::<Vec<_>>(), [BlockKind::Changed, BlockKind::WhitespaceOnly]);

  let parsed = BlockId::parse(&patches[0].id.to_string()).unwrap();
  assert_eq!(parsed, patches[0].id);
  let moved = apply_blocks(OLD.as_bytes(), new.as_bytes(), &[parsed.block]).unwrap();
  assert_eq!(String::from_utf8(moved).unwrap(), OLD.replace("2\n", "two\n"));
}

//! The `-z` output of `git merge-tree --write-tree` (without `--name-only`): the tree, the conflicted file info (one
//! entry per stage) and the informational messages, read as bytes. Shared by the sync's cherry-pick and the conflict
//! rounds of `bd update` (`update_resolve`). `parse_batch_merge` reads one merge of a `--stdin` run, from its whole
//! output (`parse_merge_tree_batch`, `bd move-hunk`) or from the running process (the sync's `CommitChain`).

use anyhow::{Context, Result, anyhow, bail};

/// One stage of a conflicted path: `<mode> <object> <stage>\t<path>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageEntry {
  pub mode: String,
  pub id: String,
  /// 1 the merge base, 2 ours, 3 theirs
  pub stage: u8,
  /// The path as git stores it, not necessarily UTF-8
  pub path: Vec<u8>,
}

/// An informational message: the paths it is about, its stable conflict type (`CONFLICT (contents)`, `Auto-merging`)
/// and the translated text, which is only shown, never parsed
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeMessage {
  pub paths: Vec<Vec<u8>>,
  pub kind: String,
  pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeTreeOutput {
  /// The merge result, with conflict markers in the conflicted files
  pub tree: String,
  /// The stages of the conflicted paths, in git's order (by path, then stage); empty for a clean merge
  pub entries: Vec<StageEntry>,
  /// Empty with `--no-messages`
  pub messages: Vec<MergeMessage>,
}

impl MergeTreeOutput {
  /// The conflicted paths, each once, in git's order
  pub fn conflicted_paths(&self) -> Vec<&[u8]> {
    let mut paths: Vec<&[u8]> = Vec::new();
    for entry in &self.entries {
      if paths.last() != Some(&entry.path.as_slice()) {
        paths.push(&entry.path);
      }
    }
    paths
  }
}

/// Parse `<tree>\0`, then `<mode> <object> <stage>\t<path>\0` per stage and an empty entry, then per message
/// `<path count>\0<path>\0...<type>\0<text>\0`
pub fn parse_merge_tree_output(output: &[u8]) -> Result<MergeTreeOutput> {
  let mut fields = output.split(|&b| b == 0);
  let tree = fields.next().filter(|tree| !tree.is_empty()).ok_or_else(|| anyhow!("git merge-tree printed no tree"))?;
  let tree = std::str::from_utf8(tree)?.trim().to_string();

  let mut entries = Vec::new();
  for field in fields.by_ref() {
    if field.is_empty() {
      break;
    }
    entries.push(parse_stage_entry(field)?);
  }

  let mut messages = Vec::new();
  while let Some(count) = fields.next().filter(|count| !count.is_empty()) {
    let count: usize = std::str::from_utf8(count)?.parse().map_err(|e| anyhow!("Unexpected merge-tree message count: {e}"))?;
    let mut paths = Vec::with_capacity(count);
    for _ in 0..count {
      paths.push(fields.next().ok_or_else(|| anyhow!("truncated merge-tree message"))?.to_vec());
    }
    let (Some(kind), Some(text)) = (fields.next(), fields.next()) else {
      bail!("truncated merge-tree message");
    };
    messages.push(MergeMessage {
      paths,
      kind: String::from_utf8_lossy(kind).into_owned(),
      text: String::from_utf8_lossy(text).into_owned(),
    });
  }
  Ok(MergeTreeOutput { tree, entries, messages })
}

/// One merge of a `merge-tree --write-tree --stdin -z --no-messages` run
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchMerge {
  pub clean: bool,
  pub output: MergeTreeOutput,
}

/// Parse the output of `merge-tree --write-tree --stdin -z --no-messages` for `count` input lines, read at once
/// (`parse_batch_merge` per merge)
pub fn parse_merge_tree_batch(output: &[u8], count: usize) -> Result<Vec<BatchMerge>> {
  let mut fields = output.split(|&b| b == 0);
  (0..count)
    .map(|index| parse_batch_merge(|| fields.next().ok_or_else(|| anyhow!("truncated merge-tree output"))).with_context(|| format!("merge {index} of {count} of git merge-tree")))
    .collect()
}

/// Read one merge of a `merge-tree --write-tree --stdin -z --no-messages` run from `next_field`, its NUL-terminated
/// fields in order: from the whole output, or a field at a time from the running process (`CommitChain`). A merge is
/// `<status>\0<tree>\0`, a stage entry per conflicted stage and an empty entry. The status is 1 for a clean merge and 0
/// for a conflict; git exits with 0 either way, so the status is the only sign of a conflict. Without `--no-messages`
/// the messages would follow each merge with no end marker, so they are not supported
pub fn parse_batch_merge<F: AsRef<[u8]>>(mut next_field: impl FnMut() -> Result<F>) -> Result<BatchMerge> {
  let status = next_field()?;
  let clean = match status.as_ref() {
    b"1" => true,
    b"0" => false,
    status => bail!("Unexpected merge-tree status {:?}", String::from_utf8_lossy(status)),
  };
  let tree = next_field()?;
  let tree = std::str::from_utf8(tree.as_ref())?;
  if tree.is_empty() {
    bail!("git merge-tree printed no tree");
  }
  let tree = tree.to_string();
  let mut entries = Vec::new();
  loop {
    let field = next_field()?;
    if field.as_ref().is_empty() {
      break;
    }
    entries.push(parse_stage_entry(field.as_ref())?);
  }
  if clean && !entries.is_empty() {
    bail!("git merge-tree reported a clean merge with {} conflicted stages", entries.len());
  }
  Ok(BatchMerge {
    clean,
    output: MergeTreeOutput {
      tree,
      entries,
      messages: Vec::new(),
    },
  })
}

fn parse_stage_entry(field: &[u8]) -> Result<StageEntry> {
  let tab = memchr::memchr(b'\t', field).ok_or_else(|| anyhow!("Unexpected merge-tree entry: {}", String::from_utf8_lossy(field)))?;
  let prefix = std::str::from_utf8(&field[..tab])?;
  let mut parts = prefix.split(' ');
  let (Some(mode), Some(id), Some(stage), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
    bail!("Unexpected merge-tree entry: {prefix}");
  };
  Ok(StageEntry {
    mode: mode.to_string(),
    id: id.to_string(),
    stage: stage.parse().map_err(|e| anyhow!("Unexpected merge-tree stage {stage:?}: {e}"))?,
    path: field[tab + 1..].to_vec(),
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;

  #[test]
  fn parses_the_tree_the_stages_and_the_messages() {
    let output = b"t1\x00100644 b1 1\tf.txt\x00100644 b2 2\tf.txt\x00100644 b3 3\tf.txt\0\0\
1\0f.txt\0Auto-merging\0Auto-merging f.txt\n\0\
2\0df~x\0df\0CONFLICT (file/directory)\0moved\n\0";
    let parsed = parse_merge_tree_output(output).unwrap();
    assert_eq!(parsed.tree, "t1");
    assert_eq!(parsed.entries.iter().map(|e| e.stage).collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(parsed.entries[1].id, "b2");
    assert_eq!(parsed.conflicted_paths(), [b"f.txt".as_slice()]);
    assert_eq!(parsed.messages.len(), 2);
    assert_eq!(parsed.messages[1].paths, [b"df~x".to_vec(), b"df".to_vec()]);
    assert_eq!(parsed.messages[1].kind, "CONFLICT (file/directory)");
  }

  #[test]
  fn parses_the_merges_of_a_stdin_run() {
    let output = b"1\0t1\0\0\x30\0t2\x00100644 b1 1\tf.txt\x00100644 b2 2\tf.txt\x00100644 b3 3\tf.txt\0\0\x31\0t3\0\0";
    let merges = parse_merge_tree_batch(output, 3).unwrap();
    assert_eq!(
      merges.iter().map(|merge| (merge.clean, merge.output.tree.as_str())).collect::<Vec<_>>(),
      [(true, "t1"), (false, "t2"), (true, "t3")]
    );
    assert_eq!(merges[1].output.conflicted_paths(), [b"f.txt".as_slice()]);
    assert!(parse_merge_tree_batch(output, 4).is_err(), "a missing merge is an error");
  }

  #[test]
  fn a_clean_merge_with_conflicted_stages_is_an_error() {
    let error = parse_merge_tree_batch(b"1\0t1\x00100644 b1 1\tf.txt\0\0", 1).unwrap_err();
    assert!(format!("{error:#}").contains("clean merge with 1 conflicted stages"), "{error:#}");
    let error = parse_merge_tree_batch(b"2\0t1\0\0", 1).unwrap_err();
    assert!(format!("{error:#}").contains("Unexpected merge-tree status"), "{error:#}");
    let error = parse_merge_tree_batch(b"1\0\0\0", 1).unwrap_err();
    assert!(format!("{error:#}").contains("printed no tree"), "{error:#}");
  }

  #[test]
  fn a_clean_merge_has_only_the_tree() {
    let parsed = parse_merge_tree_output(b"t1\0").unwrap();
    assert_eq!((parsed.tree.as_str(), parsed.entries.len(), parsed.messages.len()), ("t1", 0, 0));
  }
}

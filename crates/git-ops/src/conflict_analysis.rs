use crate::commit_files::{FilePatch, MAX_BLOB_BYTES, PATCH_LIMITS, PatchLimits, is_object_id, split_patches};
use crate::object_texts::read_texts;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;
use std::collections::{HashMap, HashSet};
use tracing::{debug, instrument};

/// Represents a commit that exists in the source branch but is missing from the target branch.
/// These commits might be causing merge conflicts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct MissingCommit {
  pub hash: String,
  pub subject: String,
  pub message: String,
  pub author_time: u32,
  pub committer_time: u32,
  pub author: String,
  pub files_touched: Vec<String>,
  pub file_diffs: Vec<FileDiff>,
  /// The branch the commit is in, `None` for an unassigned commit; set by the sync, which knows the grouping
  pub branch: Option<String>,
}

/// Represents the diff between two versions of a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
  pub old_file: FileInfo,
  pub new_file: FileInfo,
  /// The patch of the file: one `diff --git` section, or hunks that each repeat the `---`/`+++` header; the frontend
  /// joins them into one patch for the diff view
  pub hunks: Vec<String>,
}

/// Information about a file including its content and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
  pub file_name: String,
  pub file_lang: String,
  pub content: String,
}

/// Analysis results for a merge conflict, including missing commits and divergence information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictAnalysis {
  pub missing_commits: Vec<MissingCommit>,
  pub merge_base_hash: String,
  pub merge_base_subject: String,
  pub merge_base_message: String,
  pub merge_base_time: u32,
  pub merge_base_author: String,
  pub divergence_summary: DivergenceSummary,
}

/// Summary of how two branches have diverged from their common ancestor.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct DivergenceSummary {
  pub commits_ahead_in_source: u32,  // How many commits the source branch is ahead
  pub commits_ahead_in_target: u32,  // How many commits the target branch is ahead
  pub common_ancestor_distance: u32, // How far back is the common ancestor
}

/// The environment of a git whose pathspecs are paths: names, never globs (`a*.txt`, `[x].md`) or magic (`:name`)
pub(crate) const LITERAL_PATHSPECS: [(&str, &str); 1] = [("GIT_LITERAL_PATHSPECS", "1")];

/// `\x01` starts a commit, NUL ends a field; `%B` is the last one, so a message may hold anything but NUL
const MISSING_COMMIT_FORMAT: &str = "--format=%x01%H%x00%P%x00%at%x00%ct%x00%an%x00%s%x00%B";

/// The commits of `source` that `target` lacks and that change one of `conflicting_files`, newest first, from one
/// `log -z --name-only`: the paths verbatim, the whole message and the first parent of each (`None` for a root
/// commit). Their diffs are not read: `read_conflict_patches` and `get_files_content_at_revisions` read them with the
/// other objects of a conflict
#[instrument(skip(git_executor, conflicting_files))]
pub(crate) fn list_missing_commits(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  source: &str,
  target: &str,
  conflicting_files: &[String],
) -> Result<Vec<(MissingCommit, Option<String>)>> {
  if conflicting_files.is_empty() {
    return Ok(Vec::new());
  }
  let exclude_target = format!("^{target}");
  let mut args = vec![
    "log",
    "-z",
    "--name-only",
    "--no-renames",
    "--no-merges",
    MISSING_COMMIT_FORMAT,
    source,
    &exclude_target,
    "--",
  ];
  args.extend(conflicting_files.iter().map(String::as_str));
  let output = git_executor.cmd(&args, repo_path).env(&LITERAL_PATHSPECS).bytes()?;
  let output = String::from_utf8(output).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned());
  let conflicting: HashSet<&str> = conflicting_files.iter().map(String::as_str).collect();
  let listed = parse_missing_commits(&output, &conflicting);
  for (commit, _) in &listed {
    debug!(commit_hash = %commit.hash, files_touched = ?commit.files_touched, "found missing commit that touches conflicting files");
  }
  Ok(listed)
}

/// The commits of `log -z --name-only` with `MISSING_COMMIT_FORMAT` that change a `conflicting` path, without their
/// diffs, each with its first parent (`None` for a root commit). A commit is `\x01<id>`, its fields and its message,
/// each ended by NUL, then a newline before the first path and `<path>\0` per changed path
fn parse_missing_commits(output: &str, conflicting: &HashSet<&str>) -> Vec<(MissingCommit, Option<String>)> {
  let is_commit_start = |field: &str| field.strip_prefix('\x01').is_some_and(is_object_id);
  let mut commits = Vec::new();
  let mut fields = output.split('\0').peekable();
  while let Some(hash) = fields.next().and_then(|field| field.strip_prefix('\x01')) {
    let [parent_ids, author_time, committer_time, author, subject, message] = std::array::from_fn(|_| fields.next().unwrap_or_default());
    let mut files_touched = Vec::new();
    let mut first = true;
    while let Some(path) = fields.next_if(|field| !is_commit_start(field)) {
      let path = if first { path.strip_prefix('\n').unwrap_or(path) } else { path };
      first = false;
      if conflicting.contains(path) {
        files_touched.push(path.to_string());
      }
    }
    if files_touched.is_empty() {
      continue;
    }
    let parent = parent_ids.split(' ').next().filter(|parent| !parent.is_empty()).map(str::to_string);
    let commit = MissingCommit {
      hash: hash.to_string(),
      subject: subject.to_string(),
      message: message.trim().to_string(),
      author_time: author_time.parse().unwrap_or(0),
      committer_time: committer_time.parse().unwrap_or(0),
      author: author.to_string(),
      files_touched,
      file_diffs: Vec::new(),
      branch: None,
    };
    commits.push((commit, parent));
  }
  commits
}

/// The merge base of two commits; `None` when they have none (`merge-base` exits with 1), any other failure is an error
#[instrument(skip(git_executor))]
pub(crate) fn find_merge_base(git_executor: &GitCommandExecutor, repo_path: &str, commit1: &str, commit2: &str) -> Result<Option<String>> {
  let output = git_executor.cmd(&["merge-base", "--end-of-options", commit1, commit2], repo_path).status()?;
  match output.code {
    0 => Ok(Some(output.into_text())),
    1 => Ok(None),
    code => Err(anyhow!("git merge-base {commit1} {commit2} failed with {code}: {}", output.stderr)),
  }
}

/// Where two commits diverged: their merge base, and the commits each has that the other lacks
#[derive(Debug, Clone)]
pub(crate) struct Divergence {
  pub merge_base: String,
  pub summary: DivergenceSummary,
}

/// The merge base and the divergence of `source` and `target` from one `rev-list --left-right --boundary
/// <source>...<target>`: the commits of each side are counted, and the boundary, the commits outside both sides that a
/// commit of a side has as a parent, holds every merge base (the child of a merge base on the way to a side is on that
/// side), so a boundary of one commit is the merge base. Any other boundary (criss-cross merges, a side that merged the
/// other, unrelated histories) leaves the merge base to one `merge-base`; `None` without one
#[instrument(skip(git_executor))]
pub(crate) fn find_divergence(git_executor: &GitCommandExecutor, repo_path: &str, source: &str, target: &str) -> Result<Option<Divergence>> {
  let range = format!("{source}...{target}");
  let output = git_executor.execute_command(&["rev-list", "--left-right", "--boundary", &range, "--"], repo_path)?;
  let (mut ahead_in_source, mut ahead_in_target) = (0u32, 0u32);
  let mut boundary = Vec::new();
  for line in output.lines() {
    match line.as_bytes().first() {
      Some(b'<') => ahead_in_source += 1,
      Some(b'>') => ahead_in_target += 1,
      Some(b'-') => boundary.push(&line[1..]),
      _ => return Err(anyhow!("Unexpected rev-list line: {line}")),
    }
  }
  let merge_base = match boundary.as_slice() {
    [merge_base] => (*merge_base).to_string(),
    _ => match find_merge_base(git_executor, repo_path, source, target)? {
      Some(merge_base) => merge_base,
      None => return Ok(None),
    },
  };
  Ok(Some(Divergence {
    merge_base,
    summary: DivergenceSummary {
      commits_ahead_in_source: ahead_in_source,
      commits_ahead_in_target: ahead_in_target,
      common_ancestor_distance: ahead_in_source.min(ahead_in_target),
    },
  }))
}

/// A missing commit whose file diffs `file_diffs_of` builds
#[derive(Debug, Clone, Copy)]
pub(crate) struct CommitChange<'a> {
  pub id: &'a str,
  /// The first parent; `None` for a root commit, whose files are all added
  pub parent: Option<&'a str>,
  pub files: &'a [String],
}

impl<'a> CommitChange<'a> {
  /// The change of each listed commit (`list_missing_commits`)
  pub(crate) fn of(listed: &'a [(MissingCommit, Option<String>)]) -> Vec<Self> {
    listed
      .iter()
      .map(|(commit, parent)| CommitChange {
        id: &commit.hash,
        parent: parent.as_deref(),
        files: &commit.files_touched,
      })
      .collect()
  }
}

/// The revisions and files whose contents the diffs of `changes` need, for `get_files_content_at_revisions`: the files
/// of each commit at the commit, and at each parent the files of the commits whose parent it is
pub(crate) fn content_requests<'a>(changes: &[CommitChange<'a>]) -> Vec<(&'a str, Vec<String>)> {
  let mut requests: Vec<(&str, Vec<String>)> = changes.iter().map(|change| (change.id, change.files.to_vec())).collect();
  let mut files_at_parent: HashMap<&str, usize> = HashMap::new();
  for change in changes {
    let Some(parent) = change.parent else { continue };
    let index = *files_at_parent.entry(parent).or_insert_with(|| {
      requests.push((parent, Vec::new()));
      requests.len() - 1
    });
    for file in change.files {
      if !requests[index].1.contains(file) {
        requests[index].1.push(file.clone());
      }
    }
  }
  requests
}

/// The diffs of the files of each of `changes`, by commit, from the contents (`get_files_content_at_revisions` of
/// `content_requests`) and the patches (`read_conflict_patches`) read for them
pub(crate) fn file_diffs_of(
  changes: &[CommitChange<'_>],
  contents: &HashMap<String, HashMap<String, String>>,
  patches: &HashMap<String, HashMap<String, String>>,
) -> HashMap<String, Vec<FileDiff>> {
  let empty_map = HashMap::new();
  let mut result = HashMap::with_capacity(changes.len());
  for change in changes {
    let file_to_diff = patches.get(change.id).unwrap_or(&empty_map);
    // a root commit's files have no old content
    let parent_contents = change.parent.and_then(|parent| contents.get(parent)).unwrap_or(&empty_map);
    let current_contents = contents.get(change.id).unwrap_or(&empty_map);
    let file_diffs = change
      .files
      .iter()
      .map(|file| {
        let ext = file.split('.').next_back().unwrap_or("").to_string();
        let diff = file_to_diff.get(file).cloned().unwrap_or_default();
        FileDiff {
          old_file: FileInfo {
            file_name: file.clone(),
            file_lang: ext.clone(),
            content: parent_contents.get(file).cloned().unwrap_or_default(),
          },
          new_file: FileInfo {
            file_name: file.clone(),
            file_lang: ext,
            content: current_contents.get(file).cloned().unwrap_or_default(),
          },
          hunks: if diff.trim().is_empty() { vec![] } else { vec![diff] },
        }
      })
      .collect();
    result.insert(change.id.to_string(), file_diffs);
  }
  result
}

/// The patches of `diffs` limited to `files`, by commit and path, from one `diff-tree --stdin -p` cut as it streams
/// (`split_patches`): `(commit, None)` is the change of a commit (a root commit adds its files), `(commit, Some(base))`
/// the change from `base` to the commit; git prints the id of each commit on a line of its own before its patch, and
/// nothing for one without changes in `files`. A commit's patch may name other files of `files`; the caller looks up
/// its own
#[instrument(skip_all, fields(commits = diffs.len()))]
pub(crate) fn read_conflict_patches(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  diffs: &[(&str, Option<&str>)],
  files: &[String],
) -> Result<HashMap<String, HashMap<String, String>>> {
  let mut ids: Vec<String> = Vec::with_capacity(diffs.len());
  let mut input = String::new();
  for (commit, base) in diffs {
    if ids.iter().any(|id| id == commit) {
      continue;
    }
    ids.push((*commit).to_string());
    input.push_str(commit);
    if let Some(base) = base {
      input.push(' ');
      input.push_str(base);
    }
    input.push('\n');
  }
  let mut patches = HashMap::with_capacity(ids.len());
  if ids.is_empty() || files.is_empty() {
    return Ok(patches);
  }
  // `--root`: the patch of a root commit, all added
  let mut args = vec!["diff-tree", "--stdin", "--root"];
  args.extend(PATCH_ARGS);
  args.push("--");
  args.extend(files.iter().map(String::as_str));
  let command = git_executor.cmd(&args, repo_path).env(&LITERAL_PATHSPECS).stdin(input.as_bytes());
  split_patches(command, &ids, &CONFLICT_PATCH_LIMITS, |id, files| {
    patches.insert(id, sections_by_path(files));
  })?;
  Ok(patches)
}

/// Options for patch output whose sections `sections_by_path` keys regardless of user configuration: fixed `a/` and
/// `b/` prefixes and no rename pairs, so both header paths are equal. The executor's `core.quotePath=false` leaves only
/// paths with control characters, quotes or backslashes C-quoted.
pub(crate) const PATCH_ARGS: [&str; 8] = [
  "-p",
  "--no-color",
  "--no-ext-diff",
  "--no-textconv",
  "--no-renames",
  "--unified=3",
  "--src-prefix=a/",
  "--dst-prefix=b/",
];

/// What a patch of the conflict view keeps of a file (`PatchSplitter`): the patch of a file up to the largest text the
/// diff view takes (`MAX_BLOB_BYTES`), whatever its lines, and the commit's total of the commit patches
pub(crate) const CONFLICT_PATCH_LIMITS: PatchLimits = PatchLimits {
  file_bytes: MAX_BLOB_BYTES,
  file_lines: u32::MAX,
  total_bytes: PATCH_LIMITS.total_bytes,
};

/// The sections of a patch produced with `PATCH_ARGS` (`PatchSplitter`), by path; a section whose header cannot be
/// parsed is dropped
pub(crate) fn sections_by_path(files: Vec<FilePatch>) -> HashMap<String, String> {
  files
    .into_iter()
    .filter_map(|file| {
      let header = file.patch.lines().next().and_then(|line| line.strip_prefix("diff --git "))?;
      let Some(path) = parse_diff_header_path(header) else {
        debug!(header, "cannot parse diff header, skipping file section");
        return None;
      };
      Some((path, file.patch))
    })
    .collect()
}

/// Path from the part of a `diff --git` line after the command: `a/<path> b/<path>`, C-quoted when needed
fn parse_diff_header_path(header: &str) -> Option<String> {
  if header.starts_with('"') {
    let (path, _) = unquote_c_style(header)?;
    return path.strip_prefix("a/").map(str::to_string);
  }
  // Without renames both paths are equal, so the line is "a/" + path + " b/" + path
  let path_len = header.len().checked_sub(5)? / 2;
  let path = header.strip_prefix("a/")?.get(..path_len)?;
  (header.get(2 + path_len..)? == format!(" b/{path}")).then(|| path.to_string())
}

/// Decode a C-style quoted string as git writes it for unusual paths. Returns the value and the rest after the closing quote.
fn unquote_c_style(quoted: &str) -> Option<(String, &str)> {
  let bytes = quoted.as_bytes();
  if bytes.first() != Some(&b'"') {
    return None;
  }
  let mut value = Vec::with_capacity(bytes.len());
  let mut i = 1;
  while i < bytes.len() {
    match bytes[i] {
      b'"' => return Some((String::from_utf8_lossy(&value).into_owned(), &quoted[i + 1..])),
      b'\\' => {
        let escaped = *bytes.get(i + 1)?;
        i += 2;
        let decoded = match escaped {
          b'a' => 0x07,
          b'b' => 0x08,
          b'f' => 0x0c,
          b'n' => b'\n',
          b'r' => b'\r',
          b't' => b'\t',
          b'v' => 0x0b,
          b'0'..=b'7' => {
            // Three octal digits, the first one already consumed
            let octal = quoted.get(i - 1..i + 2)?;
            i += 2;
            u8::from_str_radix(octal, 8).ok()?
          }
          other => other,
        };
        value.push(decoded);
      }
      byte => {
        value.push(byte);
        i += 1;
      }
    }
  }
  None
}

/// Content of files at several revisions with one `git cat-file --batch` reading `<rev>:<path>` names, keyed by
/// revision and path (`read_texts`). A file that is absent at a revision, is not a blob, is binary or is larger than the
/// diff view shows (`MAX_BLOB_BYTES`, its bytes skipped unread) has empty content.
#[instrument(skip(git_executor, requests), fields(revisions = requests.len()))]
pub(crate) fn get_files_content_at_revisions(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  requests: &[(&str, &[String])],
) -> Result<HashMap<String, HashMap<String, String>>> {
  let mut result: HashMap<String, HashMap<String, String>> = HashMap::with_capacity(requests.len());
  let mut names = Vec::new();
  let mut keys = Vec::new();
  for (rev, file_paths) in requests {
    let contents = result.entry((*rev).to_string()).or_default();
    for file_path in *file_paths {
      // absent files keep this empty content; a file asked for twice at a revision is read once
      if contents.insert(file_path.clone(), String::new()).is_none() {
        names.push(format!("{rev}:{file_path}"));
        keys.push((*rev, file_path.as_str()));
      }
    }
  }
  let texts = read_texts(git_executor, repo_path, &names, MAX_BLOB_BYTES).map_err(|e| anyhow!("Failed to cat-file batch: {e}"))?;
  for ((rev, file_path), text) in keys.into_iter().zip(texts) {
    if let Some(text) = text
      && let Some(contents) = result.get_mut(rev)
    {
      contents.insert(file_path.to_string(), text);
    }
  }
  Ok(result)
}

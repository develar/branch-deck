use crate::commit_files::{FilePatch, MAX_BLOB_BYTES, PATCH_LIMITS, PatchLimits, is_object_id, split_patches, stdin_lines};
use crate::commit_list::Commit;
use crate::object_texts::read_texts;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
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

/// Analyze why a cherry-pick conflict occurred and find missing commits.
/// `merge_base` is the merge base of `original_parent_hash` and `target_commit_hash`, already resolved by the caller.
#[instrument(skip(git_executor, merge_base, conflicting_files), fields(merge_base = %merge_base.id))]
pub fn analyze_conflict(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  original_parent_hash: &str,
  target_commit_hash: &str,
  merge_base: &Commit,
  conflicting_files: &[PathBuf],
) -> Result<ConflictAnalysis> {
  // Find missing commits that touch conflicting files
  let missing_commits = find_missing_commits_for_conflicts(git_executor, repo_path, original_parent_hash, target_commit_hash, conflicting_files)?;

  // Calculate divergence summary
  let divergence_summary = calculate_divergence(git_executor, repo_path, original_parent_hash, target_commit_hash)?;

  Ok(ConflictAnalysis {
    missing_commits,
    merge_base_hash: merge_base.id.clone(),
    merge_base_subject: merge_base.subject.clone(),
    merge_base_message: merge_base.message.clone(),
    merge_base_time: merge_base.committer_timestamp,
    merge_base_author: merge_base.author_name.clone(),
    divergence_summary,
  })
}

/// The environment of a git whose pathspecs are paths: names, never globs (`a*.txt`, `[x].md`) or magic (`:name`)
pub(crate) const LITERAL_PATHSPECS: [(&str, &str); 1] = [("GIT_LITERAL_PATHSPECS", "1")];

/// `\x01` starts a commit, NUL ends a field; `%B` is the last one, so a message may hold anything but NUL
const MISSING_COMMIT_FORMAT: &str = "--format=%x01%H%x00%P%x00%at%x00%ct%x00%an%x00%s%x00%B";

/// Find commits that are in the source history but not in target that touch specific files, newest first, with one
/// `log -z --name-only`: the paths verbatim, the whole message and the parent of each
#[instrument(skip(git_executor, conflicting_files))]
pub(crate) fn find_missing_commits_for_conflicts(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  original_parent_hash: &str,
  target_commit_hash: &str,
  conflicting_files: &[PathBuf],
) -> Result<Vec<MissingCommit>> {
  let file_paths: Vec<String> = conflicting_files.iter().map(|p| p.to_string_lossy().into_owned()).collect();
  if file_paths.is_empty() {
    return Ok(Vec::new());
  }

  let exclude_target = format!("^{target_commit_hash}");
  let mut args = vec![
    "log",
    "-z",
    "--name-only",
    "--no-renames",
    "--no-merges",
    MISSING_COMMIT_FORMAT,
    original_parent_hash,
    &exclude_target,
    "--",
  ];
  args.extend(file_paths.iter().map(String::as_str));
  let output = git_executor.cmd(&args, repo_path).env(&LITERAL_PATHSPECS).bytes()?;
  let output = String::from_utf8(output).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned());
  let conflicting: HashSet<&str> = file_paths.iter().map(String::as_str).collect();
  let (mut missing_commits, parents) = parse_missing_commits(&output, &conflicting);
  if missing_commits.is_empty() {
    return Ok(missing_commits);
  }

  let changes: Vec<CommitChange<'_>> = missing_commits
    .iter()
    .zip(&parents)
    .map(|(commit, parent)| CommitChange {
      id: &commit.hash,
      parent: parent.as_deref(),
      files: &commit.files_touched,
    })
    .collect();
  let mut all_file_diffs = batch_get_file_diffs(git_executor, repo_path, &changes)?;
  for commit in &mut missing_commits {
    debug!(commit_hash = %commit.hash, files_touched = ?commit.files_touched, "found missing commit that touches conflicting files");
    commit.file_diffs = all_file_diffs.remove(&commit.hash).unwrap_or_default();
  }
  Ok(missing_commits)
}

/// The commits of `log -z --name-only` with `MISSING_COMMIT_FORMAT` that change a `conflicting` path, without their
/// diffs, and the first parent of each (`None` for a root commit). A commit is `\x01<id>`, its fields and its message,
/// each ended by NUL, then a newline before the first path and `<path>\0` per changed path
fn parse_missing_commits(output: &str, conflicting: &HashSet<&str>) -> (Vec<MissingCommit>, Vec<Option<String>>) {
  let is_commit_start = |field: &str| field.strip_prefix('\x01').is_some_and(is_object_id);
  let mut commits = Vec::new();
  let mut parents = Vec::new();
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
    parents.push(parent_ids.split(' ').next().filter(|parent| !parent.is_empty()).map(str::to_string));
    commits.push(MissingCommit {
      hash: hash.to_string(),
      subject: subject.to_string(),
      message: message.trim().to_string(),
      author_time: author_time.parse().unwrap_or(0),
      committer_time: committer_time.parse().unwrap_or(0),
      author: author.to_string(),
      files_touched,
      file_diffs: Vec::new(),
      branch: None,
    });
  }
  (commits, parents)
}

/// Find the merge base between two commits
#[instrument(skip(git_executor))]
pub(crate) fn find_merge_base(git_executor: &GitCommandExecutor, repo_path: &str, commit1: &str, commit2: &str) -> Result<String> {
  let args = vec!["merge-base", commit1, commit2];
  let output = git_executor.execute_command(&args, repo_path)?;
  Ok(output.trim().to_string())
}

/// Calculate how the branches have diverged using a single git command
#[instrument(skip(git_executor))]
fn calculate_divergence(git_executor: &GitCommandExecutor, repo_path: &str, source_commit: &str, target_commit: &str) -> Result<DivergenceSummary> {
  // Use git rev-list --left-right --count to get both counts in a single command
  // This shows commits that are reachable from source but not target (left)
  // and commits that are reachable from target but not source (right)
  let range_arg = format!("{source_commit}...{target_commit}");
  let args = vec!["rev-list", "--left-right", "--count", &range_arg];

  let output = git_executor.execute_command(&args, repo_path)?;

  // Parse output format: "<left>\t<right>"
  let parts: Vec<&str> = output.trim().split('\t').collect();
  if parts.len() != 2 {
    return Err(anyhow!("Unexpected rev-list output format: {output}"));
  }

  let source_count = parts[0].parse::<u32>().map_err(|e| anyhow!("Failed to parse source commit count: {e}"))?;
  let target_count = parts[1].parse::<u32>().map_err(|e| anyhow!("Failed to parse target commit count: {e}"))?;

  // The common ancestor distance is the minimum of the two
  let common_ancestor_distance = source_count.min(target_count);

  Ok(DivergenceSummary {
    commits_ahead_in_source: source_count,
    commits_ahead_in_target: target_count,
    common_ancestor_distance,
  })
}

/// A commit whose file diffs `batch_get_file_diffs` reads
#[derive(Debug, Clone, Copy)]
pub(crate) struct CommitChange<'a> {
  pub id: &'a str,
  /// The first parent; `None` for a root commit, whose files are all added
  pub parent: Option<&'a str>,
  pub files: &'a [String],
}

/// The diffs of the files of each commit, by commit: the contents at the commits and their parents from one
/// `cat-file --batch`, the patches from one `log --no-walk --stdin -p`
#[instrument(skip(git_executor, commits), fields(commits = commits.len()))]
pub(crate) fn batch_get_file_diffs(git_executor: &GitCommandExecutor, repo_path: &str, commits: &[CommitChange<'_>]) -> Result<HashMap<String, Vec<FileDiff>>> {
  if commits.is_empty() {
    return Ok(HashMap::new());
  }

  // the files of each commit at the commit, and at each parent the files of the commits whose parent it is
  let mut files_at_parent: HashMap<&str, Vec<String>> = HashMap::new();
  for commit in commits {
    if let Some(parent) = commit.parent {
      let files = files_at_parent.entry(parent).or_default();
      for file in commit.files {
        if !files.contains(file) {
          files.push(file.clone());
        }
      }
    }
  }
  let mut requests: Vec<(&str, &[String])> = commits.iter().map(|commit| (commit.id, commit.files)).collect();
  requests.extend(files_at_parent.iter().map(|(parent, files)| (*parent, files.as_slice())));
  let all_contents = get_files_content_at_revisions(git_executor, repo_path, &requests)?;

  let all_files: Vec<String> = commits.iter().flat_map(|commit| commit.files.iter().cloned()).collect::<HashSet<_>>().into_iter().collect();
  let mut patches = commit_patches(git_executor, repo_path, commits, &all_files)?;

  let empty_map = HashMap::new();
  let mut result = HashMap::with_capacity(commits.len());
  for commit in commits {
    let file_to_diff = patches.remove(commit.id).unwrap_or_default();
    // a root commit's files have no old content
    let parent_contents = commit.parent.and_then(|parent| all_contents.get(parent)).unwrap_or(&empty_map);
    let current_contents = all_contents.get(commit.id).unwrap_or(&empty_map);
    let file_diffs = commit
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
    result.insert(commit.id.to_string(), file_diffs);
  }
  Ok(result)
}

/// The patches of the commits, limited to `files`, by commit and file, with one `log --no-walk --stdin -p` cut as it
/// streams (`split_patches`): git prints the id of each commit on a line of its own before its patch. A commit's patch
/// may name files of the other commits; the caller looks up its own files only
fn commit_patches(git_executor: &GitCommandExecutor, repo_path: &str, changes: &[CommitChange<'_>], files: &[String]) -> Result<HashMap<String, HashMap<String, String>>> {
  let mut commits: Vec<String> = Vec::with_capacity(changes.len());
  for change in changes {
    if !commits.iter().any(|commit| commit == change.id) {
      commits.push(change.id.to_string());
    }
  }
  // `--root`: the patch of a root commit, whatever `log.showRoot` says
  let mut args = vec!["log", "--no-walk=unsorted", "--stdin", "--root", "--format=%H"];
  args.extend(PATCH_ARGS);
  args.push("--");
  args.extend(files.iter().map(String::as_str));
  let input = stdin_lines(&commits);
  let mut patches = HashMap::with_capacity(commits.len());
  let command = git_executor.cmd(&args, repo_path).env(&LITERAL_PATHSPECS).stdin(input.as_bytes());
  split_patches(command, &commits, &CONFLICT_PATCH_LIMITS, |id, files| {
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
      // absent files keep this empty content
      contents.insert(file_path.clone(), String::new());
      names.push(format!("{rev}:{file_path}"));
      keys.push((*rev, file_path.as_str()));
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

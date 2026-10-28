//! The size check of a push: a push that would add more than `branchdeck.pushSizeLimit` of files (50 MiB unless set,
//! uncompressed, `0` or `off` for no check) is refused before it talks to the remote, naming the directory that holds
//! most of it and the largest files. A server refuses such a pack only after the upload, with `unpacker error` and no
//! file named, and what it carries is mostly build output committed by mistake: 2236 files of a cargo `target/`
//! (229 MiB) were refused that way (docs/design/git-operations.md, "Push size").

use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::push_contents::{PushedFile, push_contents};
use std::collections::HashMap;
use std::fmt;
use std::fmt::Write;
use sync_utils::size::{format_size, parse_size};
use tracing::{info, instrument};

/// The git config key of the limit: a size (`50M`, `1G`), `0` or `off` for no check
pub const PUSH_SIZE_LIMIT_CONFIG_KEY: &str = "branchdeck.pushSizeLimit";

/// The limit when no config sets it: far above what the commits of a change add (a few hundred KiB, a few MiB with test
/// data), below what a dump of build output adds
pub const DEFAULT_PUSH_SIZE_LIMIT: u64 = 50 << 20;

/// How many of the largest files a refusal names
const NAMED_FILES: usize = 3;

/// The limit of `branchdeck.pushSizeLimit` as written: `None` for no check (`0`, `off`, `false`, `no`); an error says
/// why the text is neither a size nor off
pub fn parse_push_size_limit(text: &str) -> Result<Option<u64>, String> {
  let text = text.trim();
  if ["off", "false", "no"].iter().any(|off| text.eq_ignore_ascii_case(off)) {
    return Ok(None);
  }
  match parse_size(text) {
    Ok(0) => Ok(None),
    Ok(limit) => Ok(Some(limit)),
    Err(reason) => Err(format!("{reason}; or 0 or off for no limit")),
  }
}

/// A push refused for its size before it talked to the remote: nothing was pushed
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushTooLarge {
  /// The bytes of the files the push adds, uncompressed
  pub size: u64,
  pub limit: u64,
  /// How many files the push adds
  pub files: usize,
  /// The deepest directory that holds most (four fifths) of the bytes, `dir/` with its bytes and files; `None` when the
  /// files spread over the tree
  pub directory: Option<(String, u64, usize)>,
  /// The largest files, largest first
  pub largest: Vec<PushedFile>,
}

impl fmt::Display for PushTooLarge {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "{} Remove the files from the commits (build output?), or raise the push size limit ({PUSH_SIZE_LIMIT_CONFIG_KEY}, \
       Settings → Landing)",
      self.facts()
    )
  }
}

/// `1 file`, `3 files`
fn files(count: usize) -> String {
  if count == 1 { "1 file".to_string() } else { format!("{count} files") }
}

impl std::error::Error for PushTooLarge {}

impl PushTooLarge {
  /// What the push would add and that nothing was pushed, without what to do about it, which the app and `bd` say each
  /// in their words
  pub fn facts(&self) -> String {
    let mut text = format!(
      "The push would add {} in {}, over the push size limit of {}",
      format_size(self.size),
      files(self.files),
      format_size(self.limit)
    );
    if let Some((path, size, count)) = &self.directory {
      let _ = write!(text, ": {} of it in {path} ({})", format_size(*size), files(*count));
    }
    let largest: Vec<String> = self.largest.iter().map(|file| format!("{} ({})", file.path, format_size(file.size))).collect();
    let _ = write!(text, "; the largest: {}. Nothing was pushed.", largest.join(", "));
    text
  }

  /// What the refusal is about, for the `paths` of its error: the directory that holds most of it (`dir/`), then the
  /// largest files
  pub fn paths(&self) -> Vec<String> {
    self
      .directory
      .iter()
      .map(|(path, _, _)| path.clone())
      .chain(self.largest.iter().map(|file| file.path.clone()))
      .collect()
  }
}

/// The files a push of `tips` to `remote` adds (`push_contents`), refused when their bytes exceed `limit`: one `log`
/// and one `cat-file --batch-check`
#[instrument(skip(git, tips), fields(tips = tips.len()))]
pub fn check_push_size(git: &GitCommandExecutor, repository_path: &str, tips: &[&str], remote: &str, limit: u64) -> Result<Option<PushTooLarge>> {
  let files = push_contents(git, repository_path, tips, remote)?;
  let size: u64 = files.iter().map(|file| file.size).sum();
  if size <= limit {
    return Ok(None);
  }
  info!(size, limit, files = files.len(), "the push is too large");
  let mut largest = files.clone();
  largest.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
  largest.truncate(NAMED_FILES);
  Ok(Some(PushTooLarge {
    size,
    limit,
    files: files.len(),
    directory: main_directory(&files, size),
    largest,
  }))
}

/// The deepest directory whose files hold four fifths of `total`: from the root, into the subdirectory that holds them
/// while one does
fn main_directory(files: &[PushedFile], total: u64) -> Option<(String, u64, usize)> {
  let mut found: Option<(String, u64, usize)> = None;
  loop {
    let prefix = found.as_ref().map_or("", |(path, _, _)| path.as_str());
    let mut children: HashMap<&str, (u64, usize)> = HashMap::new();
    for file in files {
      if let Some((child, _)) = file.path.strip_prefix(prefix).and_then(|rest| rest.split_once('/')) {
        let (size, count) = children.entry(child).or_default();
        *size += file.size;
        *count += 1;
      }
    }
    let Some((child, (size, count))) = children.into_iter().max_by(|(a_name, a), (b_name, b)| a.0.cmp(&b.0).then_with(|| b_name.cmp(a_name))) else {
      return found;
    };
    if u128::from(size) * 5 < u128::from(total) * 4 {
      return found;
    }
    found = Some((format!("{prefix}{child}/"), size, count));
  }
}

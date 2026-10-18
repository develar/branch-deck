//! The objects of many names (an object id, `<rev>:<path>`, a ref) from one `git cat-file --batch --buffer -Z`, read as
//! the output streams, or only their ids and types from one `--batch-check` (`object_ids_and_types`). Names and entries
//! are NUL-terminated (`-Z`), so a name may hold a space or a newline; the content of an object over the caller's limit
//! is skipped as it arrives, never buffered, so memory holds one object within the limit, not the output.

use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use std::ops::ControlFlow;
use tracing::debug;

/// The limit of a reader that takes every object whole: commits and the files of our own stores, small by construction
pub const NO_LIMIT: usize = usize::MAX;

/// What `cat-file --batch` answered for one name
#[derive(Debug, PartialEq, Eq)]
pub enum BatchEntry<'a> {
  /// The id, the type (`blob`, `tree`, `commit`, `tag`) and the bytes as git stores them
  Object { id: &'a str, kind: &'a str, content: Vec<u8> },
  /// An object larger than the limit: its bytes were skipped unread
  TooLarge { id: &'a str, kind: &'a str, size: usize },
  /// `<name> missing` or `<name> ambiguous`: git cannot resolve the name
  Missing,
}

/// Read `names` with one `cat-file --batch --buffer -Z`: `on_entry` gets the index of each name and its entry, in this
/// order, as the output streams; an object larger than `max_bytes` is `TooLarge`. An error of `on_entry` stops git and
/// is the result. A name that holds a NUL is an error
pub fn for_each_object<S: AsRef<str>>(
  git: &GitCommandExecutor,
  repository_path: &str,
  names: &[S],
  max_bytes: usize,
  on_entry: impl FnMut(usize, BatchEntry<'_>) -> Result<()>,
) -> Result<()> {
  if names.is_empty() {
    return Ok(());
  }
  let input = nul_terminated(names)?;
  let mut parser = BatchParser::new(names.len(), max_bytes, on_entry);
  git
    .cmd(&["cat-file", "--batch", "--buffer", "-Z"], repository_path)
    .stdin(&input)
    .stream(|chunk| parser.push(chunk).map(ControlFlow::Continue))?;
  parser.finish()
}

/// The type of the object each of `names` names (`blob`, `tree`, `commit`, `tag`, and `submodule` for a gitlink of a
/// `<rev>:<path>` to a commit this repository lacks), `None` for a name git cannot resolve (`missing`, `ambiguous`):
/// `object_ids_and_types` without the ids
pub fn object_types<S: AsRef<str>>(git: &GitCommandExecutor, repository_path: &str, names: &[S]) -> Result<Vec<Option<&'static str>>> {
  Ok(
    object_ids_and_types(git, repository_path, names)?
      .into_iter()
      .map(|entry| entry.map(|(_, kind)| kind))
      .collect(),
  )
}

/// The id and the type of the object each of `names` names (as `object_types`; the id of a gitlink is the commit it
/// links), `None` for a name git cannot resolve: one `cat-file --batch-check='%(objectname) %(objecttype)' -Z`, in the
/// order of `names`. A name that holds a NUL is an error
pub fn object_ids_and_types<S: AsRef<str>>(git: &GitCommandExecutor, repository_path: &str, names: &[S]) -> Result<Vec<Option<(String, &'static str)>>> {
  if names.is_empty() {
    return Ok(Vec::new());
  }
  let input = nul_terminated(names)?;
  let output = git
    .cmd(&["cat-file", "--batch-check=%(objectname) %(objecttype)", "-Z"], repository_path)
    .stdin(&input)
    .bytes()?;
  // `<id> <type>` each, or `<name> missing` (`ambiguous`), the name maybe with a space; a gitlink is `<id> submodule`
  let entries: Vec<Option<(String, &'static str)>> = output
    .split(|&byte| byte == 0)
    .take(names.len())
    .map(|reply| {
      let space = reply.iter().rposition(|&byte| byte == b' ')?;
      let (id, kind) = (&reply[..space], &reply[space + 1..]);
      let kind = match kind {
        b"blob" => "blob",
        b"tree" => "tree",
        b"commit" => "commit",
        b"tag" => "tag",
        b"submodule" => "submodule",
        _ => return None,
      };
      Some((std::str::from_utf8(id).ok()?.to_string(), kind))
    })
    .collect();
  if entries.len() != names.len() {
    bail!("cat-file --batch-check answered {} of {} names", entries.len(), names.len());
  }
  Ok(entries)
}

/// `names` for the stdin of a `-Z` batch: each followed by a NUL, which none may hold
fn nul_terminated<S: AsRef<str>>(names: &[S]) -> Result<Vec<u8>> {
  let mut input = Vec::with_capacity(names.iter().map(|name| name.as_ref().len() + 1).sum());
  for name in names {
    let name = name.as_ref();
    if name.contains('\0') {
      bail!("An object name holds a NUL: {name:?}");
    }
    input.extend_from_slice(name.as_bytes());
    input.push(0);
  }
  Ok(input)
}

/// What `map` makes of the entry of each of `names`, in this order (`for_each_object`)
pub fn read_objects<S: AsRef<str>, T>(git: &GitCommandExecutor, repository_path: &str, names: &[S], max_bytes: usize, mut map: impl FnMut(BatchEntry<'_>) -> T) -> Result<Vec<T>> {
  let mut values = Vec::with_capacity(names.len());
  for_each_object(git, repository_path, names, max_bytes, |_, entry| {
    values.push(map(entry));
    Ok(())
  })?;
  Ok(values)
}

/// Splits the output of `cat-file --batch -Z` into its entries as the chunks arrive: `<oid> <type> <size>\0`, the
/// content and a NUL per object, `<name> missing\0` (`ambiguous`) per name git cannot resolve
struct BatchParser<F> {
  /// The entries the output must have: one per name
  names: usize,
  max_bytes: usize,
  on_entry: F,
  /// The entries handed over
  entries: usize,
  /// The header being read, then the header of the object whose content is being read
  header: Vec<u8>,
  state: State,
}

enum State {
  Header,
  /// The content of an object within the limit, `remaining` bytes still to come
  Content {
    content: Vec<u8>,
    remaining: usize,
  },
  /// The content of an object over the limit, skipped
  Skip {
    remaining: usize,
  },
  /// The NUL after the content
  End,
}

impl<F: FnMut(usize, BatchEntry<'_>) -> Result<()>> BatchParser<F> {
  const fn new(names: usize, max_bytes: usize, on_entry: F) -> Self {
    Self {
      names,
      max_bytes,
      on_entry,
      entries: 0,
      header: Vec::new(),
      state: State::Header,
    }
  }

  fn push(&mut self, mut chunk: &[u8]) -> Result<()> {
    while let Some(&first) = chunk.first() {
      match std::mem::replace(&mut self.state, State::End) {
        State::Header => {
          let Some(end) = memchr::memchr(0, chunk) else {
            // a header is short: the name of a missing object at most
            self.header.extend_from_slice(chunk);
            self.state = State::Header;
            return Ok(());
          };
          self.header.extend_from_slice(&chunk[..end]);
          chunk = &chunk[end + 1..];
          self.start_entry()?;
        }
        State::Content { mut content, remaining } => {
          let (taken, rest) = chunk.split_at(remaining.min(chunk.len()));
          content.extend_from_slice(taken);
          chunk = rest;
          if taken.len() == remaining {
            self.hand_over_object(content)?;
          } else {
            self.state = State::Content {
              content,
              remaining: remaining - taken.len(),
            };
          }
        }
        State::Skip { remaining } => {
          let skipped = remaining.min(chunk.len());
          chunk = &chunk[skipped..];
          if skipped < remaining {
            self.state = State::Skip { remaining: remaining - skipped };
          }
        }
        State::End => {
          if first != 0 {
            bail!("cat-file content of {:?} is not followed by a NUL", String::from_utf8_lossy(&self.header));
          }
          chunk = &chunk[1..];
          self.header.clear();
          self.state = State::Header;
        }
      }
    }
    Ok(())
  }

  /// The header in `header` is whole: a missing name is handed over at once, an object over the limit too before its
  /// content is skipped, the content of another one is read
  fn start_entry(&mut self) -> Result<()> {
    if self.entries == self.names {
      bail!("cat-file printed more than {} entries", self.names);
    }
    let header = std::str::from_utf8(&self.header).map_err(|e| anyhow!("cat-file header is not UTF-8: {e}"))?;
    // the name of a missing object may hold spaces
    if header.ends_with(" missing") || header.ends_with(" ambiguous") {
      debug!(header, "object missing");
      (self.on_entry)(self.entries, BatchEntry::Missing)?;
      self.entries += 1;
      self.header.clear();
      self.state = State::Header;
      return Ok(());
    }
    let (id, kind, size) = parse_header(header)?;
    if size > self.max_bytes {
      (self.on_entry)(self.entries, BatchEntry::TooLarge { id, kind, size })?;
      self.entries += 1;
      self.state = State::Skip { remaining: size };
    } else if size == 0 {
      self.hand_over_object(Vec::new())?;
    } else {
      self.state = State::Content {
        content: Vec::with_capacity(size),
        remaining: size,
      };
    }
    Ok(())
  }

  fn hand_over_object(&mut self, content: Vec<u8>) -> Result<()> {
    let header = std::str::from_utf8(&self.header).map_err(|e| anyhow!("cat-file header is not UTF-8: {e}"))?;
    let (id, kind, _) = parse_header(header)?;
    (self.on_entry)(self.entries, BatchEntry::Object { id, kind, content })?;
    self.entries += 1;
    self.state = State::End;
    Ok(())
  }

  /// Fails unless the output held an entry per name, the last one whole
  fn finish(self) -> Result<()> {
    if !matches!(self.state, State::Header) || !self.header.is_empty() || self.entries != self.names {
      bail!("truncated cat-file output: {} of {} entries", self.entries, self.names);
    }
    Ok(())
  }

  /// The bytes of content held for the entry being read, for the tests of the limit
  #[cfg(test)]
  const fn content_held(&self) -> usize {
    match &self.state {
      State::Content { content, .. } => content.capacity(),
      _ => 0,
    }
  }
}

/// `<oid> <type> <size>`
fn parse_header(header: &str) -> Result<(&str, &str, usize)> {
  let mut fields = header.splitn(3, ' ');
  match (fields.next(), fields.next(), fields.next().and_then(|size| size.parse().ok())) {
    (Some(id), Some(kind), Some(size)) => Ok((id, kind, size)),
    _ => Err(anyhow!("Failed to parse git cat-file header: {header:?}")),
  }
}

#[cfg(test)]
#[path = "cat_file_batch_test.rs"]
mod cat_file_batch_test;

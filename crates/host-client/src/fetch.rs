//! The files of a run (spec/cli/host.spec.md, "The fetch, the outputs and the changes"): `Fetch` copies them into the
//! local checkout, and `Forget` removes the run on the agent.
//!
//! Each file goes to `<name>.bd-host-part` first, and only a file of the size that the agent named takes its name. So a
//! cut connection leaves no file that looks whole. A stream writes at most the size that the agent named, and the rest
//! is read and dropped. A fetch that did not copy every file fails with `fetch_failed`, and the run then keeps its
//! stage on the agent for another `bd-host fetch`.
//!
//! No write follows a local link: a target with a link in a parent directory below the checkout is refused, and the
//! part is a new file that the open makes itself (`create_new`, and `O_NOFOLLOW` on Unix). A part that a cut fetch left
//! goes first.
//!
//! The host chooses the paths, so the client takes only what the local record of the run asked for (`record`): a
//! staged path below a `fetch` directory of the record, a file of an output that the record names, and the changes when
//! the record asked for them. It never takes a path inside a `.git` directory: a hook or a `core.fsmonitor` there would
//! run code here. A file system that ignores the case, such as APFS, reads `.GIT` as `.git`, and Windows drops a
//! trailing dot or space, so these spellings count too.
//!
//! A file of an output replaces the local copy of the output: the first file of an output removes the local file or
//! directory of its path. The patch of the changes goes to the patch file of the run, in the git directory

use crate::Client;
use crate::git::Checkout;
use crate::host::Host;
use crate::record::{RunFiles, RunRecord};
use crate::run::{Fetched, done};
use host_link::channel::{Channel, LinkError};
use host_link::proto::{FileKind, Reply, Request, is_run_id};
use host_link::refusal::{Code, Refusal};
use std::collections::BTreeSet;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use sync_utils::fs_retry;

/// The suffix of a file that is not complete yet
const PART_SUFFIX: &str = ".bd-host-part";

impl Client<'_> {
  /// Copies the files of a run into the checkout of `dir`, as its local record asks: the staged files below its `fetch`
  /// directories, its outputs and its changes. An explicit `fetch` replaces the directories of the record. Without a
  /// record, only the staged files below an explicit `fetch` come, and no `fetch` refuses with `usage`. Answers the
  /// copied paths, relative to the checkout root with `/` separators
  pub fn fetch(&self, alias: &str, run: &str, dir: &Path, fetch: &[String]) -> Result<Vec<String>, Refusal> {
    if !is_run_id(run) {
      return Err(Refusal::new(Code::Usage, format!("{run:?} cannot name a run")));
    }
    let checkout = Checkout::open(&self.git, dir)?;
    let files = RunFiles::new(&checkout.runs_dir())?;
    let record = match files.read(run)? {
      Some(record) if fetch.is_empty() => record,
      Some(record) => RunRecord { fetch: fetch.to_vec(), ..record },
      None if fetch.is_empty() => {
        return Err(Refusal::new(
          Code::Usage,
          format!(
            "{} has no record of the run {run}, so the files to take are unknown: `--fetch <dir>` takes the staged files below a directory",
            files.dir().display()
          ),
        ));
      }
      None => RunRecord {
        fetch: fetch.to_vec(),
        ..RunRecord::default()
      },
    };
    let mut host = self.host(alias)?;
    let fetched = self.fetch_run(&mut host, run, checkout.root(), &record, &files.patch(run))?;
    if let Some(changes) = &fetched.changes {
      (self.say)(&format!("bd-host: the changes of the run {run} are in {}: `git apply` applies them", changes.display()));
    }
    Ok(fetched.files)
  }

  /// Removes a run and its stage on the agent
  pub fn forget(&self, alias: &str, run: &str) -> Result<String, Refusal> {
    let mut host = self.host(alias)?;
    self.forget_on(&mut host, run)
  }

  pub(crate) fn forget_on(&self, host: &mut Host, run: &str) -> Result<String, Refusal> {
    done(host, &Request::Forget { run: run.to_owned() }, self.timing.read)
  }

  /// Copies the files of a run that `record` asks for: the staged files below `root`, the outputs in place of their
  /// local copies, and the patch of the changes to `patch`. Each other file is skipped, and fails the fetch
  pub(crate) fn fetch_run(&self, host: &mut Host, run: &str, root: &Path, record: &RunRecord, patch: &Path) -> Result<Fetched, Refusal> {
    let (mut channel, mut reply) = host.request(&Request::Fetch { run: run.to_owned() }, self.timing.read)?;
    let mut fetched = Fetched::default();
    let mut missed = Vec::new();
    let mut replaced = BTreeSet::new();
    loop {
      match reply {
        Reply::File { path, size, executable, kind } => {
          let target = match &kind {
            FileKind::Staged => local_target(root, &path, &record.fetch),
            FileKind::Output { root: output } if !record.outputs.contains(output) => Err(format!("the run did not ask for the output {output}")),
            FileKind::Output { root: output } => output_target(root, output, &path).inspect(|_| {
              if replaced.insert(output.clone())
                && let Err(error) = remove_output(root, output)
              {
                (self.say)(&format!("bd-host: cannot remove the local output {output}: {error}"));
              }
            }),
            FileKind::Changes if record.changes => Ok(patch.to_path_buf()),
            FileKind::Changes => Err("the run did not ask for its changes".to_owned()),
          };
          // The client chose the file of the patch, so only the paths of the host have parents to check
          let base = if kind == FileKind::Changes { patch.parent().unwrap_or(patch) } else { root };
          match target {
            Ok(target) => match receive_file(&mut channel, base, &target, size, executable) {
              Ok(()) if kind == FileKind::Changes => fetched.changes = Some(target),
              Ok(()) => fetched.files.push(path),
              Err(Received::Link(error)) => return Err(host.link_refusal(&error)),
              Err(Received::Local(said)) => {
                (self.say)(&format!("bd-host: cannot write {}: {said}", target.display()));
                missed.push(path);
              }
            },
            Err(said) => {
              channel.recv_stream(&mut io::sink()).map_err(|error| host.link_refusal(&error))?;
              (self.say)(&format!("bd-host: skipped {path:?} of the run {run}: {said}"));
              missed.push(path);
            }
          }
        }
        Reply::Fetched { .. } => {
          channel.close();
          if missed.is_empty() {
            return Ok(fetched);
          }
          return Err(Refusal::new(
            Code::FetchFailed,
            format!(
              "{} of {} files of the run {run} did not come: {}",
              missed.len(),
              missed.len() + fetched.files.len() + usize::from(fetched.changes.is_some()),
              missed.join(", ")
            ),
          ));
        }
        Reply::Refused { code, message } => return Err(host.agent_refusal(&code, &message)),
        reply => return Err(host.unexpected(&reply)),
      }
      reply = host.recv(&mut channel, self.timing.read)?;
    }
  }
}

/// The local file of a file of the output `output`: `output` itself or a path below it, below the checkout root
pub(crate) fn output_target(root: &Path, output: &str, path: &str) -> Result<PathBuf, String> {
  plain_target(root, output)?;
  let below = path == output || path.strip_prefix(output).is_some_and(|rest| rest.starts_with('/'));
  if !below {
    return Err(format!("the path is not in its output {output}"));
  }
  plain_target(root, path)
}

/// Removes the local copy of the output `output`. A link in a parent directory refuses, so the removal stays below the
/// checkout
fn remove_output(root: &Path, output: &str) -> io::Result<()> {
  let path = root.join(output);
  check_parents(root, &path, false)?;
  remove_local(&path)
}

/// Checks each directory between `base` and `target`: no one may be a link, so that a write or a removal at `target`
/// stays below `base`. With `create`, a missing directory is made. Without it, a missing directory ends the check
fn check_parents(base: &Path, target: &Path, create: bool) -> io::Result<()> {
  let Ok(below) = target.strip_prefix(base) else {
    return Err(io::Error::other(format!("{} is not below {}", target.display(), base.display())));
  };
  let parts: Vec<_> = below.components().collect();
  let mut dir = base.to_path_buf();
  for part in &parts[..parts.len().saturating_sub(1)] {
    dir.push(part);
    match std::fs::symlink_metadata(&dir) {
      Ok(metadata) if metadata.file_type().is_symlink() => return Err(io::Error::other(format!("{} is a link", dir.display()))),
      Ok(metadata) if metadata.is_dir() => {}
      Ok(_) => return Err(io::Error::other(format!("{} is not a directory", dir.display()))),
      Err(error) if error.kind() == io::ErrorKind::NotFound && create => make_dir(&dir)?,
      Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
      Err(error) => return Err(error),
    }
  }
  Ok(())
}

/// Makes one directory, so the check of the next part sees a link that another process put there
#[expect(clippy::create_dir, reason = "one level at a time: each parent is checked for a link before the next one")]
fn make_dir(dir: &Path) -> io::Result<()> {
  std::fs::create_dir(dir)
}

/// Removes a local file, link or directory. A missing one is no failure
fn remove_local(path: &Path) -> io::Result<()> {
  let removed = match std::fs::symlink_metadata(path) {
    Ok(metadata) if metadata.is_dir() => fs_retry::remove_dir_all(path),
    Ok(_) => fs_retry::remove_file(path),
    Err(error) => Err(error),
  };
  match removed {
    Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
    other => other,
  }
}

/// Why a fetched file did not reach its target
enum Received {
  /// The channel failed: no later file can come
  Link(LinkError),
  /// This file failed here, and the channel stays in step
  Local(String),
}

/// Writes the byte stream that comes next into `<target>.bd-host-part`, checks its size, then renames it to `target`.
/// The parent directories of `target` below `base` are made, and a link among them refuses. Only `size` bytes go into
/// the part. A local failure still reads the stream to its end, so the channel stays in step. A cut stream removes the
/// part
fn receive_file(channel: &mut Channel, base: &Path, target: &Path, size: u64, executable: bool) -> Result<(), Received> {
  let part = part_path(target);
  let opened = check_parents(base, target, true).and_then(|()| open_part(&part));
  let file = match opened {
    Ok(file) => file,
    Err(error) => {
      channel.recv_stream(&mut io::sink()).map_err(Received::Link)?;
      return Err(Received::Local(error.to_string()));
    }
  };
  let mut writer = Capped {
    inner: BufWriter::new(file),
    left: size,
  };
  let received = channel.recv_stream(&mut writer);
  let flushed = writer.inner.flush();
  drop(writer);
  let checked = match (received, flushed) {
    (Err(error), _) => Err(Received::Link(error)),
    (Ok(_), Err(error)) => Err(Received::Local(error.to_string())),
    (Ok(bytes), Ok(())) if bytes != size => Err(Received::Local(format!("{bytes} bytes came, not {size}"))),
    (Ok(_), Ok(())) => mark_executable(&part, executable)
      .and_then(|()| fs_retry::rename(&part, target))
      .map_err(|error| Received::Local(error.to_string())),
  };
  if checked.is_err() {
    let _ = fs_retry::remove_file(&part);
  }
  checked
}

/// Makes the part as a new file. A part that a cut fetch left goes first: a file or a link, never what a link names
fn open_part(part: &Path) -> io::Result<std::fs::File> {
  match fs_retry::remove_file(part) {
    Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
    _ => {}
  }
  let mut options = std::fs::OpenOptions::new();
  options.write(true).create_new(true);
  {
    use std::os::unix::fs::OpenOptionsExt as _;
    options.custom_flags(libc::O_NOFOLLOW);
  }
  options.open(part)
}

/// A writer that passes the first `left` bytes and drops the rest, so a stream over its size cannot fill the disk
struct Capped<W> {
  inner: W,
  left: u64,
}

impl<W: Write> Write for Capped<W> {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    let taken = usize::try_from(self.left).map_or(buf.len(), |left| left.min(buf.len()));
    self.inner.write_all(&buf[..taken])?;
    self.left -= taken as u64;
    Ok(buf.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    self.inner.flush()
  }
}

/// Marks a file executable for its owner, the group and others to read and run
fn mark_executable(path: &Path, executable: bool) -> io::Result<()> {
  use std::os::unix::fs::PermissionsExt as _;
  if executable {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
  } else {
    Ok(())
  }
}

fn part_path(target: &Path) -> PathBuf {
  let mut name = target.as_os_str().to_owned();
  name.push(PART_SUFFIX);
  PathBuf::from(name)
}

/// The local file of a staged path: below the checkout root, below one of the `fetch` directories, and outside each
/// `.git` directory. Answers why else
pub(crate) fn local_target(root: &Path, path: &str, fetch: &[String]) -> Result<PathBuf, String> {
  let target = plain_target(root, path)?;
  let below_fetch = fetch
    .iter()
    .any(|dir| path.strip_prefix(dir.trim_end_matches('/')).is_some_and(|rest| rest.starts_with('/')));
  if !below_fetch {
    return Err(format!("the path is in none of the fetch directories {}", fetch.join(", ")));
  }
  Ok(target)
}

/// The local file of a path that the host chose: a plain relative path below the checkout root, outside each `.git`
/// directory. Answers why else
fn plain_target(root: &Path, path: &str) -> Result<PathBuf, String> {
  let parts: Vec<&str> = path.split('/').collect();
  let plain = |part: &&str| !part.is_empty() && *part != "." && *part != ".." && !part.contains(['\\', ':']);
  if !parts.iter().all(plain) || path.ends_with(PART_SUFFIX) {
    return Err("the path is not a plain relative path".to_owned());
  }
  if parts.iter().any(|part| is_git_dir(part)) {
    return Err("the path is inside a .git directory".to_owned());
  }
  Ok(parts.iter().fold(root.to_path_buf(), |target, part| target.join(part)))
}

/// Whether a path part names a `.git` directory on some file system: any case, a trailing dot or space that Windows
/// drops, and `git~1`, the short name of `.git` on NTFS
fn is_git_dir(part: &str) -> bool {
  let name = part.trim_end_matches(['.', ' ']);
  name.eq_ignore_ascii_case(".git") || name.eq_ignore_ascii_case("git~1")
}

#[cfg(test)]
#[path = "fetch_test.rs"]
mod fetch_test;

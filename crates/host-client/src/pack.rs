//! The thin pack of a run or of a base (openspec/specs/cli/host/spec.md, "run"). git builds it into a private file first
//! (`git::Checkout::pack`), so a slow link never stalls git and a stuck git never holds a connection. The client opens
//! the connection of the pack before the build, and sends an empty chunk every `HEARTBEAT_SECONDS` of the protocol
//! while git works on the blocking pool: the supervisor of the run then knows that its client is alive. Then the client
//! sends the file on that connection.
//!
//! A host that has none of the candidate commits would need the whole history, which is no thin pack. The client
//! refuses that with `behind` before it starts the run, and starts no pack

use crate::Client;
use crate::git::{Checkout, Pack};
use crate::host::Host;
use host_link::channel::{Channel, LinkError};
use host_link::proto::{PackTarget, Reply, Request};
use host_link::refusal::{Code, Refusal};
use std::fs::File;
use std::io::{self, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use sync_utils::size::format_size;
use sync_utils::span::in_current_span;
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep};

/// How often the upload says how far it is
const PROGRESS_EVERY: Duration = Duration::from_secs(10);

/// What a pack goes to: the commit of the checkout, the repository on the host, and the ref of the host
pub(crate) struct PackOf<'a> {
  pub(crate) checkout: &'a Checkout,
  pub(crate) repo: &'a str,
  pub(crate) commit: &'a str,
  pub(crate) target: PackTarget,
}

/// The pack of a target against the commits that the host has: what `plan_pack` answers
pub(crate) struct PackPlan<'a> {
  present: &'a [String],
}

/// How long the two halves of a sent pack took
pub(crate) struct PackSteps {
  pub(crate) build: Duration,
  pub(crate) upload: Duration,
}

impl Client<'_> {
  /// Opens the connection of the pack, builds the pack while an empty chunk keeps the connection alive, and sends it.
  /// The agent points the ref of the target at the commit. `steps` takes the step lines. Ctrl+C stops the build, and
  /// the upload at the next chunk
  pub(crate) async fn send_pack(&self, host: &mut Host, pack: &PackOf<'_>, plan: &PackPlan<'_>, steps: &(dyn Fn(&str) + Sync)) -> Result<PackSteps, Refusal> {
    let alias = host.alias().to_owned();
    steps(&format!("> {alias}: pack the commits that {alias} lacks"));
    let building = Instant::now();
    let (mut channel, _) = host.connect().await?;
    let request = Request::Pack {
      repo: pack.repo.to_owned(),
      commit: pack.commit.to_owned(),
      target: pack.target.clone(),
    };
    channel.send(&request).await.map_err(|error| host.link_refusal(&error))?;
    let built = self.build_beside(&mut channel, pack, plan.present).await;
    let file = match built.map_err(|failed| match failed {
      Built::Refused(refusal) => refusal,
      Built::Link(error) => host.link_refusal(&error),
    }) {
      Ok(file) => file,
      Err(refusal) => {
        channel.close().await;
        return Err(refusal);
      }
    };
    let (build, size) = (self.took(building), file.bytes);
    steps(&format!("> {alias}: pack of {} commits, {} ready", file.commits, format_size(size)));
    let uploading = Instant::now();
    self.stream_pack(host, &mut channel, &file, steps).await?;
    steps(&format!("> {alias}: sent {}", format_size(size)));
    Ok(PackSteps {
      build,
      upload: self.took(uploading),
    })
  }

  /// Builds the pack on the blocking pool, and sends an empty chunk on `channel` at each keepalive time meanwhile.
  /// Ctrl+C and a failed chunk stop the build: git polls the stop. A test holds the build back by `Client::hold_pack`,
  /// and the chunks go out meanwhile too
  async fn build_beside(&self, channel: &mut Channel, pack: &PackOf<'_>, present: &[String]) -> Result<Pack, Built> {
    let stop = Arc::new(AtomicBool::new(false));
    let tick = self.timing.interrupt_poll.min(self.timing.keepalive);
    let held = Instant::now();
    let mut sent = Instant::now();
    let mut failed: Option<LinkError> = None;
    let mut building: Option<JoinHandle<Result<Pack, Refusal>>> = None;
    let built = loop {
      if building.is_none() && (held.elapsed() >= self.hold_pack || stop.load(Ordering::Acquire)) {
        let (checkout, commit, present, stop) = (pack.checkout.clone(), pack.commit.to_owned(), present.to_vec(), Arc::clone(&stop));
        building = Some(tokio::task::spawn_blocking(in_current_span(move || {
          checkout.pack(&commit, &present, &|| stop.load(Ordering::Acquire))
        })));
      }
      if let Some(handle) = &mut building {
        tokio::select! {
          built = handle => break built.unwrap_or_else(|_| Err(Refusal::new(Code::GitFailed, "the build of the pack failed inside the client"))),
          () = sleep(tick) => {}
        }
      } else {
        sleep(tick).await;
      }
      if self.interrupted() {
        stop.store(true, Ordering::Release);
      }
      if failed.is_none() && sent.elapsed() >= self.timing.keepalive {
        match channel.send_keepalive().await {
          Ok(()) => sent = Instant::now(),
          Err(error) => {
            failed = Some(error);
            stop.store(true, Ordering::Release);
          }
        }
      }
    };
    // A pack that is not sent goes with its guard here
    match (failed, built) {
      (Some(error), _) => Err(Built::Link(error)),
      (None, Err(refusal)) => Err(Built::Refused(refusal)),
      (None, Ok(_)) if self.interrupted() => Err(Built::Refused(interrupted())),
      (None, Ok(pack)) => Ok(pack),
    }
  }

  /// Sends the built pack on its connection, and waits for `Packed`. `steps` takes the progress lines
  async fn stream_pack(&self, host: &Host, channel: &mut Channel, file: &Pack, steps: &(dyn Fn(&str) + Sync)) -> Result<(), Refusal> {
    let alias = host.alias().to_owned();
    let stop = || self.interrupted();
    let mut reader = Upload {
      file: File::open(&file.path).map_err(|error| Refusal::new(Code::GitFailed, format!("cannot open the pack: {error}")))?,
      stop: &stop,
    };
    let mut last_said = Instant::now();
    let sent = channel
      .send_stream(&mut reader, &mut |bytes| {
        if last_said.elapsed() >= PROGRESS_EVERY {
          last_said = Instant::now();
          steps(&format!("> {alias}: sent {} of {}", format_size(bytes), format_size(file.bytes)));
        }
      })
      .await;
    if let Err(error) = sent {
      if self.interrupted() {
        return Err(Refusal::new(Code::Interrupted, "Ctrl+C stopped the upload of the pack"));
      }
      // An agent that refuses the pack replies and closes; its reply says more than the failed write
      return Err(match host.recv(channel, self.timing.connect).await {
        Ok(Reply::Refused { code, message }) => host.agent_refusal(&code, &message),
        _ => host.link_refusal(&error),
      });
    }
    match host.recv(channel, self.timing.read).await? {
      Reply::Packed => Ok(()),
      Reply::Refused { code, message } => Err(host.agent_refusal(&code, &message)),
      reply => Err(host.unexpected(&reply)),
    }
  }
}

/// The pack of the objects of the commit that the host lacks. Refuses with `behind` when the host has none of the
/// candidates; the message gives the commit count of the history, when the client knows it
pub(crate) fn plan_pack<'a>(alias: &str, present: &'a [String], history: Option<u64>) -> Result<PackPlan<'a>, Refusal> {
  if present.is_empty() {
    let count = history.map_or_else(String::new, |count| format!(" of {count} commits"));
    return Err(Refusal::new(
      Code::Behind,
      format!(
        "{alias} has none of the commits near HEAD, so the pack would hold the whole history{count}. Bring the master of the repository on {alias} near the local master first"
      ),
    ));
  }
  Ok(PackPlan { present })
}

/// Why the build of a pack gave no file
enum Built {
  /// git failed, or Ctrl+C stopped it
  Refused(Refusal),
  /// An empty chunk failed: the connection is gone
  Link(LinkError),
}

fn interrupted() -> Refusal {
  Refusal::new(Code::Interrupted, "Ctrl+C stopped the build of the pack")
}

/// The pack file as the channel reads it. Ctrl+C ends the read with an error, so the upload stops at the next chunk
struct Upload<'a> {
  file: File,
  stop: &'a (dyn Fn() -> bool + Sync),
}

impl Read for Upload<'_> {
  fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
    if (self.stop)() {
      return Err(io::Error::other("Ctrl+C"));
    }
    self.file.read(buffer)
  }
}

#[cfg(test)]
#[path = "pack_test.rs"]
mod pack_test;

//! A run on a host, and the requests of one reply (openspec/specs/cli/host/spec.md, "run" and "follow, cancel, fetch, forget,
//! runs").
//!
//! A run:
//!
//! 1. Reads the snapshot of HEAD in process (`git`): the commit, the candidate commits that the host may have. Then it
//!    refuses a checkout with local changes, because only the commit reaches the host, and meanwhile it asks the host
//!    which of the candidates it has (`Have`). The candidates include the base that the last base sync of the affinity
//!    recorded. A host that has none of them refuses with `behind` here. With `RunOptions::working_tree` it takes the
//!    commit of the working tree instead, and asks the host after it. With `RunOptions::repo_by_name` the first `Have`
//!    goes to the repository of the same name as the local one beside the repository of the host file, and to the one
//!    of the host file when the host lacks it.
//! 2. Reads `[host]` of the commit: the outputs to take back, and whether the run is heavy (`--heavy`, else the first
//!    pattern of `heavy` that matches the command). Writes the run record (`record`) and starts the run (`Start`), a
//!    heavy one with `RunSpec::heavy` and its line. An agent without a feature that `[host]` needs
//!    (`RepoConfig::needs_features`: `heavy` needs `heavy`) would refuse every run of the commit, so the start refuses
//!    with `bad_config` before it sends anything; a `--heavy` run of a commit without the key runs on such an agent with
//!    a line that names the upgrade. The run lives on the agent, not on the connection. Ctrl+C before this step stops the local work, and
//!    nothing runs on the host. A start whose reply does not come is cancelled, so the host does not wait for its pack.
//! 3. Follows the output (`Follow`) beside the other steps from the start on, so the steps of the host show while the
//!    pack builds. The bytes of each piece of output, with its line end when it has one, go to the output sink as they
//!    come (`Client::with_output`). A connection that is quiet for 120 s is lost: the client connects again from the
//!    last offset, with a backoff, for up to 10 min after the last reply of the agent. `Reply::Phases` before the end
//!    gives the steps of the host.
//! 4. Meanwhile it builds the thin pack of the commits that the host lacks, while the host takes the slot, and sends it
//!    (`Pack` with `PackTarget::Run`) on a connection of its own (`pack`). When the pack fails, the client cancels the
//!    run, stops the follow, and refuses with the code of the pack.
//! 5. After the pack, the client syncs the base of the next packs (`base`) beside the follow.
//! 6. Takes the lock of the end of the run in the checkout (`RunFiles::lock`), so a `brd-host wait` of the same run
//!    never fetches or applies it a second time; an end that such a wait finished first is the answer. Then it copies
//!    the staged files of the `fetch` directories, the outputs and the changes into the local checkout (`Fetch`), and
//!    forgets the run on the agent (`Forget`: its files go, its end and the tail of its log stay). The fetch takes only
//!    what the run record asked for. The client forgets the run only when it saw the end of the run and every file
//!    came. Otherwise the run keeps its files on the agent, and the client names `brd-host fetch` and `brd-host forget`.
//! 7. Waits for the base sync. With `RunOptions::apply`, it applies the patch of the changes of the command to the
//!    working tree (the files only, not the index). It writes the end of the run (`<run>.end.json`) and lets the lock
//!    go. Then it answers the steps of the run with their durations.
//!
//! A refusal carries the exit of `brd-host` by how far the run came (`host_link::refusal::Reached`): a refusal before the
//! command exits 75, so the caller can run it locally instead.
//!
//! With `RunOptions::max` a run follows no output: after its pack it waits for the end as `brd-host wait` does
//! (`crate::wait`), at most up to `max` from the start of the call, and leaves past it with `still_running` while the
//! run goes on, without a cancel, dropping the base sync. At the end it finishes the run as a wait does: the output into
//! `<run>.log`, the files, the changes, the end and the forget, then the summary.
//!
//! Ctrl+C after the start, the build of the pack included, cancels the run on the host. The first press sends `Cancel`
//! beside the follow and stops the pack, and the output follows up to its end. The second press stops the follow at
//! once. A press after the end of the run stops the base sync, and its connection with it.
//!
//! The steps that go on beside each other are futures of one task (`tokio::join!`), and one stops another by a
//! `CancellationToken`: a stopped step drops its future, and its connection ends with it

use crate::base::BaseSync;
use crate::fetch::Local;
use crate::git::{Checkout, Snapshot, blocking};
use crate::host::{AgentInfo, Host, another_build_hint};
use crate::pack::{PackOf, plan_pack};
use crate::phases::{phase, phases_line};
use crate::record::{RunEnd, RunFiles, RunLock, RunRecord};
use crate::{Client, short};
use git_ops::host_client::{FARTHEST, Scan};
use host_link::heavy::script_in;
use host_link::proto::{PackTarget, Phase, Reply, Request, RunInfo, RunSpec, Stream, feature};
use host_link::refusal::{Code, Exit, Reached, Refusal};
use std::fmt::Write as _;
use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::{Instant, sleep, timeout};
use tokio_util::sync::CancellationToken;

/// The largest first-parent distance from HEAD of the first `Have` of a run. git walks 17 commits for it, and a host
/// that ran a recent commit of the checkout has one of them
const NEAR: usize = 16;

/// How many changed paths the refusal of local changes shows
const SHOWN_CHANGES: usize = 5;

/// How many times the cancel of a run whose pack did not reach the host tries
const ABANDON_TRIES: u32 = 3;

/// What a run does, from the side of the client
#[derive(Clone, Copy, Debug)]
pub struct RunRequest<'a> {
  /// A directory of the local checkout: its HEAD or its working tree runs, and the fetched files go below its root
  pub checkout: &'a Path,
  /// The main repository on the host, or `None` for the one of the host file
  pub repo: Option<&'a str>,
  /// The affinity of the lend: one per local checkout (`crate::affinity`). The host lends the slot this affinity used
  /// last first, and the base of the thin packs is the affinity's
  pub affinity: &'a str,
  /// The command and its arguments, run in the root of the slot
  pub command: &'a [String],
  /// Directories relative to the slot root, such as `out/bt-runs`, whose new entries come back
  pub fetch: &'a [String],
}

/// What a run adds to a `RunRequest`. The default is the committed HEAD of a checkout without local changes, the
/// niceness of the agent, the repository of the host file, no outputs and no changes
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[expect(
  clippy::struct_excessive_bools,
  reason = "each bool is one option of a run that no other option depends on, as the flags of the CLI are"
)]
pub struct RunOptions {
  /// Run the working tree: HEAD plus the tracked and untracked changes that git does not ignore, as one commit.
  /// Without it, a checkout with local changes is refused
  pub working_tree: bool,
  /// The niceness of the prepare steps and the command on the host, 0 to 19, or the agent's when that is higher
  pub nice: Option<u8>,
  /// Bring back the outputs that `[host]` of the commit names
  pub outputs: bool,
  /// Bring back the changes of the command and apply them to the working tree
  pub apply: bool,
  /// When `RunRequest::repo` names none: try the repository of the same name as the local one beside the repository
  /// of the host file first, such as `~/projects/branch-deck` beside `~/projects/idea-1`
  pub repo_by_name: bool,
  /// A heavy run whatever `heavy` of `[host]` says: it waits for the other heavy runs of the host
  pub heavy: bool,
  /// A measurement that needs a quiet host (`--exclusive`), heavy too: it waits for a quiet host without another heavy
  /// run, and while it runs the other runs of the host wait
  pub exclusive: bool,
  /// Wait for the end of the run at most this long from the start of the call, as `brd-host wait` does, and leave the
  /// run going past it (`--max`). The output goes to the log of the run, not to the output sink
  pub max: Option<Duration>,
}

/// The runs that an agent keeps, and the heavy turn of its host
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostRuns {
  pub runs: Vec<RunInfo>,
  /// The heavy turn of the host as `heavy.json` holds it (`lease_pool::host_turn::HeavyTurn`); `None` from an agent
  /// without the feature `heavy`
  pub heavy: Option<serde_json::Value>,
}

/// What the client knows of a run when it ends, for the report of the run (`crate::report::RunReport`)
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunFacts {
  /// The id of the run, once the client chose it for `Start`
  pub run: Option<String>,
  /// The shipped commit, once the snapshot read it
  pub commit: Option<String>,
  pub head: Option<String>,
  pub working_tree: bool,
  /// The repository of the run on the host, once the host answered `Have`
  pub repository: Option<String>,
  /// The steps of the run in their order: `pack` and `upload` of the client, the steps of the host, `fetch`, and
  /// `total`, the whole time of the call
  pub phases: Vec<Phase>,
  /// The files that came back, relative to the checkout root with `/` separators
  pub fetched: Vec<String>,
  /// The count of the files that the apply changed; `None` without `RunOptions::apply`
  pub applied_files: Option<u64>,
}

/// The end of a run whose command exited
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunOutcome {
  /// The exit code of the command. A command that a signal ended answers 128 plus the signal number
  pub code: i32,
  /// The bytes of the stdout of the command; empty with `Client::with_output`
  pub stdout: Vec<u8>,
  pub facts: RunFacts,
}

/// A run that refused
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunFailure {
  pub refusal: Refusal,
  /// The exit of `brd-host`: the code of the refusal at the point the run reached
  pub exit: Exit,
  /// The exit code of the command, when it ran and its end came, as for a failed fetch or apply
  pub command_exit: Option<i32>,
  pub facts: RunFacts,
}

impl RunFailure {
  fn at(refusal: Refusal, reached: Reached, facts: &RunFacts) -> Self {
    Self {
      exit: refusal.code.exit(reached),
      refusal,
      command_exit: None,
      facts: facts.clone(),
    }
  }
}

/// The end of a followed run
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Followed {
  pub code: i32,
  /// The bytes of the stdout of the command; empty with `Client::with_output`
  pub stdout: Vec<u8>,
  /// The steps of the run on the host, from `Reply::Phases`. Empty when the agent sent none
  pub phases: Vec<Phase>,
}

/// Why one connection of `Follow` ended without the end of the run
enum Broken {
  /// The link failed: connect again
  Link(Refusal),
  /// The agent refused, or the run failed: stop
  Refused(Refusal),
}

/// How a followed run ended
pub(crate) enum Ended {
  /// The command exited
  Exit(Followed),
  /// The agent ended the run with a refusal of its own, such as `lease_failed`, after the steps of the host
  Refused(Refusal, Vec<Phase>),
  /// The client did not see the end: the link is lost, or a second Ctrl+C stopped the follow
  Unseen(Refusal),
}

/// What the watch of a run and the follow share
#[derive(Default)]
pub(crate) struct Watch {
  /// The follow ended
  finished: CancellationToken,
  /// A second Ctrl+C came, or the pack of the run failed: the follow stops
  abandoned: CancellationToken,
}

impl Watch {
  pub(crate) fn new() -> Self {
    Self::default()
  }

  /// Stops the follow at once: its connection ends with its future, and no new one comes
  fn stop_follow(&self) {
    self.abandoned.cancel();
  }
}

/// How a followed run ended, after its fetch
struct Finished {
  followed: Followed,
  fetched: Result<Fetched, Refusal>,
  /// The step `fetch`, when the run names fetch directories, outputs or changes
  fetch: Option<Phase>,
  /// The lock of the end of the run, held up to its end file; `None` when no process of the checkout holds it
  lock: Option<RunLock>,
  /// The host forgot the run
  forgotten: bool,
  /// The end that a `brd-host wait` of this checkout finished first: the fetch, the apply and the forget are done
  done: Option<RunEnd>,
}

/// What the commit and the `Have` of a run found
struct Found {
  snapshot: Snapshot,
  repo: String,
  present: Vec<String>,
}

impl Client<'_> {
  /// Runs a command of the checkout on the host of `alias` with `options`
  #[expect(clippy::result_large_err, reason = "a run ends once, and its failure carries the facts of its report")]
  pub async fn run(&self, alias: &str, request: &RunRequest<'_>, options: RunOptions) -> Result<RunOutcome, RunFailure> {
    let started = Instant::now();
    let mut facts = RunFacts {
      working_tree: options.working_tree,
      ..RunFacts::default()
    };
    if request.command.is_empty() {
      return Err(RunFailure::at(Refusal::new(Code::Usage, "a run needs a command"), Reached::Nothing, &facts));
    }
    let before = |refusal: Refusal, facts: &RunFacts| RunFailure::at(refusal, Reached::Nothing, facts);
    let mut host = self.host(alias).map_err(|refusal| before(refusal, &facts))?;
    let checkout = Checkout::open(&self.git, request.checkout).map_err(|refusal| before(refusal, &facts))?;
    let run = run_id();
    let recorded = self.config.base(alias, request.affinity);
    let Found { snapshot, repo, present } = self
      .find(&mut host, &checkout, request, options, recorded.as_deref())
      .await
      .map_err(|refusal| before(refusal, &facts))?;
    facts.commit = Some(snapshot.commit.clone());
    facts.head = Some(snapshot.head.clone());
    facts.repository = Some(repo.clone());
    let commit = snapshot.commit.clone();
    let files = RunFiles::new(&checkout.runs_dir()).map_err(|refusal| before(refusal, &facts))?;
    let config = checkout.host_config(&commit).map_err(|refusal| before(refusal, &facts))?;
    let heavy = if options.exclusive {
      Some("--exclusive".to_owned())
    } else if options.heavy {
      Some("--heavy".to_owned())
    } else {
      // a shell may run a heavy command in its script, or in a script file of the working tree that the run ships
      config.heavy_reason(request.command, &|path| script_in(checkout.root(), path))
    };
    let needs = config.needs_features();
    let outputs = if options.outputs { config.outputs } else { Vec::new() };
    let record = RunRecord {
      commit: commit.clone(),
      fetch: request.fetch.to_vec(),
      outputs,
      changes: options.apply,
    };
    let plan = if present.contains(&commit) {
      None
    } else {
      let history = if present.is_empty() { checkout.history(&snapshot.head) } else { None };
      Some(plan_pack(alias, &present, history).map_err(|refusal| before(refusal, &facts))?)
    };
    self.refuse_interrupt().map_err(|refusal| before(refusal, &facts))?;
    let spec = RunSpec {
      run: run.clone(),
      repo: repo.clone(),
      affinity: request.affinity.to_owned(),
      commit: commit.clone(),
      command: request.command.to_vec(),
      fetch: request.fetch.to_vec(),
      nice: options.nice,
      outputs: options.outputs,
      changes: options.apply,
      heavy: heavy.is_some(),
      exclusive: options.exclusive,
    };
    files.write(&run, &record).map_err(|refusal| before(refusal, &facts))?;
    facts.run = Some(run.clone());
    // A start whose reply did not come is cancelled, and the checkout of the run waits for a pack that never comes
    self.start(&mut host, spec, &needs).await.map_err(|refusal| before(refusal, &facts))?;
    (self.say)(&run_line(&run, alias, &snapshot, &repo, &host.file().repo));
    if let Some(max) = options.max {
      (self.say)(&format!(
        "brd-host: this call waits for the end of the run {run} up to {} and then leaves it going; its output goes to {}",
        crate::phases::duration_text(u64::try_from(max.as_millis()).unwrap_or(u64::MAX)),
        files.log(&run).display()
      ));
    }
    if options.exclusive {
      (self.say)(&format!(
        "brd-host: an exclusive run by --exclusive: on {alias} it waits for a quiet host without another heavy run, and while it runs no heavy run is admitted and each new run that is not heavy waits for its end"
      ));
    } else if let Some(why) = &heavy {
      (self.say)(&format!(
        "brd-host: a heavy run by {why}: on {alias} it waits until the cores that its command used before are free beside the other heavy runs, or the whole host without a measure"
      ));
    }

    let sync = BaseSync {
      checkout: &checkout,
      repo: &repo,
      affinity: request.affinity,
      commit: &commit,
      recorded: recorded.as_deref(),
      snapshot: &snapshot,
    };
    let pack = PackOf {
      checkout: &checkout,
      repo: &repo,
      commit: &commit,
      target: PackTarget::Run(run.clone()),
    };
    let watch = Watch::new();
    let local = Local {
      root: checkout.root(),
      git_common_dir: checkout.git_common_dir(),
    };
    let (run, record, files) = (&run, &record, &files);
    let (mut followed, mut synced) = (host.clone(), host.clone());
    // the end of the step of the pack, the end of the base sync, and the stop of the base sync
    let (packed, sync_ended, stop_sync) = (CancellationToken::new(), CancellationToken::new(), CancellationToken::new());
    let pack_failed = AtomicBool::new(false);
    let sending = async {
      let _sync_ended = sync_ended.clone().drop_guard();
      let packing = packed.clone().drop_guard();
      if let Some(plan) = &plan {
        match self.send_pack(&mut host, &pack, plan, self.say).await {
          Ok(steps) => {
            facts.phases.push(phase("pack", steps.build));
            facts.phases.push(phase("upload", steps.upload));
          }
          // The watch of the follow sent the cancel of the press, and the output follows up to its end
          Err(_) if self.interrupted() => {}
          Err(refusal) => {
            let refusal = self.abandon(&mut host, run, &format!("the pack did not reach {alias}"), &refusal).await;
            pack_failed.store(true, Ordering::Release);
            watch.stop_follow();
            return Err(refusal);
          }
        }
      }
      drop(packing);
      tokio::select! {
        () = self.sync_base(&mut synced, &sync) => {}
        () = stop_sync.cancelled() => {}
      }
      Ok(())
    };
    let ending = async {
      let finished = match options.max {
        None => self.follow_and_fetch(&mut followed, run, local, record, files, &watch).await,
        // the wait starts after the pack, as `brd-host wait` of the run would
        Some(max) => {
          packed.cancelled().await;
          if pack_failed.load(Ordering::Acquire) {
            return Err((Refusal::new(Code::Link, "the pack did not reach the host"), Reached::Nothing, Vec::new()));
          }
          self.wait_bounded(&mut followed, run, max, started, &checkout, files, record).await
        }
      };
      // a call that leaves the run going leaves the sync too
      let leaves = matches!(&finished, Err((refusal, ..)) if refusal.code == Code::StillRunning);
      if leaves {
        stop_sync.cancel();
      } else {
        if !sync_ended.is_cancelled() && !pack_failed.load(Ordering::Acquire) {
          (self.say)(&format!("> {alias}: the sync of the base of the next packs goes on; Ctrl+C stops it"));
        }
        tokio::select! {
          () = sync_ended.cancelled() => {}
          () = self.pressed() => stop_sync.cancel(),
        }
      }
      finished
    };
    // the steps of a run hold many futures, so they live on the heap rather than in the future of `run`
    let (sent, finished) = Box::pin(async { tokio::join!(sending, ending) }).await;
    // The checkout waits for a pack that failed, so the command never started
    let ended = sent.map_err(|refusal| (refusal, Reached::Nothing, Vec::new())).and(finished);
    let finished = match ended {
      Ok(finished) => finished,
      Err((refusal, reached, steps)) => {
        // A run that the agent refused has no outcome, so the client says its steps here
        if !steps.is_empty() {
          facts.phases.extend(steps);
          (self.say)(&phases_line(&facts.phases));
        }
        return Err(RunFailure::at(refusal, reached, &facts));
      }
    };
    let code = finished.followed.code;
    if let Some(done) = finished.done {
      // a wait of this checkout finished the run while this process followed it, or this process waited as a wait does
      facts.phases.extend(done.phases.iter().cloned());
      facts.fetched = done.fetched;
      facts.applied_files = done.applied_files;
      facts.phases.push(phase("total", self.took(started)));
      let reached = reached_of(&done.phases);
      return match done.refusal {
        Some(refusal) => Err(RunFailure {
          command_exit: done.command_exit,
          exit: refusal.code.exit(reached),
          refusal,
          facts,
        }),
        None => Ok(RunOutcome {
          code,
          stdout: finished.followed.stdout,
          facts,
        }),
      };
    }
    let lock = finished.lock;
    let forgotten = finished.forgotten;
    facts.phases.extend(finished.followed.phases.iter().cloned());
    facts.phases.extend(finished.fetch);
    // The end file says what came, so a later `wait` answers from it and never applies the changes again
    let write_end = |facts: &RunFacts, refusal: Option<&Refusal>| {
      if lock.is_some() {
        let end = RunEnd {
          exit: refusal.map_or_else(|| crate::report::command_exit_code(code), |refusal| refusal.code.exit(Reached::CommandStarted).code()),
          command_exit: Some(code),
          refusal: refusal.cloned(),
          phases: facts.phases.clone(),
          fetched: facts.fetched.clone(),
          applied_files: facts.applied_files,
          forgotten,
        };
        if let Err(refusal) = files.write_end(run, &end) {
          (self.say)(&format!("brd-host: {refusal}"));
        }
      }
    };
    let after_command = |refusal: Refusal, facts: &RunFacts| RunFailure {
      command_exit: Some(code),
      ..RunFailure::at(refusal, Reached::CommandStarted, facts)
    };
    let fetched = match finished.fetched {
      Ok(fetched) => fetched,
      Err(refusal) if options.apply => {
        let refusal = Refusal::new(
          Code::ApplyFailed,
          format!(
            "the command of the run {run} exited with {code}, and its changes did not come ({}): `brd-host fetch {alias} {run}` copies them into the git directory of this checkout",
            refusal.message
          ),
        );
        return Err(after_command(refusal, &facts));
      }
      Err(refusal) => return Err(after_command(refusal, &facts)),
    };
    facts.fetched.clone_from(&fetched.files);
    if options.apply {
      match self.apply_changes(&checkout, run, &fetched, code) {
        Ok(files) => facts.applied_files = Some(files),
        Err(refusal) => {
          write_end(&facts, Some(&refusal));
          return Err(after_command(refusal, &facts));
        }
      }
    }
    write_end(&facts, None);
    drop(lock);
    facts.phases.push(phase("total", self.took(started)));
    Ok(RunOutcome {
      code,
      stdout: finished.followed.stdout,
      facts,
    })
  }

  /// The snapshot of the run, the repository on the host, and the candidates that it has. Without the working tree, a
  /// checkout with local changes refuses at once: the status runs on the blocking pool beside the `Have`, and its
  /// refusal stops the `Have` and its connection
  async fn find(&self, host: &mut Host, checkout: &Checkout, request: &RunRequest<'_>, options: RunOptions, recorded: Option<&str>) -> Result<Found, Refusal> {
    if options.working_tree {
      let scanned = checkout.clone();
      let snapshot = blocking(move || scanned.snapshot(Scan::WorkingTree, NEAR)).await?;
      self.refuse_interrupt()?;
      let (repo, present) = self.present_in_repo(host, checkout, request, options, &snapshot, recorded).await?;
      return Ok(Found { snapshot, repo, present });
    }
    let snapshot = checkout.snapshot(Scan::Head, NEAR)?;
    self.refuse_interrupt()?;
    let dirty = CancellationToken::new();
    let clean = async {
      let scanned = checkout.clone();
      let clean = blocking(move || check_clean(&scanned)).await;
      if clean.is_err() {
        dirty.cancel();
      }
      clean
    };
    let found = async {
      tokio::select! {
        found = self.present_in_repo(host, checkout, request, options, &snapshot, recorded) => found,
        () = dirty.cancelled() => Err(Refusal::new(Code::LocalChanges, "the checkout has local changes")),
      }
    };
    let (clean, found) = tokio::join!(clean, found);
    clean?;
    let (repo, present) = found?;
    Ok(Found { snapshot, repo, present })
  }

  /// The repository of the run on the host, and the candidates of the commit that it has. With
  /// `RunOptions::repo_by_name`, a host that lacks the repository of the local name gets the repository of the host
  /// file instead
  async fn present_in_repo(
    &self,
    host: &mut Host,
    checkout: &Checkout,
    request: &RunRequest<'_>,
    options: RunOptions,
    snapshot: &Snapshot,
    recorded: Option<&str>,
  ) -> Result<(String, Vec<String>), Refusal> {
    let default = host.file().repo.clone();
    let named = match request.repo {
      Some(repo) => Some(repo.to_owned()),
      None if options.repo_by_name => checkout
        .repository_name()
        .as_deref()
        .and_then(|name| sibling(&default, name))
        .filter(|repo| *repo != default),
      None => None,
    };
    // The commit of the working tree is a candidate too: a host that ran the same tree on the same HEAD has it
    let extra = [Some(snapshot.commit.as_str()), recorded];
    let asked = match &named {
      Some(named) => match self.present(host, named, checkout, &snapshot.candidates, &extra).await {
        Err(refusal) if refusal.code == Code::RepoMissing && request.repo.is_none() => {
          (self.say)(&format!(
            "brd-host: {} has no {named}; the run goes to {default}, the repository of its host file. --repo names another one",
            host.alias()
          ));
          None
        }
        found => Some(found.map(|present| (named.clone(), present))),
      },
      None => None,
    };
    match asked {
      Some(found) => found,
      None => Ok((default.clone(), self.present(host, &default, checkout, &snapshot.candidates, &extra).await?)),
    }
  }

  /// Applies the patch of the changes of the command to the working tree. Answers the count of its files. A patch that
  /// does not apply stays, and the refusal names it
  pub(crate) fn apply_changes(&self, checkout: &Checkout, run: &str, fetched: &Fetched, code_of_command: i32) -> Result<u64, Refusal> {
    let Some(patch) = &fetched.changes else {
      (self.say)("brd-host: the command changed no file");
      return Ok(0);
    };
    match checkout.apply(patch) {
      Ok(files) => {
        (self.say)(&format!(
          "brd-host: applied the changes of the command to {files} files of this working tree; `git apply -R {}` undoes them",
          patch.display()
        ));
        Ok(files as u64)
      }
      Err(refusal) => Err(Refusal::new(
        Code::ApplyFailed,
        format!(
          "the command of the run {run} exited with {code_of_command}, and its changes do not apply to this working tree: {}. The patch is {}",
          refusal.message,
          patch.display()
        ),
      )),
    }
  }

  /// Sends `Start`, unless the agent lacks a feature of `needs`, which `[host]` of the commit needs: such an agent refuses
  /// every run of the commit, so the start refuses with `bad_config` (exit 75) and names the upgrade. A reply that does
  /// not come can still leave the run on the agent, so the client cancels it then
  async fn start(&self, host: &mut Host, spec: RunSpec, needs: &[&str]) -> Result<(), Refusal> {
    let run = spec.run.clone();
    let (mut channel, agent) = host.connect().await?;
    if let Some(hint) = another_build_hint(host.alias(), &agent, self.build.sha256.as_deref()) {
      (self.say)(&hint);
    }
    if let Some(lacking) = needs.iter().find(|needed| !agent.features.iter().any(|known| known == *needed)) {
      return Err(Refusal::new(
        Code::BadConfig,
        format!(
          "the agent on {alias} predates `{lacking}` of [host] of .branchdeck.toml, so it refuses every run of this commit: `brd-host upgrade {alias}` sends this build",
          alias = host.alias()
        ),
      ));
    }
    if spec.exclusive && !agent.features.iter().any(|known| known == feature::EXCLUSIVE) {
      return Err(Refusal::new(
        Code::BadConfig,
        format!(
          "the agent on {alias} predates exclusive runs, so it would run this measurement beside the others: `brd-host upgrade {alias}` sends this build",
          alias = host.alias()
        ),
      ));
    }
    if spec.heavy && !agent.features.iter().any(|known| known == feature::HEAVY) {
      (self.say)(&format!(
        "brd-host: the agent on {alias} predates heavy runs, so this one runs beside the others: `brd-host upgrade {alias}` sends this build",
        alias = host.alias()
      ));
    }
    channel.send(&Request::Start(spec)).await.map_err(|error| host.link_refusal(&error))?;
    match host.recv(&mut channel, self.timing.read).await {
      Ok(Reply::Started { .. }) => Ok(()),
      Ok(Reply::Refused { code, message }) => Err(host.agent_refusal(&code, &message)),
      Ok(reply) => Err(host.unexpected(&reply)),
      Err(refusal) if is_link(&refusal) => {
        let what = format!("the start of the run {run} got no reply from {}", host.alias());
        Err(self.abandon(host, &run, &what, &refusal).await)
      }
      Err(refusal) => Err(refusal),
    }
  }

  /// Follows a started run to its end, takes the lock of its end, then copies the files that `record` asks for, with the
  /// changes to the patch file of the run, and forgets the run on the host. An end that a wait finished first is the
  /// answer instead. A run that the agent refused is forgotten too, and its failure carries how far it came
  /// and the steps of the host. A run that vanished on the host after it started (`unknown_run`) may have run its
  /// command, so its exit is 255, and a caller never runs the command a second time
  async fn follow_and_fetch(
    &self,
    host: &mut Host,
    run: &str,
    local: Local<'_>,
    record: &RunRecord,
    files: &RunFiles,
    watch: &Watch,
  ) -> Result<Finished, (Refusal, Reached, Vec<Phase>)> {
    let alias = host.alias().to_owned();
    let followed = match self.follow(host, run, true, watch, None).await {
      Ended::Exit(followed) => followed,
      Ended::Refused(refusal, steps) if refusal.code == Code::UnknownRun => return Err((vanished(&refusal, run, &alias), Reached::Started, steps)),
      Ended::Refused(refusal, steps) => {
        self.forget_or_say(host, run).await;
        let reached = if steps.iter().any(|step| step.name == "command") {
          Reached::CommandStarted
        } else {
          Reached::Started
        };
        return Err((refusal, reached, steps));
      }
      Ended::Unseen(refusal) => return Err((unseen_after_start(refusal), Reached::Started, Vec::new())),
    };
    // A `brd-host wait` of this checkout may finish the run at the same time: one process fetches, applies and forgets
    let lock = match files.lock(run, None, self.timing.interrupt_poll, &|| watch.abandoned.is_cancelled()).await {
      Ok(lock) => lock,
      Err(refusal) => {
        (self.say)(&format!("brd-host: {refusal}"));
        None
      }
    };
    match files.read_end(run) {
      Ok(Some(done)) => {
        return Ok(Finished {
          followed,
          fetched: Ok(Fetched::default()),
          fetch: None,
          lock,
          forgotten: done.forgotten,
          done: Some(done),
        });
      }
      Ok(None) => {}
      Err(refusal) => (self.say)(&format!("brd-host: {refusal}")),
    }
    if !record.asks_for_files() {
      let forgotten = self.forget_or_say(host, run).await;
      return Ok(Finished {
        followed,
        fetched: Ok(Fetched::default()),
        fetch: None,
        lock,
        forgotten,
        done: None,
      });
    }
    let fetching = Instant::now();
    let fetched = self.fetch_run(host, run, local, record, &files.patch(run)).await;
    let fetch = Some(phase("fetch", self.took(fetching)));
    let mut forgotten = false;
    let fetched = match fetched {
      Ok(fetched) => {
        forgotten = self.forget_or_say(host, run).await;
        Ok(fetched)
      }
      Err(refusal) => {
        let message = format!(
          "cannot copy the files of the run {run} from {alias}: {}. The run keeps its files on {alias}: `brd-host fetch {alias} {run}` in this checkout copies them again, and `brd-host forget {alias} {run}` removes them",
          refusal.message
        );
        let code = if refusal.code == Code::FetchFailed || is_link(&refusal) {
          Code::FetchFailed
        } else {
          refusal.code
        };
        (self.say)(&format!("brd-host: {message}"));
        Err(Refusal::new(code, message))
      }
    };
    Ok(Finished {
      followed,
      fetched,
      fetch,
      lock,
      forgotten,
      done: None,
    })
  }

  /// Waits for the end of a started run as `brd-host wait` does, up to `max` from `started`, then finishes it by `record`
  /// and says its summary. The first Ctrl+C cancels the run and the wait goes on; the second one leaves. The end is the
  /// `done` of the answer
  #[expect(clippy::too_many_arguments, reason = "the run, its bound and its checkout")]
  async fn wait_bounded(
    &self,
    host: &mut Host,
    run: &str,
    max: Duration,
    started: Instant,
    checkout: &Checkout,
    files: &RunFiles,
    record: &RunRecord,
  ) -> Result<Finished, (Refusal, Reached, Vec<Phase>)> {
    let alias = host.alias().to_owned();
    let past_max = || self.past_max.map_or_else(|| started.elapsed() >= max, |past| past());
    let ended = CancellationToken::new();
    let canceller = host.clone();
    // the first press cancels the run beside the wait; the cancel gets a moment after the end of the wait, not more
    let cancelling = async {
      tokio::select! {
        () = self.pressed() => {}
        () = ended.cancelled() => return,
      }
      (self.say)(&format!(
        "brd-host: Ctrl+C, SIGTERM or SIGHUP: cancelling the run {run} on {alias}; this call waits for its end. A second one leaves"
      ));
      let graced = async {
        ended.cancelled().await;
        sleep(self.timing.cancel_grace).await;
      };
      tokio::select! {
        () = self.send_cancel(canceller, run, &ended) => {}
        () = graced => {}
      }
    };
    let left = || {
      Refusal::new(
        Code::Interrupted,
        format!("a second Ctrl+C left the wait of the run {run}; {alias} has its cancel: `brd-host wait {alias} {run}` shows its end"),
      )
    };
    let waiting = async {
      let info = self.wait_for_end(host, run, max, started, &past_max, &|| (self.presses)() >= 2, &left).await;
      ended.cancel();
      info
    };
    let unseen = |refusal: Refusal| (refusal, Reached::Started, Vec::new());
    let info = tokio::join!(cancelling, waiting).1.map_err(unseen)?;
    // a `brd-host wait` of this checkout can finish the run at the same time
    let _lock = files.lock(run, None, self.timing.interrupt_poll, &|| false).await.map_err(unseen)?;
    let (end, again) = match files.read_end(run).map_err(unseen)? {
      Some(end) => (end, true),
      None => (self.finish(host, run, checkout, files, Some(record), &info, &left).await.map_err(unseen)?, false),
    };
    self.say_summary(&alias, run, &end, files, checkout.root(), again);
    Ok(Finished {
      followed: Followed {
        code: end.command_exit.unwrap_or_default(),
        stdout: Vec::new(),
        phases: Vec::new(),
      },
      fetched: Ok(Fetched::default()),
      fetch: None,
      lock: None,
      forgotten: end.forgotten,
      done: Some(end),
    })
  }

  /// Cancels a started run whose start or pack failed, so its supervisor does not wait for the pack. Answers the
  /// refusal with its code, the failed step `what`, and what became of the run
  async fn abandon(&self, host: &mut Host, run: &str, what: &str, refusal: &Refusal) -> Refusal {
    let alias = host.alias().to_owned();
    let mut backoff = self.timing.first_backoff;
    let mut tries = 1;
    let cancelled = loop {
      match cancel_on(host, run, self.timing.read).await {
        Err(failed) if is_link(&failed) && tries < ABANDON_TRIES => {
          tries += 1;
          sleep(backoff).await;
          backoff = (backoff * 2).min(self.timing.max_backoff);
        }
        cancelled => break cancelled,
      }
    };
    let fate = match cancelled {
      Ok(_) => format!("{alias} has the cancel of the run {run}"),
      Err(failed) => format!("cannot cancel the run {run} on {alias}: {}. `brd-host cancel {alias} {run}` cancels it", failed.message),
    };
    Refusal::new(refusal.code, format!("{what}: {}; {fate}", refusal.message))
  }

  /// Sends `Cancel`, and says its answer. It connects again with the backoff of `Follow` until the agent has the cancel,
  /// `finished` says that the run has ended, or `Timing::cancel_for` has passed
  async fn send_cancel(&self, mut host: Host, run: &str, finished: &CancellationToken) {
    let started = Instant::now();
    let mut backoff = self.timing.first_backoff;
    loop {
      match cancel_on(&mut host, run, self.timing.read).await {
        Ok(message) => {
          (self.say)(&format!("brd-host: {message}"));
          return;
        }
        Err(refusal) if is_link(&refusal) && started.elapsed() < self.timing.cancel_for && !finished.is_cancelled() => {
          (self.say)(&format!(
            "brd-host: cannot send the cancel of the run {run}: {}; again in {} s",
            refusal.message,
            backoff.as_secs()
          ));
          sleep(backoff).await;
          backoff = (backoff * 2).min(self.timing.max_backoff);
        }
        Err(refusal) => {
          (self.say)(&format!("brd-host: cannot cancel the run {run}: {}", refusal.message));
          return;
        }
      }
    }
  }

  /// Answers at the first Ctrl+C, which it looks for every `Timing::interrupt_poll`
  pub(crate) async fn pressed(&self) {
    while !self.interrupted() {
      sleep(self.timing.interrupt_poll).await;
    }
  }

  /// Follows a run from its first line, as `run` does. Ctrl+C here cancels nothing: the caller leaves, and the run goes
  /// on
  pub async fn follow_run(&self, alias: &str, run: &str) -> Result<Followed, Refusal> {
    let mut host = self.host(alias)?;
    match self.follow(&mut host, run, false, &Watch::new(), None).await {
      Ended::Exit(followed) => Ok(followed),
      Ended::Refused(refusal, _) | Ended::Unseen(refusal) => Err(refusal),
    }
  }

  /// Asks the agent to interrupt a run. The answer comes as soon as the host has the request; the run ends later, and
  /// `follow_run` shows its end
  pub async fn cancel(&self, alias: &str, run: &str) -> Result<String, Refusal> {
    let mut host = self.host(alias)?;
    cancel_on(&mut host, run, self.timing.read).await
  }

  /// The runs that the agent keeps, and the heavy turn of its host
  pub async fn runs(&self, alias: &str) -> Result<HostRuns, Refusal> {
    self
      .ask(&mut self.host(alias)?, &Request::Runs, |reply| match reply {
        Reply::Runs { runs, heavy } => Taken::Answer(HostRuns { runs, heavy }),
        reply => Taken::Other(reply),
      })
      .await
  }

  /// The lend table of each repository that the agent served, as `host-slots.json` holds it
  pub async fn slots(&self, alias: &str) -> Result<Vec<host_link::proto::RepositorySlots>, Refusal> {
    self
      .ask(&mut self.host(alias)?, &Request::Slots, |reply| match reply {
        Reply::Slots { repositories } => Taken::Answer(repositories),
        reply => Taken::Other(reply),
      })
      .await
  }

  /// What the pools of the host hold on disk, as the JSON of `pool_disk::report::DiskReport` (`Request::Disk`); `fresh`
  /// walks every slot again. An agent without the feature `disk` refuses with `agent_failed` and the upgrade to run
  /// (`featured_host`)
  pub async fn disk(&self, alias: &str, fresh: bool) -> Result<serde_json::Value, Refusal> {
    self
      .ask(&mut self.featured_host(alias, feature::DISK).await?, &Request::Disk { fresh }, |reply| match reply {
        Reply::Disk { report } => Taken::Answer(report),
        reply => Taken::Other(reply),
      })
      .await
  }

  /// The trim of every pool of the host, as the JSON of `pool_disk::trim_report::TrimReport` (`Request::Trim`): the free
  /// slots past `max`, else `branchdeck.leaseMaxSlots` there; `plan` only says what would go. An agent without the
  /// feature `trim` refuses with `agent_failed` and the upgrade to run (`featured_host`)
  pub async fn trim(&self, alias: &str, max: Option<usize>, plan: bool) -> Result<serde_json::Value, Refusal> {
    self
      .ask(&mut self.featured_host(alias, feature::TRIM).await?, &Request::Trim { max, plan }, |reply| match reply {
        Reply::Trimmed { report } => Taken::Answer(report),
        reply => Taken::Other(reply),
      })
      .await
  }

  /// The host of `alias` once its hello names `feature`. An agent without it refuses with `agent_failed` and the upgrade
  /// to run, before the request: it would close the connection on a request it does not know
  async fn featured_host(&self, alias: &str, wanted: &str) -> Result<Host, Refusal> {
    let mut host = self.host(alias)?;
    let (channel, info) = host.connect().await?;
    channel.close().await;
    if !info.features.iter().any(|known| known == wanted) {
      return Err(Refusal::new(
        Code::AgentFailed,
        format!("the agent on {alias} has no feature {wanted}: `brd-host upgrade {alias}` sends it this build"),
      ));
    }
    Ok(host)
  }

  /// One request on a new connection, and its reply that `take` reads. A refusal of the agent keeps its code, and a
  /// reply that `take` gives back is out of turn
  async fn ask<T>(&self, host: &mut Host, request: &Request, take: impl FnOnce(Reply) -> Taken<T> + Send) -> Result<T, Refusal> {
    ask_within(host, request, self.timing.read, take).await
  }

  /// The hello of the agent: its version, its build and its platform, and the address that answered
  pub async fn hello(&self, alias: &str) -> Result<AgentInfo, Refusal> {
    let mut host = self.host(alias)?;
    let (channel, info) = host.connect().await?;
    channel.close().await;
    Ok(info)
  }

  /// The commits of `near` and `extra` that the host has. `near` are the candidates of the snapshot, which git finds at
  /// once. Only a host that has none of them gets a second `Have` with the first-parent walk up to `FARTHEST`, which
  /// reads up to 65537 commits
  pub(crate) async fn present(&self, host: &mut Host, repo: &str, checkout: &Checkout, near: &[String], extra: &[Option<&str>]) -> Result<Vec<String>, Refusal> {
    let candidates = |of: &[String]| {
      let mut found = Vec::new();
      for candidate in of.iter().map(String::as_str).map(Some).chain(extra.iter().copied()) {
        add_candidate(&mut found, candidate);
      }
      found
    };
    let present = self.have(host, repo, candidates(near)).await?;
    if !present.is_empty() {
      return Ok(present);
    }
    let scanned = checkout.clone();
    let far = blocking(move || scanned.snapshot(Scan::Head, FARTHEST)).await?;
    self.have(host, repo, candidates(&far.candidates)).await
  }

  /// The commits of `candidates` that the host has
  pub(crate) async fn have(&self, host: &mut Host, repo: &str, candidates: Vec<String>) -> Result<Vec<String>, Refusal> {
    let request = Request::Have {
      repo: repo.to_owned(),
      commits: candidates,
    };
    self
      .ask(host, &request, |reply| match reply {
        Reply::Have { commits } => Taken::Answer(commits),
        reply => Taken::Other(reply),
      })
      .await
  }

  fn refuse_interrupt(&self) -> Result<(), Refusal> {
    if self.interrupted() {
      Err(Refusal::new(Code::Interrupted, "Ctrl+C came before the run started; nothing runs on the host"))
    } else {
      Ok(())
    }
  }

  /// Forgets the run on the host, or says why it cannot. Answers whether the host forgot it
  pub(crate) async fn forget_or_say(&self, host: &mut Host, run: &str) -> bool {
    match self.forget_on(host, run).await {
      Ok(_) => true,
      Err(refusal) => {
        (self.say)(&format!(
          "brd-host: cannot forget the run {run} on {}: {}. `brd-host forget {} {run}` removes its files",
          host.alias(),
          refusal.message,
          host.alias()
        ));
        false
      }
    }
  }

  /// Follows the output of a run up to its end. With `cancel_on_interrupt`, the first Ctrl+C sends `Cancel`, and the
  /// second one stops the follow; without it, the first Ctrl+C leaves the follow, and the run goes on.
  /// `Watch::stop_follow` stops it from another step. With `log`, the bytes of both streams go to that file instead
  pub(crate) async fn follow(&self, host: &mut Host, run: &str, cancel_on_interrupt: bool, watch: &Watch, log: Option<&File>) -> Ended {
    let watched = host.clone();
    let watching = async {
      if cancel_on_interrupt {
        self.watch_interrupts(&watched, run, watch).await;
      } else {
        self.leave_on_interrupt(watch).await;
      }
    };
    let following = async {
      let ended = self.follow_loop(host, run, cancel_on_interrupt, watch, log).await;
      watch.finished.cancel();
      ended
    };
    tokio::join!(watching, following).1
  }

  /// Reads the Ctrl+C count while the run goes on. The cancel goes beside the follow, and the end of the run waits for it
  /// only `Timing::cancel_grace`, so a cancel that hangs never holds the end of the run
  async fn watch_interrupts(&self, host: &Host, run: &str, watch: &Watch) {
    let mut cancel = None;
    let mut cancelled = false;
    loop {
      let presses = (self.presses)();
      if presses >= 1 && cancel.is_none() && !cancelled {
        (self.say)(&format!(
          "brd-host: Ctrl+C, SIGTERM or SIGHUP: cancelling the run {run} on {}; its output follows up to its end. A second one stops the follow",
          host.alias()
        ));
        cancel = Some(Box::pin(self.send_cancel(host.clone(), run, &watch.finished)));
      }
      if presses >= 2 {
        watch.stop_follow();
      }
      let sending = async {
        match cancel.as_mut() {
          Some(sending) => sending.await,
          None => std::future::pending().await,
        }
      };
      tokio::select! {
        () = watch.finished.cancelled() => break,
        () = sleep(self.timing.interrupt_poll) => {}
        () = sending => {
          cancel = None;
          cancelled = true;
        }
      }
    }
    // The answer of the cancel can come just after the end of the run; it gets a moment, not more
    if let Some(sending) = cancel {
      let _ = timeout(self.timing.cancel_grace, sending).await;
    }
  }

  /// Reads the Ctrl+C count while a follow without a cancel goes on: the first press stops the follow at once
  async fn leave_on_interrupt(&self, watch: &Watch) {
    tokio::select! {
      () = self.pressed() => watch.stop_follow(),
      () = watch.finished.cancelled() => {}
    }
  }

  async fn follow_loop(&self, host: &mut Host, run: &str, cancel_on_interrupt: bool, watch: &Watch, log: Option<&File>) -> Ended {
    let mut state = FollowState {
      offset: 0,
      stdout: Vec::new(),
      stderr: Vec::new(),
      log,
      last_reply: Instant::now(),
      backoff: self.timing.first_backoff,
      phases: Vec::new(),
    };
    loop {
      let once = tokio::select! {
        once = self.follow_once(host, run, &mut state) => once,
        () = watch.abandoned.cancelled() => Err(Broken::Link(Refusal::new(Code::Interrupted, "the follow stopped"))),
      };
      self.say_open_stderr(&mut state);
      if watch.abandoned.is_cancelled() {
        let alias = host.alias();
        return Ended::Unseen(Refusal::new(
          Code::Interrupted,
          if cancel_on_interrupt {
            format!("a second Ctrl+C stopped the follow of the run {run}; {alias} has its cancel. `brd-host follow {alias} {run}` shows its end")
          } else {
            format!("Ctrl+C, SIGTERM or SIGHUP left the follow of the run {run}, which goes on on {alias}: `brd-host follow {alias} {run}` follows it again")
          },
        ));
      }
      match once {
        Ok(code) => {
          return Ended::Exit(Followed {
            code,
            stdout: state.stdout,
            phases: state.phases,
          });
        }
        Err(Broken::Refused(refusal)) if is_local(&refusal) => return Ended::Unseen(refusal),
        Err(Broken::Refused(refusal)) => return Ended::Refused(refusal, state.phases),
        Err(Broken::Link(refusal)) => {
          if state.last_reply.elapsed() >= self.timing.reconnect_for {
            return Ended::Unseen(Refusal::new(
              Code::TimedOut,
              format!(
                "{}; no reply for {} s. The run can go on on the host: `brd-host follow {} {run}` follows it",
                refusal.message,
                self.timing.reconnect_for.as_secs(),
                host.alias()
              ),
            ));
          }
          (self.say)(&format!(
            "brd-host: {}; following the run {run} again from byte {} in {} s",
            refusal.message,
            state.offset,
            state.backoff.as_secs()
          ));
          tokio::select! {
            () = sleep(state.backoff) => {}
            () = watch.abandoned.cancelled() => {}
          }
          state.backoff = (state.backoff * 2).min(self.timing.max_backoff);
        }
      }
    }
  }

  /// One connection of `Follow`. Answers the exit code at the end of the run
  async fn follow_once(&self, host: &mut Host, run: &str, state: &mut FollowState<'_>) -> Result<i32, Broken> {
    let (mut channel, _) = host.connect().await.map_err(classify)?;
    let request = Request::Follow {
      run: run.to_owned(),
      offset: state.offset,
    };
    channel.send(&request).await.map_err(|error| Broken::Link(host.link_refusal(&error)))?;
    loop {
      let reply = {
        let mut heard = || {
          state.last_reply = Instant::now();
          state.backoff = self.timing.first_backoff;
        };
        let reply = host.recv_beating(&mut channel, self.timing.read, &mut heard).await.map_err(classify)?;
        heard();
        reply
      };
      match reply {
        Reply::Output { stream, piece, end } => {
          // A record that a lost connection had delivered already comes again after the new `Follow`
          if end > state.offset {
            state.offset = end;
            let bytes = piece
              .into_stream_bytes()
              .map_err(|message| Broken::Refused(Refusal::new(Code::Link, format!("the agent on {} sent {message}", host.alias()))))?;
            self.put_output(stream, &bytes, state)?;
          }
        }
        // A new connection after a lost one can bring the steps again, and the last ones hold
        Reply::Phases { phases } => state.phases = phases,
        Reply::Exit { code } => {
          channel.close().await;
          return Ok(code);
        }
        Reply::Refused { code, message } => return Err(Broken::Refused(host.agent_refusal(&code, &message))),
        reply => return Err(Broken::Refused(host.unexpected(&reply))),
      }
    }
  }

  /// Gives the bytes of a piece of output to the log of the follow, else to the output sink. Without either, the stdout
  /// collects, and each complete line of the stderr goes to `say`. A write to the log that fails stops the follow
  fn put_output(&self, stream: Stream, bytes: &[u8], state: &mut FollowState<'_>) -> Result<(), Broken> {
    if let Some(mut log) = state.log {
      return log
        .write_all(bytes)
        .map_err(|error| Broken::Refused(Refusal::new(Code::Config, format!("cannot write the output of the run to its log: {error}"))));
    }
    match (stream, self.output) {
      (_, Some(output)) => output(stream, bytes),
      (Stream::Stdout, None) => state.stdout.extend_from_slice(bytes),
      (Stream::Stderr, None) => {
        state.stderr.extend_from_slice(bytes);
        while let Some(newline) = state.stderr.iter().position(|&byte| byte == b'\n') {
          let line: Vec<u8> = state.stderr.drain(..=newline).collect();
          (self.say)(&String::from_utf8_lossy(&line[..newline]));
        }
      }
    }
    Ok(())
  }

  /// Gives `say` the stderr line without its line end that the follow holds, when the client has no output sink: the
  /// line of the client that comes next stays on a line of its own
  fn say_open_stderr(&self, state: &mut FollowState<'_>) {
    if !state.stderr.is_empty() {
      (self.say)(&String::from_utf8_lossy(&state.stderr));
      state.stderr.clear();
    }
  }
}

/// Refuses a checkout with local changes: tracked or untracked files that differ from HEAD. `git status` of a large
/// checkout reads every file, so a run asks the host meanwhile
fn check_clean(checkout: &Checkout) -> Result<(), Refusal> {
  let changed = checkout.snapshot(Scan::Status, 1)?.changed;
  if changed.is_empty() {
    return Ok(());
  }
  let more = match changed.len() {
    0..=SHOWN_CHANGES => String::new(),
    count => format!("\n  (+{} more)", count - SHOWN_CHANGES),
  };
  Err(Refusal::new(
    Code::LocalChanges,
    format!(
      "--head takes the committed HEAD only, and this checkout has local changes. Commit them first, or run without --head:\n  {}{more}",
      changed[..changed.len().min(SHOWN_CHANGES)].join("\n  ")
    ),
  ))
}

/// `Client::ask` with the read deadline `deadline`
pub(crate) async fn ask_within<T>(host: &mut Host, request: &Request, deadline: Duration, take: impl FnOnce(Reply) -> Taken<T> + Send) -> Result<T, Refusal> {
  match take(host.request(request, deadline).await?.1) {
    Taken::Answer(answer) => Ok(answer),
    Taken::Other(Reply::Refused { code, message }) => Err(host.agent_refusal(&code, &message)),
    Taken::Other(reply) => Err(host.unexpected(&reply)),
  }
}

/// What `Client::ask` reads of a reply: its answer, or the reply that is no answer
pub(crate) enum Taken<T> {
  Answer(T),
  Other(Reply),
}

/// What a follow keeps across its connections
struct FollowState<'l> {
  offset: u64,
  /// The stdout of the command, without an output sink
  stdout: Vec<u8>,
  /// The bytes of the stderr after its last line end, without an output sink
  stderr: Vec<u8>,
  /// The file that takes the bytes of both streams, in place of the output sink
  log: Option<&'l File>,
  last_reply: Instant,
  backoff: Duration,
  /// The steps of the host from the last `Reply::Phases`
  phases: Vec<Phase>,
}

/// Sends `Cancel`. The agent answers as soon as the host has the request
async fn cancel_on(host: &mut Host, run: &str, deadline: Duration) -> Result<String, Refusal> {
  done(host, &Request::Cancel { run: run.to_owned() }, deadline).await
}

/// How far a run came by the steps of the host: past the start, or into its command
pub(crate) fn reached_of(phases: &[Phase]) -> Reached {
  if phases.iter().any(|phase| phase.name == "command") {
    Reached::CommandStarted
  } else {
    Reached::Started
  }
}

/// Whether a refusal is a lost link, so that a new connection can do better
pub(crate) const fn is_link(refusal: &Refusal) -> bool {
  matches!(refusal.code, Code::Unreachable | Code::TimedOut | Code::Link)
}

/// Whether a refusal comes from this client and not from the agent: the run can go on on the host
const fn is_local(refusal: &Refusal) -> bool {
  is_link(refusal)
    || matches!(
      refusal.code,
      Code::Untrusted | Code::Schema | Code::Config | Code::NoHost | Code::BadHostFile | Code::Interrupted
    )
}

/// A refusal of this client after the start, when it did not see the end of the run. The command can run on the host,
/// so the exit must be 255: a caller never runs it a second time. Only a second Ctrl+C keeps its own code. A code whose
/// exit is 75 at every point, such as `untrusted` from a new connection, becomes `link` with its name in the message
fn unseen_after_start(refusal: Refusal) -> Refusal {
  if refusal.code == Code::Interrupted || refusal.code.exit(Reached::Started) != Exit::NotRun {
    return refusal;
  }
  Refusal::new(Code::Link, format!("{}: {}", refusal.code, refusal.message))
}

/// A request whose reply is `Done`
pub(crate) async fn done(host: &mut Host, request: &Request, deadline: Duration) -> Result<String, Refusal> {
  match host.request(request, deadline).await? {
    (channel, Reply::Done { message }) => {
      channel.close().await;
      Ok(message)
    }
    (_, Reply::Refused { code, message }) => Err(host.agent_refusal(&code, &message)),
    (_, reply) => Err(host.unexpected(&reply)),
  }
}

/// A failed connect of `Follow`: a lost link connects again, anything else stops
const fn classify(refusal: Refusal) -> Broken {
  if is_link(&refusal) { Broken::Link(refusal) } else { Broken::Refused(refusal) }
}

/// Adds a commit to the candidates when it is a new one
fn add_candidate(candidates: &mut Vec<String>, commit: Option<&str>) {
  if let Some(commit) = commit
    && !commit.is_empty()
    && !candidates.iter().any(|known| known == commit)
  {
    candidates.push(commit.to_owned());
  }
}

/// The files of a run that came back
#[derive(Debug, Default)]
pub(crate) struct Fetched {
  /// The staged files and the files of the outputs, relative to the checkout root with `/` separators
  pub(crate) files: Vec<String>,
  /// The patch of the changes of the command, when it came
  pub(crate) changes: Option<PathBuf>,
}

/// The refusal of a run that vanished on the host after it started: the command may have run, and there is nothing
/// left to forget
pub(crate) fn vanished(refusal: &Refusal, run: &str, alias: &str) -> Refusal {
  Refusal::new(
    refusal.code,
    format!(
      "{}: the run {run} started on {alias} and is gone there, so whether its command ran is unknown. `brd-host runs {alias}` lists the runs that {alias} keeps",
      refusal.message
    ),
  )
}

/// The line of a started run: `brd-host: run <id> of <commit> on <alias>`, then the repository when it is not the one of
/// the host file, and what the commit holds beyond HEAD
fn run_line(run: &str, alias: &str, snapshot: &Snapshot, repo: &str, default_repo: &str) -> String {
  let mut line = format!("brd-host: run {run} of {} on {alias}", short(&snapshot.commit));
  if repo != default_repo {
    line.push_str(" in ");
    line.push_str(repo);
  }
  if snapshot.commit != snapshot.head {
    let _ = write!(line, ": the working tree, HEAD {} and {} changed paths", short(&snapshot.head), snapshot.changed.len());
  }
  line
}

/// The repository `name` beside `repo` on the host: `~/projects/branch-deck` beside `~/projects/idea-1`
fn sibling(repo: &str, name: &str) -> Option<String> {
  let parent = repo.trim_end_matches('/').rsplit_once('/')?.0;
  (!parent.is_empty()).then(|| format!("{parent}/{name}"))
}

/// The id of a new run: `<epoch ms>-<pid>`
fn run_id() -> String {
  let millis = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis());
  format!("{millis}-{}", std::process::id())
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;

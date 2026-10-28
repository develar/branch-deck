//! A run on a host, and the requests of one reply (spec/cli/host.spec.md, "run" and "follow, cancel, fetch, forget,
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
//! 2. Writes the run record (`record`) and starts the run (`Start`). The run lives on the agent, not on the connection.
//!    Ctrl+C before this step stops the local work, and nothing runs on the host. A start whose reply does not come is
//!    cancelled, so the host does not wait for its pack.
//! 3. Follows the output (`Follow`) on a thread of its own from the start on, so the steps of the host show while the
//!    pack builds. Each stderr line goes to the stderr sink as it comes, and the stdout lines to the stdout sink. A
//!    connection that is quiet for 120 s is lost: the client connects again from the last offset, with a backoff, for up
//!    to 10 min after the last reply of the agent. `Reply::Phases` before the end gives the steps of the host.
//! 4. Meanwhile it builds the thin pack of the commits that the host lacks, while the host takes the slot, and sends it
//!    (`Pack` with `PackTarget::Run`) on a connection of its own (`pack`). When the pack fails, the client cancels the
//!    run, stops the follow, and refuses with the code of the pack.
//! 5. After the pack, a thread syncs the base of the next packs (`base`).
//! 6. Copies the staged files of the `fetch` directories, the outputs and the changes into the local checkout (`Fetch`),
//!    then removes the run on the agent (`Forget`). The fetch takes only what the run record asked for. The client
//!    removes the run only when it saw the end of the run and every file came. Otherwise the run stays on the agent,
//!    and the client names `bd-host fetch` and `bd-host forget`.
//! 7. Waits for the base sync. With `RunOptions::apply`, it applies the patch of the changes of the command to the
//!    working tree (the files only, not the index). Then it answers the steps of the run with their durations.
//!
//! A refusal carries the exit of `bd-host` by how far the run came (`host_link::refusal::Reached`): a refusal before the
//! command exits 75, so the caller can run it locally instead.
//!
//! Ctrl+C after the start, the build of the pack included, cancels the run on the host. The first press sends `Cancel`
//! on a thread of its own and stops the pack, and the output follows up to its end. The second press stops the follow
//! at once. A press after the end of the run closes the connection of the base sync

use crate::base::BaseSync;
use crate::git::{Checkout, Snapshot};
use crate::host::{AgentInfo, Host, Latest, another_build_hint};
use crate::pack::{PackOf, plan_pack};
use crate::phases::{phase, phases_line};
use crate::record::{RunFiles, RunRecord};
use crate::{Client, Timing, short};
use git_ops::host_client::{FARTHEST, Scan};
use host_link::channel::Closer;
use host_link::proto::{PackTarget, Phase, Reply, Request, RunInfo, RunSpec, Stream};
use host_link::refusal::{Code, Exit, Reached, Refusal};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sync_utils::span::in_current_span;

/// The largest first-parent distance from HEAD of the first `Have` of a run. git walks 17 commits for it, and a host
/// that ran a recent commit of the checkout has one of them
const NEAR: usize = 16;

/// How many changed paths the refusal of local changes shows
const SHOWN_CHANGES: usize = 5;

/// How often the watch of a run asks whether the person pressed Ctrl+C
const INTERRUPT_POLL: Duration = Duration::from_millis(100);

/// How long the cancel of a run connects again after a failed try
const CANCEL_FOR: Duration = Duration::from_secs(120);

/// How long the end of a run waits for the last lines of its cancel
const CANCEL_GRACE: Duration = Duration::from_millis(500);

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

/// What a run adds to a `RunRequest`. The default is the run of bt: the committed HEAD of a checkout without local
/// changes, the niceness of the agent, the repository of the host file, no outputs and no changes
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[expect(
  clippy::struct_excessive_bools,
  reason = "each bool is one option of a run that no other option depends on, as the flags of the CLI are"
)]
pub struct RunOptions {
  /// Run the working tree: HEAD plus the tracked and untracked changes that git does not ignore, as one commit.
  /// Without it, a checkout with local changes is refused
  pub working_tree: bool,
  /// The niceness of the prepare steps and the command on the host, 0 to 19
  pub nice: Option<u8>,
  /// Bring back the outputs that `[host]` of the commit names
  pub outputs: bool,
  /// Bring back the changes of the command and apply them to the working tree
  pub apply: bool,
  /// When `RunRequest::repo` names none: try the repository of the same name as the local one beside the repository
  /// of the host file first, such as `~/projects/branch-deck` beside `~/projects/idea-1`
  pub repo_by_name: bool,
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
  /// The stdout lines of the command, each with its line end; empty with `Client::with_stdout`
  pub stdout: String,
  pub facts: RunFacts,
}

/// A run that refused
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunFailure {
  pub refusal: Refusal,
  /// The exit of `bd-host`: the code of the refusal at the point the run reached
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
  pub stdout: String,
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
enum Ended {
  /// The command exited
  Exit(Followed),
  /// The agent ended the run with a refusal of its own, such as `lease_failed`, after the steps of the host
  Refused(Refusal, Vec<Phase>),
  /// The client did not see the end: the link is lost, or a second Ctrl+C stopped the follow
  Unseen(Refusal),
}

/// What the watch of a run and the follow share
struct Watch {
  /// The follow ended
  finished: Arc<AtomicBool>,
  /// The connection of the follow, which a second Ctrl+C closes
  current: Mutex<Option<Closer>>,
  /// A second Ctrl+C came, or the pack of the run failed: the follow stops
  abandoned: AtomicBool,
}

impl Watch {
  fn new() -> Self {
    Self {
      finished: Arc::new(AtomicBool::new(false)),
      current: Mutex::new(None),
      abandoned: AtomicBool::new(false),
    }
  }

  /// Stops the follow at once: it closes the connection of the follow, and no new one comes
  fn stop_follow(&self) {
    if !self.abandoned.swap(true, Ordering::AcqRel)
      && let Some(closer) = self.current.lock().unwrap_or_else(PoisonError::into_inner).take()
    {
      closer.close();
    }
  }
}

/// How a followed run ended, after its fetch
struct Finished {
  followed: Followed,
  fetched: Result<Fetched, Refusal>,
  /// The step `fetch`, when the run names fetch directories, outputs or changes
  fetch: Option<Phase>,
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
  pub fn run(&self, alias: &str, request: &RunRequest<'_>, options: RunOptions) -> Result<RunOutcome, RunFailure> {
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
      .map_err(|refusal| before(refusal, &facts))?;
    facts.commit = Some(snapshot.commit.clone());
    facts.head = Some(snapshot.head.clone());
    facts.repository = Some(repo.clone());
    let commit = snapshot.commit.clone();
    let files = RunFiles::new(&checkout.runs_dir()).map_err(|refusal| before(refusal, &facts))?;
    let outputs = if options.outputs {
      checkout.host_config(&commit).map_err(|refusal| before(refusal, &facts))?.outputs
    } else {
      Vec::new()
    };
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
    };
    files.write(&run, &record).map_err(|refusal| before(refusal, &facts))?;
    facts.run = Some(run.clone());
    // A start whose reply did not come is cancelled, and the checkout of the run waits for a pack that never comes
    self.start(&mut host, spec).map_err(|refusal| before(refusal, &facts))?;
    (self.say)(&run_line(&run, alias, &snapshot, &repo, &host.file().repo));

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
    let root = checkout.root().to_path_buf();
    let ended = thread::scope(|scope| {
      let mut followed = host.clone();
      let (run, watch, record) = (&run, &watch, &record);
      let patch = files.patch(run);
      let root = &root;
      let following = scope.spawn(in_current_span(move || self.follow_and_fetch(&mut followed, run, root, record, &patch, watch)));
      if let Some(plan) = &plan {
        match self.send_pack(&mut host, &pack, plan, self.say) {
          Ok(steps) => {
            facts.phases.push(phase("pack", steps.build));
            facts.phases.push(phase("upload", steps.upload));
          }
          // The watch of the follow sent the cancel of the press, and the output follows up to its end
          Err(_) if self.interrupted() => {}
          Err(refusal) => {
            let refusal = self.abandon(&mut host, run, &format!("the pack did not reach {alias}"), &refusal);
            watch.stop_follow();
            let _ = following.join();
            // The checkout waits for the pack, so the command never started
            return Err((refusal, Reached::Nothing, Vec::new()));
          }
        }
      }
      let latest = Arc::new(Latest::default());
      let mut synced = host.watched(Arc::clone(&latest));
      let syncing = scope.spawn(in_current_span(move || self.sync_base(&mut synced, &sync)));
      let finished = following
        .join()
        .unwrap_or_else(|_| Err((Refusal::new(Code::Link, "the follow of the run failed"), Reached::Started, Vec::new())));
      if !syncing.is_finished() {
        (self.say)(&format!("> {alias}: the sync of the base of the next packs goes on; Ctrl+C stops it"));
      }
      while !syncing.is_finished() {
        if self.interrupted() {
          latest.close();
        }
        thread::sleep(INTERRUPT_POLL);
      }
      let _ = syncing.join();
      finished
    });
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
    facts.phases.extend(finished.followed.phases.iter().cloned());
    facts.phases.extend(finished.fetch);
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
            "the command of the run {run} exited with {code}, and its changes did not come ({}): `bd-host fetch {alias} {run}` copies them into the git directory of this checkout",
            refusal.message
          ),
        );
        return Err(after_command(refusal, &facts));
      }
      Err(refusal) => return Err(after_command(refusal, &facts)),
    };
    facts.fetched.clone_from(&fetched.files);
    if options.apply {
      facts.applied_files = Some(self.apply_changes(&checkout, &run, &fetched, code).map_err(|refusal| after_command(refusal, &facts))?);
    }
    facts.phases.push(phase("total", started.elapsed()));
    Ok(RunOutcome {
      code,
      stdout: finished.followed.stdout,
      facts,
    })
  }

  /// The snapshot of the run, the repository on the host, and the candidates that it has. Without the working tree, a
  /// checkout with local changes refuses at once: its refusal closes the connection of the `Have`
  fn find(&self, host: &mut Host, checkout: &Checkout, request: &RunRequest<'_>, options: RunOptions, recorded: Option<&str>) -> Result<Found, Refusal> {
    if options.working_tree {
      let snapshot = checkout.snapshot(Scan::WorkingTree, NEAR)?;
      self.refuse_interrupt()?;
      let (repo, present) = self.present_in_repo(host, checkout, request, options, &snapshot, recorded)?;
      return Ok(Found { snapshot, repo, present });
    }
    let snapshot = checkout.snapshot(Scan::Head, NEAR)?;
    self.refuse_interrupt()?;
    let asked = Arc::new(Latest::default());
    let (clean, found) = thread::scope(|scope| {
      let mut asking = host.watched(Arc::clone(&asked));
      let snapshot = &snapshot;
      let found = scope.spawn(in_current_span(move || self.present_in_repo(&mut asking, checkout, request, options, snapshot, recorded)));
      let clean = check_clean(checkout);
      if clean.is_err() {
        asked.close();
      }
      let found = found
        .join()
        .unwrap_or_else(|_| Err(Refusal::new(Code::Link, "the question about the commits of the host failed")));
      (clean, found)
    });
    clean?;
    let (repo, present) = found?;
    Ok(Found { snapshot, repo, present })
  }

  /// The repository of the run on the host, and the candidates of the commit that it has. With
  /// `RunOptions::repo_by_name`, a host that lacks the repository of the local name gets the repository of the host
  /// file instead
  fn present_in_repo(
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
      Some(named) => match self.present(host, named, checkout, &snapshot.candidates, &extra) {
        Err(refusal) if refusal.code == Code::RepoMissing && request.repo.is_none() => {
          (self.say)(&format!(
            "bd-host: {} has no {named}; the run goes to {default}, the repository of its host file. --repo names another one",
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
      None => Ok((default.clone(), self.present(host, &default, checkout, &snapshot.candidates, &extra)?)),
    }
  }

  /// Applies the patch of the changes of the command to the working tree. Answers the count of its files. A patch that
  /// does not apply stays, and the refusal names it
  fn apply_changes(&self, checkout: &Checkout, run: &str, fetched: &Fetched, code_of_command: i32) -> Result<u64, Refusal> {
    let Some(patch) = &fetched.changes else {
      (self.say)("bd-host: the command changed no file");
      return Ok(0);
    };
    match checkout.apply(patch) {
      Ok(files) => {
        (self.say)(&format!(
          "bd-host: applied the changes of the command to {files} files of this working tree; `git apply -R {}` undoes them",
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

  /// Sends `Start`. A reply that does not come can still leave the run on the agent, so the client cancels it then
  fn start(&self, host: &mut Host, spec: RunSpec) -> Result<(), Refusal> {
    let run = spec.run.clone();
    let (mut channel, agent) = host.connect()?;
    if let Some(hint) = another_build_hint(host.alias(), &agent, self.build.sha256.as_deref()) {
      (self.say)(&hint);
    }
    channel.send(&Request::Start(spec)).map_err(|error| host.link_refusal(&error))?;
    match host.recv(&mut channel, self.timing.read) {
      Ok(Reply::Started { .. }) => Ok(()),
      Ok(Reply::Refused { code, message }) => Err(host.agent_refusal(&code, &message)),
      Ok(reply) => Err(host.unexpected(&reply)),
      Err(refusal) if is_link(&refusal) => {
        let what = format!("the start of the run {run} got no reply from {}", host.alias());
        Err(self.abandon(host, &run, &what, &refusal))
      }
      Err(refusal) => Err(refusal),
    }
  }

  /// Follows a started run to its end, then copies the files that `record` asks for, with the changes to `patch`, and
  /// removes the run on the host. A run that the agent refused is removed too, and its failure carries how far it came
  /// and the steps of the host. A run that vanished on the host after it started (`unknown_run`) may have run its
  /// command, so its exit is 255, and a caller never runs the command a second time
  fn follow_and_fetch(&self, host: &mut Host, run: &str, root: &Path, record: &RunRecord, patch: &Path, watch: &Watch) -> Result<Finished, (Refusal, Reached, Vec<Phase>)> {
    let alias = host.alias().to_owned();
    let followed = match self.follow(host, run, true, watch) {
      Ended::Exit(followed) => followed,
      Ended::Refused(refusal, steps) if refusal.code == Code::UnknownRun => return Err((vanished(&refusal, run, &alias), Reached::Started, steps)),
      Ended::Refused(refusal, steps) => {
        self.forget_or_say(host, run);
        let reached = if steps.iter().any(|step| step.name == "command") {
          Reached::CommandStarted
        } else {
          Reached::Started
        };
        return Err((refusal, reached, steps));
      }
      Ended::Unseen(refusal) => return Err((refusal, Reached::Started, Vec::new())),
    };
    if !record.asks_for_files() {
      self.forget_or_say(host, run);
      return Ok(Finished {
        followed,
        fetched: Ok(Fetched::default()),
        fetch: None,
      });
    }
    let fetching = Instant::now();
    let fetched = self.fetch_run(host, run, root, record, patch);
    let fetch = Some(phase("fetch", fetching.elapsed()));
    let fetched = match fetched {
      Ok(fetched) => {
        self.forget_or_say(host, run);
        Ok(fetched)
      }
      Err(refusal) => {
        let message = format!(
          "cannot copy the files of the run {run} from {alias}: {}. The run stays on {alias}: `bd-host fetch {alias} {run}` in this checkout copies its files again, and `bd-host forget {alias} {run}` removes it",
          refusal.message
        );
        let code = if refusal.code == Code::FetchFailed || is_link(&refusal) {
          Code::FetchFailed
        } else {
          refusal.code
        };
        (self.say)(&format!("bd-host: {message}"));
        Err(Refusal::new(code, message))
      }
    };
    Ok(Finished { followed, fetched, fetch })
  }

  /// Cancels a started run whose start or pack failed, so its supervisor does not wait for the pack. Answers the
  /// refusal with its code, the failed step `what`, and what became of the run
  fn abandon(&self, host: &mut Host, run: &str, what: &str, refusal: &Refusal) -> Refusal {
    let alias = host.alias().to_owned();
    let mut backoff = self.timing.first_backoff;
    let mut tries = 1;
    let cancelled = loop {
      match cancel_on(host, run, self.timing.read) {
        Err(failed) if is_link(&failed) && tries < ABANDON_TRIES => {
          tries += 1;
          thread::sleep(backoff);
          backoff = (backoff * 2).min(self.timing.max_backoff);
        }
        cancelled => break cancelled,
      }
    };
    let fate = match cancelled {
      Ok(_) => format!("{alias} has the cancel of the run {run}"),
      Err(failed) => format!("cannot cancel the run {run} on {alias}: {}. `bd-host cancel {alias} {run}` cancels it", failed.message),
    };
    Refusal::new(refusal.code, format!("{what}: {}; {fate}", refusal.message))
  }

  /// Follows a run from its first line, as `run` does. Ctrl+C here cancels nothing: the caller leaves, and the run goes
  /// on
  pub fn follow_run(&self, alias: &str, run: &str) -> Result<Followed, Refusal> {
    let mut host = self.host(alias)?;
    match self.follow(&mut host, run, false, &Watch::new()) {
      Ended::Exit(followed) => Ok(followed),
      Ended::Refused(refusal, _) | Ended::Unseen(refusal) => Err(refusal),
    }
  }

  /// Asks the agent to interrupt a run. The answer comes as soon as the host has the request; the run ends later, and
  /// `follow_run` shows its end
  pub fn cancel(&self, alias: &str, run: &str) -> Result<String, Refusal> {
    let mut host = self.host(alias)?;
    cancel_on(&mut host, run, self.timing.read)
  }

  /// The runs that the agent keeps
  pub fn runs(&self, alias: &str) -> Result<Vec<RunInfo>, Refusal> {
    self.ask(&mut self.host(alias)?, &Request::Runs, |reply| match reply {
      Reply::Runs { runs } => Taken::Answer(runs),
      reply => Taken::Other(reply),
    })
  }

  /// The lend table of each repository that the agent served, as `host-slots.json` holds it
  pub fn slots(&self, alias: &str) -> Result<Vec<host_link::proto::RepositorySlots>, Refusal> {
    self.ask(&mut self.host(alias)?, &Request::Slots, |reply| match reply {
      Reply::Slots { repositories } => Taken::Answer(repositories),
      reply => Taken::Other(reply),
    })
  }

  /// One request on a new connection, and its reply that `take` reads. A refusal of the agent keeps its code, and a
  /// reply that `take` gives back is out of turn
  fn ask<T>(&self, host: &mut Host, request: &Request, take: impl FnOnce(Reply) -> Taken<T>) -> Result<T, Refusal> {
    match take(host.request(request, self.timing.read)?.1) {
      Taken::Answer(answer) => Ok(answer),
      Taken::Other(Reply::Refused { code, message }) => Err(host.agent_refusal(&code, &message)),
      Taken::Other(reply) => Err(host.unexpected(&reply)),
    }
  }

  /// The hello of the agent: its version, its build and its platform, and the address that answered
  pub fn hello(&self, alias: &str) -> Result<AgentInfo, Refusal> {
    let mut host = self.host(alias)?;
    let (channel, info) = host.connect()?;
    channel.close();
    Ok(info)
  }

  /// The commits of `near` and `extra` that the host has. `near` are the candidates of the snapshot, which git finds at
  /// once. Only a host that has none of them gets a second `Have` with the first-parent walk up to `FARTHEST`, which
  /// reads up to 65537 commits
  pub(crate) fn present(&self, host: &mut Host, repo: &str, checkout: &Checkout, near: &[String], extra: &[Option<&str>]) -> Result<Vec<String>, Refusal> {
    let ask = |candidates: &[String], host: &mut Host| {
      let mut found = Vec::new();
      for candidate in candidates.iter().map(String::as_str).map(Some).chain(extra.iter().copied()) {
        add_candidate(&mut found, candidate);
      }
      self.have(host, repo, found)
    };
    let present = ask(near, host)?;
    if !present.is_empty() {
      return Ok(present);
    }
    let far = checkout.snapshot(Scan::Head, FARTHEST)?;
    ask(&far.candidates, host)
  }

  /// The commits of `candidates` that the host has
  pub(crate) fn have(&self, host: &mut Host, repo: &str, candidates: Vec<String>) -> Result<Vec<String>, Refusal> {
    let request = Request::Have {
      repo: repo.to_owned(),
      commits: candidates,
    };
    self.ask(host, &request, |reply| match reply {
      Reply::Have { commits } => Taken::Answer(commits),
      reply => Taken::Other(reply),
    })
  }

  fn refuse_interrupt(&self) -> Result<(), Refusal> {
    if self.interrupted() {
      Err(Refusal::new(Code::Interrupted, "Ctrl+C came before the run started; nothing runs on the host"))
    } else {
      Ok(())
    }
  }

  fn forget_or_say(&self, host: &mut Host, run: &str) {
    if let Err(refusal) = self.forget_on(host, run) {
      (self.say)(&format!(
        "bd-host: cannot remove the run {run} on {}: {}. `bd-host forget {} {run}` removes it",
        host.alias(),
        refusal.message,
        host.alias()
      ));
    }
  }

  /// Follows the output of a run up to its end. With `cancel_on_interrupt`, the first Ctrl+C sends `Cancel`, and the
  /// second one stops the follow. `Watch::stop_follow` stops it from another thread
  fn follow(&self, host: &mut Host, run: &str, cancel_on_interrupt: bool, watch: &Watch) -> Ended {
    let watched = host.clone();
    thread::scope(|scope| {
      if cancel_on_interrupt {
        scope.spawn(in_current_span(|| self.watch_interrupts(&watched, run, watch)));
      }
      let ended = self.follow_loop(host, run, watch);
      watch.finished.store(true, Ordering::Release);
      ended
    })
  }

  /// Reads the Ctrl+C count while the run goes on. The cancel runs on a thread that the scope does not wait for, so a
  /// cancel that hangs never holds the end of the run
  fn watch_interrupts(&self, host: &Host, run: &str, watch: &Watch) {
    let mut cancel: Option<mpsc::Receiver<String>> = None;
    while !watch.finished.load(Ordering::Acquire) {
      let presses = (self.presses)();
      if presses >= 1 && cancel.is_none() {
        (self.say)(&format!(
          "bd-host: Ctrl+C: cancelling the run {run} on {}; its output follows up to its end. A second Ctrl+C stops the follow",
          host.alias()
        ));
        cancel = Some(start_cancel(host.clone(), run.to_owned(), self.timing, Arc::clone(&watch.finished)));
      }
      if presses >= 2 {
        watch.stop_follow();
      }
      if let Some(said) = &cancel {
        while let Ok(line) = said.try_recv() {
          (self.say)(&line);
        }
      }
      thread::sleep(INTERRUPT_POLL);
    }
    // The answer of the cancel can come just after the end of the run; it gets a moment, not more
    if let Some(said) = cancel {
      let until = Instant::now() + CANCEL_GRACE;
      while let Ok(line) = said.recv_timeout(until.saturating_duration_since(Instant::now())) {
        (self.say)(&line);
      }
    }
  }

  fn follow_loop(&self, host: &mut Host, run: &str, watch: &Watch) -> Ended {
    let mut state = FollowState {
      offset: 0,
      stdout: String::new(),
      last_reply: Instant::now(),
      backoff: self.timing.first_backoff,
      phases: Vec::new(),
    };
    loop {
      let once = self.follow_once(host, run, watch, &mut state);
      if watch.abandoned.load(Ordering::Acquire) {
        return Ended::Unseen(Refusal::new(
          Code::Interrupted,
          format!(
            "a second Ctrl+C stopped the follow of the run {run}; {} has its cancel. `bd-host follow {} {run}` shows its end",
            host.alias(),
            host.alias()
          ),
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
              refusal.code,
              format!(
                "{}; no reply for {} s. The run can go on on the host: `bd-host follow {} {run}` follows it",
                refusal.message,
                self.timing.reconnect_for.as_secs(),
                host.alias()
              ),
            ));
          }
          (self.say)(&format!(
            "bd-host: {}; following the run {run} again from byte {} in {} s",
            refusal.message,
            state.offset,
            state.backoff.as_secs()
          ));
          sleep_unless(state.backoff, &watch.abandoned);
          state.backoff = (state.backoff * 2).min(self.timing.max_backoff);
        }
      }
    }
  }

  /// One connection of `Follow`. Answers the exit code at the end of the run
  fn follow_once(&self, host: &mut Host, run: &str, watch: &Watch, state: &mut FollowState) -> Result<i32, Broken> {
    let (mut channel, _) = host.connect().map_err(classify)?;
    *watch.current.lock().unwrap_or_else(PoisonError::into_inner) = channel.closer().ok();
    if watch.abandoned.load(Ordering::Acquire) {
      return Err(Broken::Link(Refusal::new(Code::Interrupted, "a second Ctrl+C")));
    }
    let request = Request::Follow {
      run: run.to_owned(),
      offset: state.offset,
    };
    channel.send(&request).map_err(|error| Broken::Link(host.link_refusal(&error)))?;
    loop {
      let mut heard = || {
        state.last_reply = Instant::now();
        state.backoff = self.timing.first_backoff;
      };
      let reply = host.recv_beating(&mut channel, self.timing.read, &mut heard).map_err(classify)?;
      heard();
      match reply {
        Reply::Output { stream, text, end } => {
          // A record that a lost connection had delivered already comes again after the new `Follow`
          if end > state.offset {
            state.offset = end;
            match (stream, self.stdout) {
              (Stream::Stderr, _) => (self.say)(&text),
              (Stream::Stdout, Some(stdout)) => stdout(&text),
              (Stream::Stdout, None) => {
                state.stdout.push_str(&text);
                state.stdout.push('\n');
              }
            }
          }
        }
        // A new connection after a lost one can bring the steps again, and the last ones hold
        Reply::Phases { phases } => state.phases = phases,
        Reply::Exit { code } => {
          channel.close();
          return Ok(code);
        }
        Reply::Refused { code, message } => return Err(Broken::Refused(host.agent_refusal(&code, &message))),
        reply => return Err(Broken::Refused(host.unexpected(&reply))),
      }
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

/// What `Client::ask` reads of a reply: its answer, or the reply that is no answer
enum Taken<T> {
  Answer(T),
  Other(Reply),
}

/// What a follow keeps across its connections
struct FollowState {
  offset: u64,
  stdout: String,
  last_reply: Instant,
  backoff: Duration,
  /// The steps of the host from the last `Reply::Phases`
  phases: Vec<Phase>,
}

/// Sends `Cancel`. The agent answers as soon as the host has the request
fn cancel_on(host: &mut Host, run: &str, deadline: Duration) -> Result<String, Refusal> {
  done(host, &Request::Cancel { run: run.to_owned() }, deadline)
}

/// Sends `Cancel` on a thread of its own, and answers the lines that it has to say. It connects again with the backoff
/// of `Follow` until the agent has the cancel, the run has ended, or `CANCEL_FOR` has passed
fn start_cancel(mut host: Host, run: String, timing: Timing, finished: Arc<AtomicBool>) -> mpsc::Receiver<String> {
  let (said, lines) = mpsc::channel();
  let failed = said.clone();
  let cancelling = thread::Builder::new().name("bd-host-cancel".to_owned()).spawn(in_current_span(move || {
    let started = Instant::now();
    let mut backoff = timing.first_backoff;
    loop {
      match cancel_on(&mut host, &run, timing.read) {
        Ok(message) => {
          let _ = said.send(format!("bd-host: {message}"));
          return;
        }
        Err(refusal) if is_link(&refusal) && started.elapsed() < CANCEL_FOR && !finished.load(Ordering::Acquire) => {
          let _ = said.send(format!(
            "bd-host: cannot send the cancel of the run {run}: {}; again in {} s",
            refusal.message,
            backoff.as_secs()
          ));
          thread::sleep(backoff);
          backoff = (backoff * 2).min(timing.max_backoff);
        }
        Err(refusal) => {
          let _ = said.send(format!("bd-host: cannot cancel the run {run}: {}", refusal.message));
          return;
        }
      }
    }
  }));
  if let Err(error) = cancelling {
    let _ = failed.send(format!("bd-host: cannot start the cancel: {error}"));
  }
  lines
}

/// Sleeps for `time`, and wakes early when `stop` turns `true`
fn sleep_unless(time: Duration, stop: &AtomicBool) {
  let until = Instant::now() + time;
  while !stop.load(Ordering::Acquire) {
    let left = until.saturating_duration_since(Instant::now());
    if left.is_zero() {
      return;
    }
    thread::sleep(left.min(INTERRUPT_POLL));
  }
}

/// Whether a refusal is a lost link, so that a new connection can do better
const fn is_link(refusal: &Refusal) -> bool {
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

/// A request whose reply is `Done`
pub(crate) fn done(host: &mut Host, request: &Request, deadline: Duration) -> Result<String, Refusal> {
  match host.request(request, deadline)? {
    (channel, Reply::Done { message }) => {
      channel.close();
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
fn vanished(refusal: &Refusal, run: &str, alias: &str) -> Refusal {
  Refusal::new(
    refusal.code,
    format!(
      "{}: the run {run} started on {alias} and is gone there, so whether its command ran is unknown. `bd-host runs {alias}` lists the runs that {alias} keeps",
      refusal.message
    ),
  )
}

/// The line of a started run: `bd-host: run <id> of <commit> on <alias>`, then the repository when it is not the one of
/// the host file, and what the commit holds beyond HEAD
fn run_line(run: &str, alias: &str, snapshot: &Snapshot, repo: &str, default_repo: &str) -> String {
  let mut line = format!("bd-host: run {run} of {} on {alias}", short(&snapshot.commit));
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

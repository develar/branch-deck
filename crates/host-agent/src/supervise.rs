//! `bd-host supervise <run dir> --lock-fd <descriptor>`: one run, from the lend of its host slot to `exit.json`
//! (spec/cli/host.spec.md, "serve"). `serve` starts it in a session of its own, so a restart or an upgrade of `serve`
//! does not end the run. It keeps the host awake for its whole life (`awake`), also when its client is gone. `serve`
//! takes the flock of the run before the run is visible and hands its descriptor over. The supervisor holds it for its
//! whole life, so a run is alive exactly while its supervisor is.
//!
//! The steps:
//!
//! 1. Lend a host slot of the repository (`slots::lend`, with the pid of this process): the slot that the affinity used
//!    last, else the one used longest ago, else a place in the queue. While Homebrew upgrades the git of the host, the
//!    run waits in the phase `git`. While every slot is lent, the run waits up to an hour, and the queue line goes to
//!    the log once a minute. A git below its floor ends the run with `old_git`.
//! 2. Wait for the pack, up to `PACK_WAIT_SECONDS` of the protocol, and up to `PACK_STALL_SECONDS` without a chunk of
//!    it. A start can come before the pack of its commit, so the lend goes on while the pack travels. A commit that the
//!    repository has already needs no pack. Else the run waits for the file `packed`, which the pack writes after its
//!    ref. A watch of the run directory ends the wait at once.
//! 3. Check out the commit in the slot by the diff (`slots::checkout`). The give-back of the last run discarded its
//!    changes; the ignored files stay as the warm cache.
//! 4. Prepare: read `[host]` of the commit, build the environment (`run_env`, with the login environment of `login_env`
//!    and the directory of the git of the host before `/usr/bin`, `run_git::for_run`), and run each prepare step whose
//!    inputs changed since its last success in this slot (`prepare`). For a client that asks for them, remove the
//!    outputs of the config, and take the tree of the slot before the command (`repo::Repository::snapshot`).
//! 5. Run the command in the slot root, in its own process group, with stdin `/dev/null`, the environment of step 4 and
//!    the niceness of the spec. Its lines go to `events.log`. When the command exits, the other processes of its group
//!    get SIGTERM, and SIGKILL 5 s later.
//! 6. Stage: the patch of the changes of the command, the outputs, and the new entries of each `fetch` directory, into
//!    the run directory: nothing reads the slot after the run.
//!
//! Each step is a phase: `git` (only after a wait), `slot`, `pack_wait`, `checkout`, `prepare`, `command` and `stage`.
//! At the end of each step, whatever its outcome, the supervisor rewrites `phases.json` with the steps that have ended.
//!
//! On every way out it keeps the fingerprints of the slot, and writes `exit.json` last. Then it drops the flock of the
//! run and gives the slot back (`slots::give_back`): the processes that the run left in the slot stop, its changes go,
//! the ref of its pack goes, and a run that got no slot leaves the queue. No signal cuts the give-back, and its lines go
//! to `supervisor.log`. The `cancel` file of the run, or SIGINT, SIGTERM or SIGHUP, interrupts the run. Before the
//! command starts, the run ends with the exit code 130 after the current step; a checkout and a scan of the slot run to
//! their end, since the pool runs their git in process. After the command started, the command group gets SIGINT, and
//! SIGKILL after the grace time. The progress lines go to `events.log` as stderr events with the prefix `bd-host: `. A
//! prepare step writes both of its streams as stderr events, so the stdout of a run holds only the lines of its command

use crate::files::{FILE_MODE, open_private, private_dir, remove_entry, try_flock};
use crate::home::Home;
use crate::log::supervisor;
use crate::login_env::LoginEnv;
use crate::prepare::Prepared;
use crate::repo::Repository;
use crate::run::{EventLog, ExitRecord, MAX_EVENT_TEXT, RunDir, epoch_ms};
use crate::{awake, login_env, prepare, run_env, run_git, slots, sys, tuning};
use host_link::config::{FILE, RepoConfig, YONDER_FILE, parse_branchdeck, resolve};
use host_link::proto::{Phase, RunSpec, Stream};
use host_link::refusal::{Code, Refusal};
use lease_pool::host_git::with_first;
use lease_pool::host_slots::{Lend, LendAnswer, SlotCheckout};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, File, Permissions};
use std::io::{self, BufRead, BufReader, Read};
use std::os::fd::RawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Child, ExitStatus, Stdio};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use sync_utils::child_process::windowless_command;
use sync_utils::fs_retry;
use sync_utils::span::in_current_span;

/// How long a run waits for a host slot
const SLOT_WAIT: Duration = Duration::from_hours(1);

/// How long a run waits while Homebrew upgrades the git of the host: the pool waits up to 60 min for the lends to end,
/// and Homebrew takes up to 30 min
const GIT_WAIT: Duration = Duration::from_mins(100);

/// The wait of one lend while Homebrew upgrades the git of the host, so the run sees the end of the upgrade
const GIT_POLL: Duration = Duration::from_secs(5);

/// How often the supervisor looks at the command and at the cancel request
const POLL: Duration = Duration::from_millis(100);

/// The longest wait of the pack wait for a change of the run directory. It bounds the time until the wait sees a stall
/// or a stop
const PACK_RECHECK: Duration = Duration::from_secs(1);

/// A wait for the pack that is longer than this gets a progress line
const PACK_WAIT_SAY: Duration = Duration::from_secs(1);

/// How long the supervisor reads the pipes of a command group that has ended. A daemon can keep them open
const DRAIN: Duration = Duration::from_secs(5);

/// How long the supervisor waits for the command after SIGKILL
const KILL_WAIT: Duration = Duration::from_secs(10);

/// How long the group of a command that has exited can take to empty by itself
const GROUP_SETTLE: Duration = Duration::from_millis(500);

/// How long the other processes of the group have after SIGTERM, once the command has exited
const GROUP_TERM_WAIT: Duration = Duration::from_secs(5);

/// The exit code of a run that a cancel ended before its command started: 128 plus SIGINT
const INTERRUPTED: i32 = 128 + libc::SIGINT;

/// The prefix of the lines of the agent in the log of a run
const SAY: &str = "bd-host: ";

/// Runs the run of `run_dir` to its end, and writes `exit.json`. `lock` is the descriptor of the flock of the run that
/// `serve` took. Without it, the supervisor takes the flock itself
pub fn supervise(home: &Home, run_dir: &Path, lock: Option<RawFd>) {
  sys::catch_stop_signals();
  awake::hold_for_life();
  if let Err(error) = run_env::raise_open_files() {
    supervisor(&format!("cannot raise the open-files limit: {error}"));
  }
  let run = RunDir::new(run_dir.to_path_buf());
  let lock = match adopt_lock(&run, lock) {
    Ok(lock) => lock,
    Err(error) => {
      supervisor(&format!("cannot hold the lock of {}: {error}", run_dir.display()));
      return;
    }
  };
  // read before `exit.json`: once it exists, a client can forget the run and its directory goes
  let spec = run.spec().ok();
  let mut repo = None;
  let outcome = run_to_end(home, &run, &mut repo);
  let exit = match outcome {
    Ok(code) => ExitRecord::Code { code },
    Err(refusal) => ExitRecord::refused(&refusal),
  };
  if let Err(error) = run.write_exit(&exit) {
    supervisor(&format!("cannot write the end of {}: {error}", run_dir.display()));
  }
  drop(lock);
  if let Some(spec) = spec {
    give_back(home, repo, &spec);
  }
}

/// Writes the end of a run whose supervisor cannot find its home: the run then ends at once
pub fn refuse_run(run_dir: &Path, refusal: &Refusal) {
  let _ = RunDir::new(run_dir.to_path_buf()).write_exit(&ExitRecord::refused(refusal));
}

/// Gives back the host slot of the run, or takes the run out of the queue when it got none. A signal does not cut it
fn give_back(home: &Home, repo: Option<Repository>, spec: &RunSpec) {
  let repo = match repo.map_or_else(|| Repository::open(home, home.expand(&spec.repo)), Ok) {
    Ok(repo) => repo,
    Err(refusal) => {
      supervisor(&format!("cannot give the slot of the run back: {refusal}"));
      return;
    }
  };
  match slots::give_back(&repo, &spec.run) {
    Ok(gave_back) => {
      if let Some(n) = gave_back.n {
        supervisor(&format!("gave back the slot {n}, {} process(es) of the run stopped", gave_back.stopped));
        if !gave_back.quarantined.is_empty() {
          supervisor(&format!("the slot {n} stays in quarantine while these work in it: {}", gave_back.quarantined.join(", ")));
        }
      }
      if let Some(deleted) = &gave_back.deleted_ref {
        supervisor(&format!("deleted {deleted}"));
      }
      if gave_back.maintenance_started {
        supervisor("started the git maintenance of the repository");
      }
    }
    Err(refusal) => supervisor(&format!("cannot give the slot of the run back: {refusal}")),
  }
}

/// The flock of the run: the inherited descriptor, which the command must not inherit in turn, or a new one
fn adopt_lock(run: &RunDir, descriptor: Option<RawFd>) -> io::Result<File> {
  let Some(descriptor) = descriptor else {
    let file = open_private(&run.lock_path())?;
    return if try_flock(&file)? {
      Ok(file)
    } else {
      Err(io::Error::other("another supervisor holds the run"))
    };
  };
  sys::adopt_descriptor(descriptor)
}

/// What the cleanup of a run needs from its steps: the fingerprints of the prepare steps of its slot
#[derive(Default)]
struct Held {
  prepared: Option<Prepared>,
}

fn run_to_end(home: &Home, run: &RunDir, repo: &mut Option<Repository>) -> Result<i32, Refusal> {
  let spec = run
    .spec()
    .map_err(|error| Refusal::new(Code::AgentError, format!("cannot read the spec of the run: {error}")))?;
  let log = match EventLog::open(&run.events_path()) {
    Ok(log) => Arc::new(log),
    Err(error) => return Err(Refusal::new(Code::AgentError, format!("cannot open the log of the run: {error}"))),
  };
  let opened = Repository::open(home, home.expand(&spec.repo));
  let outcome = match opened {
    Ok(opened) => {
      let opened = repo.insert(opened);
      let stop = || sys::stop_signalled() || run.cancel_requested();
      let mut held = Held::default();
      // without its own binary the agent cannot ask the login shell, and a run gets the environment of the agent
      let exe = std::env::current_exe().ok();
      let context = Context {
        home,
        exe: exe.as_deref(),
        spec: &spec,
        run,
        repo: opened,
        log: &log,
      };
      let outcome = panic::catch_unwind(AssertUnwindSafe(|| steps(&context, &stop, &mut held)))
        .unwrap_or_else(|payload| Err(Refusal::new(Code::AgentError, format!("the supervisor failed: {}", panic_text(&*payload)))));
      if let Some(prepared) = &held.prepared
        && let Err(error) = prepared.save()
      {
        context.say(&format!("cannot keep the fingerprints of the prepare steps of the slot: {error}"));
      }
      outcome
    }
    Err(refusal) => Err(refusal),
  };
  log.close();
  outcome
}

/// What the steps of a run share
struct Context<'a> {
  home: &'a Home,
  /// The binary of the agent, which the login shell runs for `dump-env`
  exe: Option<&'a Path>,
  spec: &'a RunSpec,
  run: &'a RunDir,
  repo: &'a Repository,
  log: &'a Arc<EventLog>,
}

impl Context<'_> {
  fn say(&self, message: &str) {
    self.log.write(Stream::Stderr, &format!("{SAY}{message}"));
  }
}

/// How the command of a run starts, from the prepare step
struct Launch {
  env: BTreeMap<OsString, OsString>,
  nice: Option<u8>,
  /// The outputs of `[host]`, when the client asked for them
  outputs: Vec<String>,
  /// The tree of the slot before the command, when the client asked for the changes
  before: Option<String>,
  /// The login environment of the run, for its first line
  login: Option<LoginEnv>,
  /// The git of the command, for its first line: `git 2.56.0`
  git: String,
}

fn steps(context: &Context<'_>, stop: &dyn Fn() -> bool, held: &mut Held) -> Result<i32, Refusal> {
  let (spec, run) = (context.spec, context.run);
  let stage = run.stage_path();
  private_dir(&stage).map_err(|error| Refusal::new(Code::AgentError, format!("cannot make {}: {error}", stage.display())))?;
  let mut phases = Phases { run, ended: Vec::new() };

  let Some(lent) = take_slot(context, &mut phases, stop)? else {
    return Ok(INTERRUPTED);
  };
  let slot = PathBuf::from(&lent.path);
  held.prepared = Some(Prepared::load(&context.home.slots_dir(), &lent.lease_id, lent.generation));
  if stop() {
    return Ok(INTERRUPTED);
  }
  let limits = PackLimits {
    deadline: tuning::pack_wait(),
    stall: tuning::pack_stall(),
  };
  let has = |commit: &str| context.repo.has_commit(commit);
  let mut say = |message: &str| context.say(message);
  if !phases.time("pack_wait", || wait_for_pack(run, &context.repo.path, spec, limits, stop, &mut say, &has))? {
    return Ok(INTERRUPTED);
  }
  match phases.time("checkout", || slots::checkout(context.repo, &spec.run, &spec.commit)) {
    Ok(checked_out) => {
      if let SlotCheckout::Forced(why) = checked_out.moved {
        context.say(&format!("checked out with a scan of every file, because git refused the diff: {why}"));
      }
    }
    Err(_) if stop() => return Ok(INTERRUPTED),
    Err(refusal) => {
      return Err(Refusal::new(
        refusal.code,
        format!("cannot check out {} in {}: {}", spec.commit, slot.display(), refusal.message),
      ));
    }
  }
  if stop() {
    return Ok(INTERRUPTED);
  }
  let launch = match phases.time("prepare", || prepare_slot(context, &slot, stop, held)) {
    Ok(Some(launch)) => launch,
    Ok(None) => return Ok(INTERRUPTED),
    Err(_) if stop() => return Ok(INTERRUPTED),
    Err(refusal) => return Err(refusal),
  };
  if stop() {
    return Ok(INTERRUPTED);
  }
  let before: Vec<BTreeSet<OsString>> = spec.fetch.iter().map(|dir| entries(&slot.join(dir))).collect();
  context.say(&start_line(&slot, &launch));
  if let Some(warning) = run_git::login_shell_warning(&spec.command, &launch.git, cfg!(target_os = "macos")) {
    context.say(&warning);
  }
  let code = phases.time("command", || run_group(&spec.command, &slot, &launch, context.log, stop, Stream::Stdout))?;
  for error in phases.time("stage", || stage_run(context, &slot, &launch, &before)) {
    context.say(&error);
  }
  Ok(code)
}

/// Lends a host slot for the run, in the phases `git` and `slot`. A first lend answers at once. A lend that queues
/// because Homebrew upgrades the git of the host says so, and waits in the phase `git` with lends of [`GIT_POLL`] up to
/// [`GIT_WAIT`]. Then one wait of up to an hour for the slot: the queue lines go to the log of the run once a minute,
/// and a wait that ends in the queue refuses with `slot_timeout`. A git below its floor refuses with `old_git`. Answers
/// `None` when a cancel ended the wait; the give-back at the end takes the run out of the queue then
fn take_slot(context: &Context<'_>, phases: &mut Phases<'_>, stop: &dyn Fn() -> bool) -> Result<Option<Lend>, Refusal> {
  let started = Instant::now();
  let first = lend(context, None, stop);
  if !matches!(first, Ok(Some(LendAnswer::Queued(_)))) {
    phases.record("slot", started.elapsed());
  }
  let mut queued = match first? {
    None => return Ok(None),
    Some(LendAnswer::Lent(lent)) => return Ok(Some(said_lent(context, lent))),
    Some(LendAnswer::Queued(queued)) => queued,
  };
  let git_waited = queued.git_upgrading.is_some();
  if let Some(command) = queued.git_upgrading.clone() {
    context.say(&format!("waiting for the upgrade of git on the host: `{command}` runs once no host slot is lent"));
    let upgrading = Instant::now();
    let waited = phases.time("git", || {
      while queued.git_upgrading.is_some() && upgrading.elapsed() < GIT_WAIT {
        match lend(context, Some(GIT_POLL), stop)? {
          None => return Ok(None),
          Some(LendAnswer::Lent(lent)) => return Ok(Some(lent)),
          Some(LendAnswer::Queued(next)) => queued = next,
        }
      }
      Ok::<_, Refusal>(None)
    })?;
    if let Some(lent) = waited {
      phases.record("slot", Duration::ZERO);
      return Ok(Some(said_lent(context, lent)));
    }
    if stop() {
      return Ok(None);
    }
  }
  let slot = Instant::now();
  let waited = lend(context, Some(SLOT_WAIT), stop);
  phases.record("slot", if git_waited { slot.elapsed() } else { started.elapsed() });
  match waited? {
    None => Ok(None),
    Some(LendAnswer::Lent(lent)) => Ok(Some(said_lent(context, lent))),
    Some(LendAnswer::Queued(queued)) => Err(Refusal::new(
      Code::SlotTimeout,
      format!("no host slot for {} s; giving up: {}", SLOT_WAIT.as_secs(), slots::waiting_line(&queued, context.repo)),
    )),
  }
}

/// The lent slot, after its line in the log of the run
fn said_lent(context: &Context<'_>, lent: Lend) -> Lend {
  context.say(&slots::lent_line(&lent, context.repo, epoch_ms(SystemTime::now())));
  lent
}

/// One lend of the run, with a wait of `wait`. Answers `None` when a cancel ended it
fn lend(context: &Context<'_>, wait: Option<Duration>, stop: &dyn Fn() -> bool) -> Result<Option<LendAnswer>, Refusal> {
  match slots::lend(context.repo, &context.spec.run, &context.spec.affinity, wait, stop, &mut |line| context.say(line)) {
    Err(_) if stop() => Ok(None),
    other => other,
  }
}

/// `[host]` of `.branchdeck.toml` of `commit`, else its `.yonder.toml`, else the default. A file that does not parse
/// refuses with `bad_config`
fn read_config(repo: &Repository, commit: &str) -> Result<RepoConfig, Refusal> {
  let blob = |file: &str| {
    repo
      .show(commit, Some(file), &[])
      .map(|shown| shown.blob.map(|blob| blob.text))
      .map_err(|refusal| Refusal::new(Code::BadConfig, format!("cannot read {file} of {commit}: {}", refusal.message)))
  };
  let bad_config = |error: host_link::config::ConfigError| Refusal::new(Code::BadConfig, error.to_string());
  let branchdeck = blob(FILE)?;
  let has_host = branchdeck.as_deref().map(parse_branchdeck).transpose().map_err(bad_config)?.flatten().is_some();
  let yonder = if has_host { None } else { blob(YONDER_FILE)? };
  resolve(branchdeck.as_deref(), yonder.as_deref()).map_err(bad_config)
}

/// The prepare step of a run: the config of the commit, the environment, the prepare steps, and for a client that asks
/// for them the removal of the outputs and the tree before the command. Answers `None` when a cancel ended it
fn prepare_slot(context: &Context<'_>, slot: &Path, stop: &dyn Fn() -> bool, held: &mut Held) -> Result<Option<Launch>, Refusal> {
  let spec = context.spec;
  let config = read_config(context.repo, &spec.commit)?;
  let login = context
    .exe
    .and_then(|exe| login_env::for_run(context.home, exe, login_env::MAX_AGE, &mut |line| context.say(line)));
  let mut base = login_env::applied(login.as_ref());
  let host_git = run_git::for_run()?;
  if let Some(dir) = &host_git.dir {
    let path = base.get(std::ffi::OsStr::new("PATH")).cloned().unwrap_or_default();
    base.insert(OsString::from("PATH"), with_first(&path, dir));
  }
  let env = run_env::run_env(base, &config.env, context.home, spec, slot)?;
  let launch = Launch {
    env,
    nice: spec.nice,
    outputs: if spec.outputs { config.outputs.clone() } else { Vec::new() },
    before: None,
    login,
    git: format!("git {}", host_git.version),
  };
  let Some(prepared) = held.prepared.as_mut() else {
    return Err(Refusal::new(Code::AgentError, "the prepare step has no slot"));
  };
  for step in &config.prepare {
    if stop() {
      return Ok(None);
    }
    let ids = if step.inputs.is_empty() {
      Vec::new()
    } else {
      context
        .repo
        .show(&spec.commit, None, &step.inputs)
        .map_err(|refusal| Refusal::new(Code::AgentError, format!("cannot read the inputs of the prepare step {:?}: {}", step.name, refusal.message)))?
        .ids
    };
    let fingerprint = prepare::fingerprint(step, &ids);
    let Some(why) = prepare::reason(step, &fingerprint, &prepared.steps, slot) else {
      continue;
    };
    context.say(&format!("prepare: {} ({why})", step.name));
    let code = run_group(&step.run, slot, &launch, context.log, stop, Stream::Stderr)?;
    if stop() {
      return Ok(None);
    }
    if code != 0 {
      return Err(Refusal::new(
        Code::PrepareFailed,
        format!("the prepare step {:?} exited with {code}; its output is above", step.name),
      ));
    }
    prepared.steps.insert(prepare::key(step), fingerprint);
  }
  for output in &launch.outputs {
    if let Err(error) = remove_entry(&slot.join(output)) {
      context.say(&format!("cannot remove the output {output} of the last run: {error}"));
    }
  }
  let before = if spec.changes {
    let before = context
      .repo
      .snapshot(&spec.run)
      .map_err(|refusal| Refusal::new(Code::AgentError, format!("cannot scan the files of {}: {}", slot.display(), refusal.message)))?;
    Some(before)
  } else {
    None
  };
  Ok(Some(Launch { before, ..launch }))
}

/// The first line of the command: where and how it runs, and the load of the host
fn start_line(slot: &Path, launch: &Launch) -> String {
  let load = run_env::load().map_or_else(|| "?".to_owned(), |load| format!("{load:.1}"));
  let cores = thread::available_parallelism().map_or(0, std::num::NonZero::get);
  let files = run_env::raise_open_files().map_or_else(|error| format!("? ({error})"), |limit| limit.to_string());
  let login = launch.login.as_ref().map_or_else(
    || "the environment of the agent".to_owned(),
    |login| format!("the login environment of {} from {} ago", login.shell, login.age_text(SystemTime::now())),
  );
  format!(
    "running in {} (slot {}): load {load} on {cores} cores, nice {}, {files} open files, {}, {login}",
    slot.display(),
    run_env::slot_number(slot),
    launch.nice.unwrap_or(0),
    launch.git
  )
}

/// The stage step: the changes of the command, the outputs, and the new entries of the fetch directories. Answers a
/// message for each failure
fn stage_run(context: &Context<'_>, slot: &Path, launch: &Launch, before: &[BTreeSet<OsString>]) -> Vec<String> {
  let run = context.run;
  let mut errors = Vec::new();
  if let Some(before) = &launch.before
    && let Err(refusal) = context.repo.changes(&context.spec.run, before, &run.changes_path())
  {
    errors.push(format!("cannot stage the changes of the command: {}", refusal.message));
  }
  if !launch.outputs.is_empty() {
    errors.extend(stage_outputs(slot, &launch.outputs, run));
  }
  errors.extend(stage_new_entries(slot, &context.spec.fetch, before, &run.stage_path()));
  errors
}

/// Moves each output that the command made to the outputs of the run, and records the moved ones. Answers a message for
/// each failure
fn stage_outputs(slot: &Path, outputs: &[String], run: &RunDir) -> Vec<String> {
  let mut errors = Vec::new();
  let mut staged = Vec::new();
  for output in outputs {
    let source = slot.join(output);
    if fs::symlink_metadata(&source).is_err() {
      continue;
    }
    let target = run.outputs_path().join(output);
    let moved = target.parent().map_or(Ok(()), private_dir).and_then(|()| move_entry(&source, &target));
    match moved {
      Ok(()) => staged.push(output.clone()),
      Err(error) => errors.push(format!("cannot stage the output {output}: {error}")),
    }
  }
  if let Err(error) = run.write_output_roots(&staged) {
    errors.push(format!("cannot record the outputs of the run: {error}"));
  }
  errors
}

/// The steps of a run that have ended, each with its duration
struct Phases<'a> {
  run: &'a RunDir,
  ended: Vec<Phase>,
}

impl Phases<'_> {
  /// Runs `step`, adds its duration as the phase `name` whatever its outcome, and rewrites `phases.json`
  fn time<T>(&mut self, name: &str, step: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let outcome = step();
    self.record(name, started.elapsed());
    outcome
  }

  /// Adds the phase `name` of `took`, and rewrites `phases.json`
  fn record(&mut self, name: &str, took: Duration) {
    self.ended.push(Phase {
      name: name.to_owned(),
      ms: u64::try_from(took.as_millis()).unwrap_or(u64::MAX),
    });
    if let Err(error) = self.run.write_phases(&self.ended) {
      supervisor(&format!("cannot write the phases of {}: {error}", self.run.path().display()));
    }
  }
}

/// The limits of the wait for a pack
#[derive(Clone, Copy, Debug)]
pub(crate) struct PackLimits {
  /// The whole wait
  pub(crate) deadline: Duration,
  /// The wait for the first chunk, and from each chunk to the next
  pub(crate) stall: Duration,
}

/// Waits for the pack of the run. A commit that the repository has already, as `has` answers, needs no pack, because
/// the client sends none then. Else the wait ends when the pack wrote `packed` in the run directory, after the ref of
/// the run: so the delete of the ref at the end of the run comes after it. The wait fails at the deadline of `limits`, or
/// after its stall time without a chunk, which a client that is gone sends no more. Answers `false` when `stop` ended
/// the wait. A wait longer than [`PACK_WAIT_SAY`] says so once
pub(crate) fn wait_for_pack(
  run: &RunDir,
  repo: &Path,
  spec: &RunSpec,
  limits: PackLimits,
  stop: &dyn Fn() -> bool,
  say: &mut dyn FnMut(&str),
  has: &dyn Fn(&str) -> Result<bool, Refusal>,
) -> Result<bool, Refusal> {
  let look = |commit: &str| has(commit).map_err(|refusal| Refusal::new(Code::AgentError, format!("cannot look for {commit} in {}: {}", repo.display(), refusal.message)));
  if look(&spec.commit)? {
    return Ok(true);
  }
  // the watch comes before the first look, so no change between a look and a wait is lost
  let mut watch = crate::watch::Watch::new(&[run.path()]);
  let started = Instant::now();
  let started_at = SystemTime::now();
  let mut said = false;
  loop {
    if run.packed() {
      return Ok(true);
    }
    if stop() {
      return Ok(false);
    }
    let quiet = run.pack_progress().map_or(started_at, |noted| noted.max(started_at)).elapsed().unwrap_or_default();
    let failure = if started.elapsed() >= limits.deadline {
      Some(format!("did not come in {} s", limits.deadline.as_secs_f64()))
    } else if quiet >= limits.stall {
      Some(format!("sent no chunk for {} s, so its client is gone", limits.stall.as_secs_f64()))
    } else {
      None
    };
    if let Some(failure) = failure {
      // a pack whose mark could not be written leaves its commit and no mark
      if look(&spec.commit)? {
        return Ok(true);
      }
      return Err(Refusal::new(
        Code::PackMissing,
        format!("the pack of the run {} {failure}, and {} is missing in {}", spec.run, spec.commit, repo.display()),
      ));
    }
    if !said && started.elapsed() > PACK_WAIT_SAY {
      say(&format!("waiting for the pack of {}", spec.commit));
      said = true;
    }
    watch.wait(PACK_RECHECK.min(limits.stall).min(limits.deadline));
  }
}

/// Runs `argv` in the slot root, in its own process group, with the environment and the niceness of `launch`, and
/// answers its exit code. Its stdout goes to `events.log` as events of `stdout`, and its stderr as stderr events. The
/// other processes of the group end before the answer
fn run_group(argv: &[String], slot: &Path, launch: &Launch, log: &Arc<EventLog>, stop: &dyn Fn() -> bool, stdout: Stream) -> Result<i32, Refusal> {
  let Some((program, arguments)) = argv.split_first() else {
    return Err(Refusal::new(Code::SpawnFailed, "the run has no command"));
  };
  let mut command = windowless_command(program);
  command
    .args(arguments)
    .current_dir(slot)
    .env_clear()
    .envs(&launch.env)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .process_group(0);
  if let Some(nice) = launch.nice {
    sys::set_niceness(&mut command, nice);
  }
  let mut child = command
    .spawn()
    .map_err(|error| Refusal::new(Code::SpawnFailed, format!("cannot start `{program}` in {}: {error}", slot.display())))?;
  let group = child.id();
  let (done, readers) = mpsc::channel();
  for (stream, pipe) in [
    (stdout, child.stdout.take().map(|pipe| Box::new(pipe) as Box<dyn Read + Send>)),
    (Stream::Stderr, child.stderr.take().map(|pipe| Box::new(pipe) as Box<dyn Read + Send>)),
  ] {
    let (log, done) = (Arc::clone(log), done.clone());
    if let Some(pipe) = pipe {
      thread::spawn(in_current_span(move || {
        read_lines(pipe, &mut |line| log.write(stream, line));
        let _ = done.send(());
      }));
    }
  }
  drop(done);

  let grace = tuning::grace();
  let mut interrupted: Option<Instant> = None;
  let status = loop {
    match child.try_wait() {
      Ok(Some(status)) => break Some(status),
      Ok(None) => {}
      Err(error) => {
        // the slot goes back with the give-back, so the command must not outlive it
        sys::signal_group(group, libc::SIGKILL);
        let _ = child.kill();
        let _ = wait_up_to(&mut child, KILL_WAIT);
        return Err(Refusal::new(Code::AgentError, format!("cannot wait for `{program}`: {error}")));
      }
    }
    if interrupted.is_none() && stop() {
      sys::signal_group(group, libc::SIGINT);
      interrupted = Some(Instant::now());
    }
    if interrupted.is_some_and(|at| at.elapsed() >= grace) {
      log.write(Stream::Stderr, &format!("{SAY}the command did not end {} ms after SIGINT; SIGKILL", grace.as_millis()));
      sys::signal_group(group, libc::SIGKILL);
      // a command that left its group still gets the signal through its pid, which std has not reaped yet
      let _ = child.kill();
      break wait_up_to(&mut child, KILL_WAIT);
    }
    thread::sleep(POLL);
  };
  end_group(group, log);
  let drained = Instant::now() + DRAIN;
  for _ in 0..2 {
    if readers.recv_timeout(drained.saturating_duration_since(Instant::now())).is_err() {
      break;
    }
  }
  Ok(status.map_or(128 + libc::SIGKILL, exit_code))
}

/// The status of `child` within `deadline`, or `None`
fn wait_up_to(child: &mut Child, deadline: Duration) -> Option<ExitStatus> {
  let started = Instant::now();
  while started.elapsed() < deadline {
    if let Ok(Some(status)) = child.try_wait() {
      return Some(status);
    }
    thread::sleep(POLL);
  }
  None
}

/// Ends the processes that the command left in its group: SIGTERM, then SIGKILL after [`GROUP_TERM_WAIT`]. A group that
/// empties within [`GROUP_SETTLE`] counts as empty: the system can need a moment to remove an ended process
fn end_group(group: u32, log: &EventLog) {
  if settle(group) {
    return;
  }
  log.write(Stream::Stderr, &format!("{SAY}the command left processes in its group; SIGTERM"));
  sys::signal_group(group, libc::SIGTERM);
  let started = Instant::now();
  while sys::group_alive(group) {
    if started.elapsed() >= GROUP_TERM_WAIT {
      log.write(Stream::Stderr, &format!("{SAY}the processes of the group did not end after SIGTERM; SIGKILL"));
      sys::signal_group(group, libc::SIGKILL);
      settle(group);
      return;
    }
    thread::sleep(POLL);
  }
}

/// Waits up to [`GROUP_SETTLE`] for the group to empty. Answers whether it is empty
fn settle(group: u32) -> bool {
  let started = Instant::now();
  while sys::group_alive(group) {
    if started.elapsed() >= GROUP_SETTLE {
      return false;
    }
    thread::sleep(Duration::from_millis(10));
  }
  true
}

/// The exit code of a command: its code, or 128 plus the number of the signal that ended it
pub(crate) fn exit_code(status: ExitStatus) -> i32 {
  status.code().unwrap_or_else(|| 128 + status.signal().unwrap_or(0))
}

/// Calls `line` with each line of `reader`, without its line end. A line longer than [`MAX_EVENT_TEXT`] comes in pieces.
/// A last line without its end comes too
pub(crate) fn read_lines(reader: impl Read, line: &mut dyn FnMut(&str)) {
  let mut reader = BufReader::new(reader);
  let mut pending: Vec<u8> = Vec::new();
  let emit = |bytes: &mut Vec<u8>, line: &mut dyn FnMut(&str)| {
    if bytes.last() == Some(&b'\r') {
      bytes.pop();
    }
    line(&String::from_utf8_lossy(bytes));
    bytes.clear();
  };
  loop {
    let available = match reader.fill_buf() {
      Ok([]) => break,
      Ok(available) => available,
      Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
      Err(_) => break,
    };
    // a full piece goes out before more bytes come, and its line end goes with it
    if pending.len() >= MAX_EVENT_TEXT {
      if available[0] == b'\n' {
        reader.consume(1);
      }
      emit(&mut pending, line);
      continue;
    }
    let room = MAX_EVENT_TEXT - pending.len();
    if let Some(newline) = available.iter().take(room + 1).position(|&byte| byte == b'\n') {
      pending.extend_from_slice(&available[..newline]);
      reader.consume(newline + 1);
      emit(&mut pending, line);
    } else {
      let taken = available.len().min(room);
      pending.extend_from_slice(&available[..taken]);
      reader.consume(taken);
    }
  }
  if !pending.is_empty() {
    emit(&mut pending, line);
  }
}

/// The names in `dir`, or none when it is missing
fn entries(dir: &Path) -> BTreeSet<OsString> {
  fs::read_dir(dir).map_or_else(|_| BTreeSet::new(), |entries| entries.filter_map(Result::ok).map(|entry| entry.file_name()).collect())
}

/// Moves each entry of each `fetch` directory of the slot that `before` does not name to the same path under `stage`.
/// Answers a message for each entry that cannot move
pub(crate) fn stage_new_entries(slot: &Path, fetch: &[String], before: &[BTreeSet<OsString>], stage: &Path) -> Vec<String> {
  let mut errors = Vec::new();
  for (dir, before) in fetch.iter().zip(before) {
    let source = slot.join(dir);
    let target = stage.join(dir);
    for name in entries(&source).difference(before) {
      let moved = fs::create_dir_all(&target).and_then(|()| move_entry(&source.join(name), &target.join(name)));
      if let Err(error) = moved {
        errors.push(format!("cannot stage {}: {error}", source.join(name).display()));
      }
    }
  }
  errors
}

/// Moves a file or a directory. Across two file systems it copies, then removes the source
fn move_entry(source: &Path, target: &Path) -> io::Result<()> {
  match fs_retry::rename(source, target) {
    Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {
      copy_tree(source, target)?;
      remove_entry(source)
    }
    other => other,
  }
}

/// Copies a tree into the stage: each directory with the mode 0700, each file with 0600, or 0700 when it is executable
fn copy_tree(source: &Path, target: &Path) -> io::Result<()> {
  let metadata = fs::symlink_metadata(source)?;
  if metadata.is_dir() {
    private_dir(target)?;
    for entry in fs::read_dir(source)? {
      let entry = entry?;
      copy_tree(&entry.path(), &target.join(entry.file_name()))?;
    }
    Ok(())
  } else if metadata.file_type().is_symlink() {
    std::os::unix::fs::symlink(fs::read_link(source)?, target)
  } else {
    fs::copy(source, target)?;
    let executable = metadata.permissions().mode() & 0o111 != 0;
    fs::set_permissions(target, Permissions::from_mode(if executable { 0o700 } else { FILE_MODE }))
  }
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
  payload
    .downcast_ref::<&str>()
    .map(|text| (*text).to_owned())
    .or_else(|| payload.downcast_ref::<String>().cloned())
    .unwrap_or_else(|| "a panic".to_owned())
}

#[cfg(test)]
#[path = "supervise_test.rs"]
mod supervise_test;

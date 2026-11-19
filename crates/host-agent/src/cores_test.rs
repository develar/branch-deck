use super::*;
use pretty_assertions::assert_eq;
use std::io::Write as _;
use std::os::unix::process::CommandExt;
use sync_utils::process_scan::{cpu_times, process_list, process_started};

/// A `yes` in a process group of its own, killed and reaped when the test ends, a failed one too, so no test leaves it
struct Spinner(std::process::Child);

impl Spinner {
  fn start() -> Self {
    Self(std::process::Command::new("yes").stdout(std::process::Stdio::null()).process_group(0).spawn().unwrap())
  }

  fn group(&self) -> u32 {
    self.0.id()
  }

  /// Until it used `cpu` of CPU time: the state a sample needs, not a time
  fn spin(&self, cpu: Duration) {
    while cpu_time(self.group()).is_none_or(|used| used < cpu) {
      std::thread::yield_now();
    }
  }

  fn end(mut self) {
    self.0.kill().unwrap();
    self.0.wait().unwrap();
  }
}

impl Drop for Spinner {
  fn drop(&mut self) {
    // after `end` both fail, as the process is reaped; a failed test reaps it here
    let _ = self.0.kill();
    let _ = self.0.wait();
  }
}

/// A sample counts the CPU time of the processes of the command group since the sample before, and the end the larger of
/// it and the CPU time of the reaped children: a command that computes shows its cores live and in its measure. The
/// assertions are on the CPU time that counts, never on the live cores, a rate over the wall time of a loaded host
#[test]
fn the_samples_count_the_cpu_of_the_command_group_and_the_end_its_measure() {
  let slot = tempfile::tempdir().unwrap();
  let mut sampler = Sampler::start(slot.path(), Duration::ZERO);
  assert!(sampler.is_due());
  let spinner = Spinner::start();
  let group = spinner.group();
  spinner.spin(Duration::from_millis(300));
  let live = sampler.sample(group);
  let file = sampler.file();
  assert!(
    file.cpu_ms >= 300,
    "a process that started after the command counts its whole CPU time: {file:?}, started {:?}, the command at {:?}",
    process_started(group),
    sampler.started_clock,
  );
  assert_eq!(file.samples.iter().map(|sample| sample.cores).collect::<Vec<_>>(), [live]);
  spinner.end();
  let file = sampler.finish(group).clone();
  assert!(file.ended);
  assert!(file.cpu_ms >= 300 && file.peak >= file.mean, "{file:?}");
  assert_eq!(file.samples.len(), 2, "one last sample at the end");
  assert_eq!(file.run_cores(), RunCores { mean: file.mean, peak: file.peak });
}

/// A process of the group that started before the command, as a Bazel server of the slot that the sampler finds only at
/// a later sample, counts from that sample on, not its whole life
#[test]
fn a_process_older_than_the_command_counts_only_from_its_first_sample() {
  let slot = tempfile::tempdir().unwrap();
  let old = Spinner::start();
  let group = old.group();
  // its life before the command: 300 ms of CPU
  old.spin(Duration::from_millis(300));
  let mut sampler = Sampler::start(slot.path(), Duration::ZERO);
  // a machine of cores enough that the cap of a sample never hides its life
  sampler.machine_cores = 1000;
  let _first = sampler.sample(group);
  assert!(sampler.file().cpu_ms < 300, "its life before the command counts: {:?}", sampler.file());
  old.end();
}

/// A sample counts at most the cores of the machine over its time: a process that the sampler takes as new to the
/// command with a second of CPU, sampled a few ms after the start on a machine of one core, counts those few ms
#[test]
fn a_sample_counts_at_most_the_cores_of_the_machine() {
  let slot = tempfile::tempdir().unwrap();
  let spinner = Spinner::start();
  let group = spinner.group();
  spinner.spin(Duration::from_secs(1));
  let mut sampler = Sampler::start(slot.path(), Duration::ZERO);
  // a command that started before the spinner, so its whole second counts, on one core
  sampler.started_clock = sampler.started_clock.map(|clock| StartClock { now: 0, ..clock });
  sampler.machine_cores = 1;
  let _first = sampler.sample(group);
  let file = sampler.file();
  let interval_ms = file.samples[0].ms;
  assert!(interval_ms < 1000, "the sample came after the second of CPU it would count: {file:?}");
  assert!(file.cpu_ms <= interval_ms, "one core over {interval_ms} ms: {file:?}");
  spinner.end();
}

/// A shell in a group of its own that runs `child` and then waits for a line of its stdin, killed and reaped at the end
struct Shell(std::process::Child);

impl Shell {
  fn start(script: &str) -> Self {
    Self(
      std::process::Command::new("sh")
        .args(["-c", script])
        .stdin(std::process::Stdio::piped())
        .process_group(0)
        .spawn()
        .unwrap(),
    )
  }

  fn pid(&self) -> u32 {
    self.0.id()
  }

  fn line(&mut self) {
    self.0.stdin.as_mut().unwrap().write_all(b"go\n").unwrap();
  }

  /// Until the shell reaped a child that used CPU: the state a sample needs, not a time
  fn reaped(&self) -> sync_utils::process_scan::CpuTimes {
    loop {
      let times = cpu_times(self.pid()).unwrap();
      if !times.children.is_zero() {
        return times;
      }
      std::thread::yield_now();
    }
  }
}

impl Drop for Shell {
  fn drop(&mut self) {
    let _ = self.0.kill();
    let _ = self.0.wait();
  }
}

/// A loop of a shell that uses some 100 ms of CPU
const LOOP: &str = "i=0; while [ $i -lt 100000 ]; do i=$((i+1)); done";

/// A child that started and ended between two samples counts in the process that reaped it: a compiler of a build, a
/// test of a suite
#[test]
fn a_child_that_ends_between_two_samples_counts_in_the_process_that_reaped_it() {
  let slot = tempfile::tempdir().unwrap();
  let mut sampler = Sampler::start(slot.path(), Duration::ZERO);
  let shell = Shell::start(&format!("sh -c '{LOOP}'; read line"));
  shell.reaped();
  let _first = sampler.sample(shell.pid());
  let total = cpu_times(shell.pid()).unwrap().total();
  let counted = sampler.file().cpu_ms;
  assert!(counted.abs_diff(millis(total)) <= 1, "{counted} ms of {total:?}: {:?}", sampler.file());
}

/// A child that a sample saw alive counts once: its parent counts it again once it reaps it, and its time at that
/// sample is a credit that the growth of the parent pays off
#[test]
fn a_child_seen_alive_counts_once_after_its_parent_reaped_it() {
  let slot = tempfile::tempdir().unwrap();
  let mut sampler = Sampler::start(slot.path(), Duration::ZERO);
  let mut shell = Shell::start(&format!("sh -c '{LOOP}; read x'; read y"));
  // the state the first sample needs: the child computed its loop and waits for its line
  let child = loop {
    let child = process_list().unwrap().into_iter().find(|process| process.parent == shell.pid());
    if let Some(child) = child
      && cpu_times(child.pid).is_some_and(|times| times.own >= Duration::from_millis(20))
    {
      break child.pid;
    }
    std::thread::yield_now();
  };
  let _alive = sampler.sample(shell.pid());
  let seen_alive = sampler.file().cpu_ms;
  assert!(seen_alive >= 20, "the child at the first sample: {:?}", sampler.file());
  shell.line();
  shell.reaped();
  let _reaped = sampler.sample(shell.pid());
  let total = cpu_times(shell.pid()).unwrap().total();
  let counted = sampler.file().cpu_ms;
  assert!(
    counted.abs_diff(millis(total)) <= 1,
    "{counted} ms of {total:?}, the child {child} counted twice?: {:?}",
    sampler.file()
  );
}

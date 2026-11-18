use super::*;
use pretty_assertions::assert_eq;
use std::os::unix::process::CommandExt;
use sync_utils::process_scan::process_started;

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
/// a later sample, counts from that sample on, not its whole life; a sample counts at most the cores of the machine
#[test]
fn a_process_older_than_the_command_counts_only_from_its_first_sample() {
  let slot = tempfile::tempdir().unwrap();
  let old = Spinner::start();
  let group = old.group();
  // its life before the command: 300 ms of CPU
  old.spin(Duration::from_millis(300));
  let mut sampler = Sampler::start(slot.path(), Duration::ZERO);
  let first = sampler.sample(group);
  assert!(sampler.file().cpu_ms < 300, "its life before the command counts: {:?}", sampler.file());
  let machine = Cores::whole(u32::try_from(std::thread::available_parallelism().unwrap().get()).unwrap());
  assert!(first <= machine, "{first}");
  old.end();
}

use super::*;
use pretty_assertions::assert_eq;
use std::os::unix::process::CommandExt;

/// A sample counts the CPU time of the processes of the command group since the sample before, and the end the larger of
/// it and the CPU time of the reaped children: a command that computes shows its cores live and in its measure
#[test]
fn the_samples_count_the_cpu_of_the_command_group_and_the_end_its_measure() {
  let slot = tempfile::tempdir().unwrap();
  let mut sampler = Sampler::start(slot.path(), Duration::ZERO);
  assert!(sampler.is_due());
  let mut child = std::process::Command::new("yes").stdout(std::process::Stdio::null()).process_group(0).spawn().unwrap();
  let group = child.id();
  // the state the sample needs: 300 ms of CPU of the command, not a time
  while cpu_time(group).is_none_or(|cpu| cpu < Duration::from_millis(300)) {
    std::thread::yield_now();
  }
  let live = sampler.sample(group);
  assert!(live > Cores(0), "{live}");
  assert!(sampler.file().cpu_ms >= 300, "{:?}", sampler.file());
  assert_eq!(sampler.file().samples.len(), 1);
  child.kill().unwrap();
  child.wait().unwrap();
  let file = sampler.finish(group).clone();
  assert!(file.ended);
  assert!(file.cpu_ms >= 300 && file.mean > Cores(0) && file.peak >= file.mean, "{file:?}");
  assert_eq!(file.samples.len(), 2, "one last sample at the end");
  assert_eq!(file.run_cores(), RunCores { mean: file.mean, peak: file.peak });
}

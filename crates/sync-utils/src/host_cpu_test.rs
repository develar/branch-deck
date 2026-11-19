use super::*;
use pretty_assertions::assert_eq;

/// The busy cores between two readings are the busy part of the ticks between them, of all the cores; no tick between
/// them, or a counter that went back, tells none
#[test]
fn the_busy_cores_are_the_busy_part_of_the_ticks_between_two_readings() {
  let at = |busy, total| HostTicks { busy, total };
  assert_eq!(busy_cores(at(100, 1000), at(550, 2000), 20), Some(Cores(90)));
  assert_eq!(busy_cores(at(0, 0), at(1000, 1000), 20), Some(Cores::whole(20)));
  assert_eq!(busy_cores(at(5, 10), at(5, 10), 20), None);
  assert_eq!(busy_cores(at(5, 10), at(4, 20), 20), None);
}

/// The line `cpu` of `/proc/stat` counts user, nice, system, irq, softirq and steal as busy, idle and iowait as not
#[test]
fn the_ticks_of_proc_stat_count_the_work_as_busy_and_idle_and_iowait_as_not() {
  let stat = "cpu  10 1 4 80 5 2 3 1 0 0\ncpu0 5 0 2 40 2 1 1 0 0 0\nintr 1\n";
  assert_eq!(proc_stat_ticks(stat), Some(HostTicks { busy: 21, total: 106 }));
  assert_eq!(proc_stat_ticks("intr 1\n"), None);
}

/// The ticks of this host grow, and two readings tell its busy cores
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn the_ticks_of_this_host_grow() {
  let before = host_ticks().unwrap();
  let mut spin = 0u64;
  let after = loop {
    for _ in 0..100_000 {
      spin = std::hint::black_box(spin.wrapping_add(1));
    }
    let after = host_ticks().unwrap();
    if after.total > before.total && after.busy > before.busy {
      break after;
    }
  };
  let cores = u32::try_from(std::thread::available_parallelism().unwrap().get()).unwrap();
  let busy = busy_cores(before, after, cores).unwrap();
  assert!(busy <= Cores::whole(cores), "{busy}");
}

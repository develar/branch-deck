//! The busy CPU of the whole host, whatever its processes are: the heavy turn of brd-host admits a heavy run by it
//! (openspec/specs/cli/host/spec.md, "serve: Heavy runs share the cores of the host by their measure"), so the load of
//! the runs that are not heavy and of processes no run counts holds a heavy run back too. The counters of the system
//! grow over the life of the machine; two readings give the busy cores between them ([`busy_cores`])

use crate::cores::Cores;

/// The CPU ticks of every core of the host so far: those spent on work (user, nice, system and interrupts) and all of
/// them
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostTicks {
  pub busy: u64,
  pub total: u64,
}

/// The CPU ticks of the host now: `host_statistics` (`HOST_CPU_LOAD_INFO`) on macOS, the line `cpu` of `/proc/stat` on
/// Linux; `None` elsewhere, and where the system won't tell them
pub fn host_ticks() -> Option<HostTicks> {
  platform::host_ticks()
}

/// The busy cores of a host of `cores` between the readings `before` and `after`: the busy part of the ticks between
/// them, of all the cores; `None` without a tick between them, or when a counter went back
pub fn busy_cores(before: HostTicks, after: HostTicks, cores: u32) -> Option<Cores> {
  let busy = after.busy.checked_sub(before.busy)?;
  let total = after.total.checked_sub(before.total)?;
  if total == 0 || busy > total {
    return None;
  }
  let tenths = (u128::from(busy) * u128::from(cores) * 10 + u128::from(total) / 2) / u128::from(total);
  u32::try_from(tenths).ok().map(Cores)
}

/// The ticks of the line `cpu  user nice system idle iowait irq softirq steal …` of `/proc/stat`
#[cfg_attr(
  not(any(target_os = "linux", test)),
  expect(dead_code, reason = "only Linux reads /proc/stat; the tests read the text everywhere")
)]
fn proc_stat_ticks(stat: &str) -> Option<HostTicks> {
  let line = stat.lines().find(|line| line.split_whitespace().next() == Some("cpu"))?;
  let fields: Vec<u64> = line.split_whitespace().skip(1).map(|field| field.parse().ok()).collect::<Option<_>>()?;
  let field = |index: usize| fields.get(index).copied().unwrap_or(0);
  let busy = field(0) + field(1) + field(2) + field(5) + field(6) + field(7);
  Some(HostTicks {
    busy,
    total: busy + field(3) + field(4),
  })
}

#[cfg(target_os = "macos")]
#[expect(unsafe_code, reason = "host_statistics(3) of Mach, which std does not offer")]
#[expect(deprecated, reason = "libc points to the mach2 crate for mach_host_self, which this crate does not depend on")]
mod platform {
  use super::HostTicks;
  use std::sync::OnceLock;

  /// The host port, asked once: each `mach_host_self` adds a reference to it that this process never gives back
  fn host() -> libc::mach_port_t {
    static HOST: OnceLock<libc::mach_port_t> = OnceLock::new();
    // SAFETY: the call takes nothing and answers the host port of this task
    *HOST.get_or_init(|| unsafe { libc::mach_host_self() })
  }

  pub(super) fn host_ticks() -> Option<HostTicks> {
    let mut info = std::mem::MaybeUninit::<libc::host_cpu_load_info>::zeroed();
    let mut count = libc::HOST_CPU_LOAD_INFO_COUNT;
    // SAFETY: the buffer is one `host_cpu_load_info` of `count` integers, which the call writes at most
    let status = unsafe { libc::host_statistics(host(), libc::HOST_CPU_LOAD_INFO, info.as_mut_ptr().cast(), &raw mut count) };
    if status != libc::KERN_SUCCESS {
      return None;
    }
    // SAFETY: the call succeeded and filled it; every bit pattern is valid for the plain C struct
    let ticks = unsafe { info.assume_init() }.cpu_ticks.map(u64::from);
    let state = |index: libc::c_int| usize::try_from(index).ok().and_then(|index| ticks.get(index).copied()).unwrap_or(0);
    let busy = state(libc::CPU_STATE_USER) + state(libc::CPU_STATE_NICE) + state(libc::CPU_STATE_SYSTEM);
    Some(HostTicks {
      busy,
      total: busy + state(libc::CPU_STATE_IDLE),
    })
  }
}

#[cfg(target_os = "linux")]
mod platform {
  use super::HostTicks;

  pub(super) fn host_ticks() -> Option<HostTicks> {
    super::proc_stat_ticks(&std::fs::read_to_string("/proc/stat").ok()?)
  }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod platform {
  use super::HostTicks;

  pub(super) const fn host_ticks() -> Option<HostTicks> {
    None
  }
}

#[cfg(test)]
#[path = "host_cpu_test.rs"]
mod host_cpu_test;

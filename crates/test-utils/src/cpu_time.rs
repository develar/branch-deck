//! The CPU time of the calling thread: what a budget test measures, as the other threads and processes of a loaded
//! machine add to the wall time of a run with every preemption, never to the CPU time of its thread

use std::time::Duration;

/// The CPU time, user and system, that the calling thread has used: `CLOCK_THREAD_CPUTIME_ID` on Unix (nanoseconds),
/// `GetThreadTimes` on Windows, which counts whole clock ticks of about 15.6 ms
///
/// # Panics
///
/// When the system cannot read the clock of this thread
#[must_use]
#[cfg(unix)]
#[expect(unsafe_code, reason = "clock_gettime(2) of the CPU clock of this thread, which std does not offer")]
pub fn thread_cpu_time() -> Duration {
  let mut time = libc::timespec { tv_sec: 0, tv_nsec: 0 };
  // SAFETY: clock_gettime writes one timespec through a pointer to a local of that type
  let done = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &raw mut time) };
  assert_eq!(done, 0, "the CPU clock of this thread: {}", std::io::Error::last_os_error());
  Duration::new(u64::try_from(time.tv_sec).unwrap_or_default(), u32::try_from(time.tv_nsec).unwrap_or_default())
}

/// The CPU time, user and system, that the calling thread has used: `CLOCK_THREAD_CPUTIME_ID` on Unix (nanoseconds),
/// `GetThreadTimes` on Windows, which counts whole clock ticks of about 15.6 ms
///
/// # Panics
///
/// When the system cannot read the clock of this thread
#[must_use]
#[cfg(windows)]
#[expect(unsafe_code, reason = "GetThreadTimes of this thread, which std does not offer")]
pub fn thread_cpu_time() -> Duration {
  use windows_sys::Win32::Foundation::FILETIME;
  use windows_sys::Win32::System::Threading::{GetCurrentThread, GetThreadTimes};
  let zero = FILETIME {
    dwLowDateTime: 0,
    dwHighDateTime: 0,
  };
  let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
  // SAFETY: GetCurrentThread answers a pseudo handle that needs no close, and GetThreadTimes writes four FILETIMEs
  // through pointers to locals of that type
  let done = unsafe { GetThreadTimes(GetCurrentThread(), &raw mut created, &raw mut exited, &raw mut kernel, &raw mut user) };
  assert_ne!(done, 0, "the times of this thread: {}", std::io::Error::last_os_error());
  // a FILETIME counts 100 ns
  let hundreds = |time: FILETIME| (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
  Duration::from_nanos((hundreds(kernel) + hundreds(user)).saturating_mul(100))
}

#[cfg(test)]
#[path = "cpu_time_test.rs"]
mod cpu_time_test;

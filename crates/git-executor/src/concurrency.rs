/// Upper bound for git processes one operation runs at the same time (per-branch sync, integration detection).
/// Git work is mostly process startup and disk I/O, so more processes than cores only add contention.
pub fn max_parallel_git_processes() -> usize {
  std::thread::available_parallelism().map_or(4, std::num::NonZero::get).min(8)
}

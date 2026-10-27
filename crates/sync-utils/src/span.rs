//! The span of the caller for work on another thread. A thread starts without a span: `tokio::task::spawn_blocking`,
//! `JoinSet::spawn_blocking`, a scoped thread and `std::thread::spawn` run their closure outside the span the caller is in,
//! so the lines the closure logs name no operation (`operation{operation_id=…}` of a `bd` run or a Tauri command,
//! docs/design/async-patterns.md#operation-spans). Every closure the cores hand to another thread goes through
//! [`in_current_span`]; the scan `thread_span_scan_test.rs` of `git-executor` checks it. The closure takes the tally of
//! the operation along too (`operation_tally`), so the git processes it starts count into the operation.

use crate::operation_tally;
use tracing::Span;

/// `work` run inside the span the caller is in now, and counting into the tally of its operation, for a closure handed
/// to another thread
pub fn in_current_span<T>(work: impl FnOnce() -> T) -> impl FnOnce() -> T {
  let span = Span::current();
  let tally = operation_tally::current();
  move || operation_tally::within(tally.as_ref(), || span.in_scope(work))
}

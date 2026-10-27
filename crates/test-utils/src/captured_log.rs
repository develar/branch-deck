//! The log lines of a test: the one global subscriber of its process writes them into memory, at DEBUG and without
//! colors. nextest runs a process per test, so the subscriber sees the lines of every thread of the test.

use std::io::Write;
use std::sync::{Arc, Mutex, PoisonError};
use tracing_subscriber::fmt::format::FmtSpan;

/// The lines the subscriber of this process wrote so far
#[derive(Clone, Default)]
pub struct CapturedLog(Arc<Mutex<Vec<u8>>>);

impl CapturedLog {
  /// Installs the subscriber of this process; `span_closes`: a line as each span closes too (the span of a git process
  /// as it ends)
  ///
  /// # Panics
  /// When the process has a subscriber already
  #[must_use]
  pub fn install(span_closes: bool) -> Self {
    let captured = Self::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
      .with_writer(move || writer.clone())
      .with_ansi(false)
      .with_max_level(tracing::Level::DEBUG)
      .with_span_events(if span_closes { FmtSpan::CLOSE } else { FmtSpan::NONE })
      .finish();
    tracing::subscriber::set_global_default(subscriber).expect("the first subscriber of this process");
    captured
  }

  /// The text of the lines so far
  ///
  /// # Panics
  /// When a line is no UTF-8
  #[must_use]
  pub fn text(&self) -> String {
    String::from_utf8(self.0.lock().unwrap_or_else(PoisonError::into_inner).clone()).expect("UTF-8 log lines")
  }
}

impl Write for CapturedLog {
  fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
    self.0.lock().unwrap_or_else(PoisonError::into_inner).extend_from_slice(bytes);
    Ok(bytes.len())
  }

  fn flush(&mut self) -> std::io::Result<()> {
    Ok(())
  }
}

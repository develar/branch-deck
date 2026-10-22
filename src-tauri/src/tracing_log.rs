//! The app's `tracing` events as records of the `log` facade, which `tauri_plugin_log` writes, each with the spans it ran
//! in: `get_pull_requests{params=…}:operation{operation_id=…}: message`.
//!
//! Without a subscriber, `tracing` hands its events to `log` itself (its `log` feature) but drops their spans, so the log
//! could not tell which command or operation wrote a line. Once [`install`] set this subscriber, `tracing` stops
//! emitting records of its own; what crates log through `log` directly goes to the plugin as before.

use std::io::{self, Write};
use tracing::{Level, Metadata};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{Layer, Registry, fmt};

/// Sets the global `tracing` subscriber that formats each event with its spans and hands the line to `log`; debug and
/// above, the level of the plugin's log
pub fn install() {
  let layer = fmt::layer()
    .with_writer(LogRecords)
    .with_ansi(false)
    // the plugin writes the time, the level and the target of the record
    .without_time()
    .with_level(false)
    .with_target(false)
    // a line when a span starts, as `tracing` wrote one for each span before: which command ran, which git it started
    .with_span_events(FmtSpan::NEW)
    .with_filter(LevelFilter::DEBUG);
  if let Err(error) = tracing::subscriber::set_global_default(Registry::default().with(layer)) {
    log::warn!("Cannot route tracing to the log: {error}");
  }
}

/// Makes a [`LogRecord`] for each event, with the event's level and target
struct LogRecords;

impl<'a> MakeWriter<'a> for LogRecords {
  type Writer = LogRecord;

  fn make_writer(&'a self) -> Self::Writer {
    LogRecord::new(Level::INFO, "branch_deck", None, None, None)
  }

  fn make_writer_for(&'a self, meta: &Metadata<'_>) -> Self::Writer {
    LogRecord::new(*meta.level(), meta.target(), meta.module_path(), meta.file(), meta.line())
  }
}

/// The text of one formatted event, logged as one record when the formatter drops it
struct LogRecord {
  level: log::Level,
  target: String,
  module_path: Option<String>,
  file: Option<String>,
  line: Option<u32>,
  text: Vec<u8>,
}

impl LogRecord {
  fn new(level: Level, target: &str, module_path: Option<&str>, file: Option<&str>, line: Option<u32>) -> Self {
    let level = match level {
      Level::ERROR => log::Level::Error,
      Level::WARN => log::Level::Warn,
      Level::INFO => log::Level::Info,
      Level::DEBUG => log::Level::Debug,
      Level::TRACE => log::Level::Trace,
    };
    Self {
      level,
      target: target.to_owned(),
      module_path: module_path.map(str::to_owned),
      file: file.map(str::to_owned),
      line,
      text: Vec::new(),
    }
  }
}

impl Write for LogRecord {
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    self.text.extend_from_slice(bytes);
    Ok(bytes.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

impl Drop for LogRecord {
  fn drop(&mut self) {
    let text = String::from_utf8_lossy(&self.text);
    let text = text.trim_end();
    if text.is_empty() {
      return;
    }
    let logger = log::logger();
    let metadata = log::Metadata::builder().level(self.level).target(&self.target).build();
    if logger.enabled(&metadata) {
      logger.log(
        &log::Record::builder()
          .metadata(metadata)
          .module_path(self.module_path.as_deref())
          .file(self.file.as_deref())
          .line(self.line)
          .args(format_args!("{text}"))
          .build(),
      );
    }
  }
}

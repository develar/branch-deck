use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use std::sync::{Mutex, PoisonError};
use tracing::instrument;

/// The records the app's log would write: level, target and text
static RECORDS: Mutex<Vec<(log::Level, String, String)>> = Mutex::new(Vec::new());

struct CapturedLog;

impl log::Log for CapturedLog {
  fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
    true
  }

  fn log(&self, record: &log::Record<'_>) {
    let entry = (record.level(), record.target().to_owned(), record.args().to_string());
    RECORDS.lock().unwrap_or_else(PoisonError::into_inner).push(entry);
  }

  fn flush(&self) {}
}

static LOGGER: CapturedLog = CapturedLog;

/// A command as the app writes one: its span names it, its git work runs through `run_blocking`
#[instrument]
async fn get_widget(repository_path: String) -> Result<(), CommandError> {
  run_blocking(|| {
    tracing::debug!(files = 3, "reading the widget");
    Err::<(), _>("no widget".to_string())
  })
  .await
}

#[tokio::test]
async fn a_log_line_of_a_command_names_the_command_and_its_operation() {
  log::set_logger(&LOGGER).unwrap();
  log::set_max_level(log::LevelFilter::Debug);
  crate::tracing_log::install();

  let error = get_widget("/work/app".to_owned()).await.unwrap_err();
  let operation_id = error.operation_id.unwrap();

  let records = RECORDS.lock().unwrap_or_else(PoisonError::into_inner).clone();
  let command = r#"get_widget{repository_path="/work/app"}"#;
  let operation = format!("{command}:operation{{operation_id={operation_id}}}");
  let expected = vec![
    // the start of each span, then the event in both
    (log::Level::Info, module_path!().to_owned(), format!("{command}: new")),
    (log::Level::Info, "branch_deck_lib::blocking".to_owned(), format!("{operation}: new")),
    (log::Level::Debug, module_path!().to_owned(), format!("{operation}: reading the widget files=3")),
  ];
  assert_eq!(records, expected);
}

use crate::operation::{operation, run_blocking};
use domain_types::error::ApiError;
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
async fn get_widget(repository_path: String) -> Result<(), ApiError> {
  run_blocking(|| {
    tracing::debug!(files = 3, "reading the widget");
    Err::<(), _>("no widget".to_string())
  })
  .await
}

/// A command whose work is async, a service call, with a blocking step inside: one operation
#[instrument]
async fn restart_widget(widget_id: u32) -> Result<(), ApiError> {
  operation(async move {
    tracing::debug!("asking the service");
    let _ = run_blocking(move || Ok::<_, anyhow::Error>(widget_id)).await?;
    tracing::debug!("the service refused");
    Err(ApiError::internal("refused"))
  })
  .await
}

fn install_captured_log() {
  log::set_logger(&LOGGER).unwrap();
  log::set_max_level(log::LevelFilter::Debug);
  crate::tracing_log::install();
}

fn records() -> Vec<(log::Level, String, String)> {
  RECORDS.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

#[tokio::test]
async fn a_log_line_of_a_command_names_the_command_and_its_operation() {
  install_captured_log();

  let error = get_widget("/work/app".to_owned()).await.unwrap_err();
  let operation_id = error.operation_id.unwrap();

  let command = r#"get_widget{repository_path="/work/app"}"#;
  let operation = format!("{command}:operation{{operation_id={operation_id}}}");
  let expected = vec![
    // the start of each span, then the event in both
    (log::Level::Info, module_path!().to_owned(), format!("{command}: new")),
    // the span is built where the id is (`OperationId::span`), so its line names that module
    (log::Level::Info, "sync_utils::operation_id".to_owned(), format!("{operation}: new")),
    (log::Level::Debug, module_path!().to_owned(), format!("{operation}: reading the widget files=3")),
  ];
  assert_eq!(records(), expected);
}

#[tokio::test]
async fn the_async_lines_of_a_command_and_its_blocking_step_name_one_operation() {
  install_captured_log();

  let error = restart_widget(7).await.unwrap_err();
  let operation_id = error.operation_id.unwrap();

  let operation = format!("restart_widget{{widget_id=7}}:operation{{operation_id={operation_id}}}");
  let expected = vec![
    (log::Level::Info, module_path!().to_owned(), "restart_widget{widget_id=7}: new".to_owned()),
    (log::Level::Info, "sync_utils::operation_id".to_owned(), format!("{operation}: new")),
    (log::Level::Debug, module_path!().to_owned(), format!("{operation}: asking the service")),
    // the blocking step opens no operation of its own
    (log::Level::Debug, module_path!().to_owned(), format!("{operation}: the service refused")),
  ];
  assert_eq!(records(), expected);
}

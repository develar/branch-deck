use serde::Serialize;
use specta::Type;

#[derive(Clone, Serialize, Type)]
pub struct SyncEvent<'a> {
  pub message: &'a str,
}

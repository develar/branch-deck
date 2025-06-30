use serde::Serialize;
use specta::Type;

#[derive(Clone, Serialize, Type)]
pub struct SyncEvent {
  pub message: String,
  pub index: i16,
}

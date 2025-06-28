use serde::Serialize;
use specta::Type;

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "event", content = "data")]
pub enum SyncEvent<'a> {
  Progress { message: &'a str },
  Finished {},
}

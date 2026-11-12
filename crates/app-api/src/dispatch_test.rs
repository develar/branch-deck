use crate::dispatch::{no_channel, prepare, spec};
use crate::registry::CommandKind;
use serde_json::json;

/// A command is found by its name with the arguments the page sends: the object of its named arguments, its `params`,
/// or nothing for a command without arguments
#[test]
fn a_command_is_prepared_by_its_name_and_its_arguments() {
  let prepared = prepare("get_archived_branch_commits", json!({ "repositoryPath": "/r", "branchName": "b" })).unwrap();
  assert_eq!(prepared.spec.name, "get_archived_branch_commits");
  prepare("get_commit_files", json!({ "params": { "repositoryPath": "/r", "commitId": "c" } })).unwrap();
  prepare("cancel_symbol_search", serde_json::Value::Null).unwrap();
  prepare("cancel_symbol_search", json!({})).unwrap();
  assert_eq!(
    prepare(
      "get_blob_syntax",
      json!({ "params": { "repositoryPath": "/r", "blob": { "id": "a", "path": null }, "filePath": "f", "fromLine": 0, "toLine": 1, "text": null } })
    )
    .unwrap()
    .spec
    .kind,
    CommandKind::Raw
  );
}

/// A name no command has and the arguments of another command are refused before anything runs
#[test]
fn an_unknown_command_and_wrong_arguments_are_refused() {
  let unknown = prepare("nothing", json!({})).err().unwrap();
  assert_eq!(unknown.message, "No command is named nothing");
  let wrong = prepare("get_archived_branch_commits", json!({ "repositoryPath": "/r" })).err().unwrap();
  assert!(wrong.message.starts_with("Invalid arguments of get_archived_branch_commits: "), "{}", wrong.message);
  assert!(spec("nothing").is_none());
  assert_eq!(spec("execute").and_then(|spec| spec.channel.as_ref()).map(|channel| channel.name), Some("events"));
}

/// A call without a channel sends to none: a send fails as one to a page that went away
#[test]
fn no_channel_takes_nothing() {
  assert!(crate::transport::EventSink::send(&*no_channel(), json!(1)).is_err());
}

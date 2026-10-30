use crate::engine::{execute, prepare};
use crate::events::NoEvents;
use crate::land_execute::Accepted;
use crate::push::push_test::Services;
use crate::registry::registry_test::sessions;
use crate::registry::{Registry, RegistryFiles};
use crate::registry_file::registry_file_test::{record, safe_push, sent, session};
use crate::registry_file::{External, ProcessRegistry, Record, RecordStatus};
use domain_types::error::{ErrorCode, HolderTool};
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectStatus, Intent, OperationId};
use pretty_assertions::assert_eq;
use std::sync::Arc;

/// Forget This Start is an intent of the engine: its plan forgets the start, without a confirmation of its own (its
/// words say Patronus may still run it); its run takes the record out of the registry of the repository of the session,
/// and what the start held is free: the Safe Push of its branch is admitted again. A forget of an id the registry lacks
/// fails its effect
#[tokio::test(flavor = "multi_thread")]
async fn forget_this_start_takes_the_record_out_and_frees_what_it_held() {
  let directory = tempfile::tempdir().unwrap();
  let dir = directory.path().to_path_buf();
  let sessions = sessions();
  let key = session("user");
  let registry = Registry::new(Arc::clone(&sessions), RegistryFiles::new(HolderTool::App, move |_key| Some(dir.clone())).unflushed());
  let files = ProcessRegistry::of(directory.path()).unflushed();
  let gone = crate::registry_file::registry_file_test::process(4_000_000, Some(1));
  let lost = Record {
    status: RecordStatus::Unknown,
    external: Some(External {
      pending_start: Some(sent("auth", 1_800_000_000)),
    }),
    ..record("lost", &gone, &safe_push("auth"), 1_800_000_000)
  };
  files.insert_of_another_process(lost).unwrap();
  assert_eq!(
    files
      .admit(&OperationId("held".to_string()), HolderTool::App, &session("user"), &safe_push("auth"))
      .unwrap_err()
      .code,
    ErrorCode::Busy
  );

  let forget = Intent::ForgetStart {
    op: OperationId("lost".to_string()),
  };
  let plan = prepare(&sessions, &key, forget.clone()).await.unwrap().unwrap();
  assert_eq!(
    (plan.effects.as_slice(), plan.confirmations.as_slice()),
    (
      &[PlannedEffect::ForgetStart {
        op: OperationId("lost".to_string())
      }][..],
      &[][..]
    )
  );
  let accepted = || Accepted {
    fingerprint: plan.fingerprint.clone(),
    confirmations: Vec::new(),
  };
  let offline = Services { patronus: None, github: None };
  let outcome = execute(&registry, &key, forget.clone(), accepted(), offline.hosts(), Arc::new(NoEvents)).await.unwrap();
  assert_eq!(outcome.effects[0].status, EffectStatus::Done);
  assert_eq!(files.records().unwrap().unwrap(), Vec::<Record>::new());
  drop(
    files
      .admit(&OperationId("auth".to_string()), HolderTool::App, &session("user"), &safe_push("auth"))
      .unwrap(),
  );

  let again = execute(&registry, &key, forget, accepted(), offline.hosts(), Arc::new(NoEvents)).await.unwrap();
  let EffectStatus::Failed { error } = &again.effects[0].status else {
    panic!("{again:?}")
  };
  assert_eq!(error.code, ErrorCode::InvalidArguments);
}

use crate::error::{ApiError, ErrorCode, ErrorDetails, HolderTool, LockHolderInfo, OperationHolder, OperationKind, Retry};
use serde_json::json;

#[test]
fn an_error_without_data_is_its_snake_case_code_with_null_details_and_operation() {
  let error = ApiError::new(ErrorCode::OperationInProgress, "Not synced: rebase is in progress");
  assert_eq!(
    serde_json::to_value(&error).unwrap(),
    json!({
      "code": "operation_in_progress",
      "message": "Not synced: rebase is in progress",
      "details": null,
      "retry": "no",
      "operation_id": null,
    })
  );
  assert_eq!(serde_json::from_value::<ApiError>(serde_json::to_value(&error).unwrap()).unwrap(), error);
}

#[test]
fn an_error_carries_the_retry_of_its_code() {
  assert_eq!(ApiError::new(ErrorCode::Locked, "").retry, Retry::Same);
  assert_eq!(ApiError::new(ErrorCode::HeadMoved, "").retry, Retry::AfterReread);
  // a plan prepared over facts that changed is prepared again, never run as it was
  assert_eq!(ApiError::new(ErrorCode::PlanChanged, "").retry, Retry::AfterReread);
  assert_eq!(ErrorCode::PlanChanged.exit_code(), 3);
  assert_eq!(ApiError::new(ErrorCode::LeaseRejected, "").retry, Retry::No);
  assert_eq!(serde_json::to_value(Retry::AfterReread).unwrap(), json!("after_reread"));
}

#[test]
fn an_error_without_a_code_of_its_own_is_internal_and_canceled_is_a_code() {
  let error = ApiError::internal("cannot read HEAD: fatal: bad object");
  assert_eq!(error.code, ErrorCode::Internal);
  assert_eq!(serde_json::to_value(&error).unwrap()["code"], json!("internal"));
  assert_eq!(serde_json::to_value(ErrorCode::Canceled).unwrap(), json!("canceled"));
  assert_eq!((ErrorCode::Internal.exit_code(), ErrorCode::Internal.retry()), (5, Retry::No));
  assert_eq!((ErrorCode::Canceled.exit_code(), ErrorCode::Canceled.retry()), (4, Retry::No));
}

#[test]
fn the_details_are_tagged_by_their_kind_of_data() {
  let conflict = ApiError::new(ErrorCode::TargetConflict, "conflict").with_details(ErrorDetails::Conflict {
    commits: vec!["abc".to_owned()],
    paths: vec!["src/a.rs".to_owned()],
  });
  assert_eq!(
    serde_json::to_value(&conflict).unwrap()["details"],
    json!({"type": "conflict", "commits": ["abc"], "paths": ["src/a.rs"]})
  );
  let cases = [
    (ErrorDetails::Paths { paths: vec!["a".to_owned()] }, json!({"type": "paths", "paths": ["a"]})),
    (
      ErrorDetails::Branches {
        branches: vec!["user/virtual/auth".to_owned()],
      },
      json!({"type": "branches", "branches": ["user/virtual/auth"]}),
    ),
    (ErrorDetails::Blocks { blocks: vec!["b1".to_owned()] }, json!({"type": "blocks", "blocks": ["b1"]})),
    (ErrorDetails::Locked { holder: None }, json!({"type": "locked", "holder": null})),
    (
      ErrorDetails::PrefixSuggestion { prefix: "jane".to_owned() },
      json!({"type": "prefixSuggestion", "prefix": "jane"}),
    ),
  ];
  for (details, expected) in cases {
    assert_eq!(serde_json::to_value(&details).unwrap(), expected);
    assert_eq!(serde_json::from_value::<ErrorDetails>(expected).unwrap(), details);
  }
}

#[test]
fn a_locked_error_names_its_holder() {
  let error = ApiError::new(ErrorCode::Locked, "Another Branch Deck operation holds the repository").with_details(ErrorDetails::Locked {
    holder: Some(LockHolderInfo {
      tool: HolderTool::Bd,
      command: "bd push auth".to_owned(),
      pid: 4242,
      held_for: 40,
    }),
  });
  assert_eq!(
    serde_json::to_value(&error).unwrap(),
    json!({
      "code": "locked",
      "message": "Another Branch Deck operation holds the repository",
      "details": {"type": "locked", "holder": {"tool": "bd", "command": "bd push auth", "pid": 4242, "held_for": 40}},
      "retry": "same",
      "operation_id": null,
    })
  );
}

/// `busy` names the operation of another process that holds what the refused one needs, and retries as it is
#[test]
fn a_busy_error_names_the_operation_of_another_process_that_holds() {
  let error = ApiError::new(ErrorCode::Busy, "`bd` (pid 4242) has run the Safe Push of auth for 12 s").with_details(ErrorDetails::Busy {
    holder: Some(Box::new(OperationHolder {
      op: "01K".to_owned(),
      tool: HolderTool::Bd,
      pid: 4242,
      kind: OperationKind::SafePush,
      subject: Some("auth".to_owned()),
      held_for: 12,
      unknown: false,
    })),
  });
  assert_eq!(
    serde_json::to_value(&error).unwrap()["details"],
    json!({"type": "busy", "holder": {"op": "01K", "tool": "bd", "pid": 4242, "kind": "safe_push", "subject": "auth", "held_for": 12, "unknown": false}})
  );
  assert_eq!((error.retry, ErrorCode::Busy.exit_code()), (Retry::Same, 3));
}

#[test]
fn an_undo_that_a_moved_ref_refuses_is_stale_and_never_retried() {
  for code in [ErrorCode::HeadMoved, ErrorCode::GroupsMoved, ErrorCode::StacksMoved, ErrorCode::ParkedMoved] {
    assert!(code.moved_ref(), "{code:?}");
    // a request built again on the refs as they are now may pass
    assert_eq!(code.retry(), Retry::AfterReread, "{code:?}");
    let error = ApiError::new(code, "Branch master moved").in_undo();
    assert_eq!((error.code, error.retry, error.message.as_str()), (ErrorCode::UndoStale, Retry::No, "Branch master moved"));
  }
  assert_eq!(ErrorCode::InvalidPlan.in_undo(), ErrorCode::InvalidArguments);
  // a plan that changed, a failure after a write: no ref the request was made on moved
  for code in [ErrorCode::PlanChanged, ErrorCode::GitError, ErrorCode::Internal] {
    assert!(!code.moved_ref(), "{code:?}");
  }
  for code in [ErrorCode::Locked, ErrorCode::UncommittedOverlap, ErrorCode::Internal, ErrorCode::Canceled] {
    assert_eq!(ApiError::new(code, "").in_undo(), ApiError::new(code, ""));
  }
}

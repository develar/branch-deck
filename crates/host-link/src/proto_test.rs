use crate::proto::{
  DEFAULT_PORT, FileKind, PackTarget, Phase, Reply, RepositorySlots, Request, RunInfo, RunSpec, SCHEMA_VERSION, Stream, VersionLine, feature, features, is_run_id,
};
use crate::refusal::Code;
use pretty_assertions::assert_eq;

#[test]
fn an_agent_on_macos_has_the_upgrade_and_elsewhere_no_feature() {
  if cfg!(target_os = "macos") {
    assert_eq!(features(), [feature::UPGRADE]);
  } else {
    assert_eq!(features(), Vec::<&str>::new());
  }
  assert_eq!((DEFAULT_PORT, SCHEMA_VERSION), (7742, 1));
}

/// The JSON shape is the contract between two builds, so the tests pin it
#[test]
fn a_request_and_a_reply_have_a_tagged_json_shape() {
  let request = Request::Follow { run: "1-2".to_owned(), offset: 7 };
  let json = serde_json::to_string(&request).expect("json");
  assert_eq!(json, r#"{"request":"follow","run":"1-2","offset":7}"#);
  assert_eq!(serde_json::from_str::<Request>(&json).expect("parsed"), request);
  assert_eq!(serde_json::to_string(&Request::Runs).expect("json"), r#"{"request":"runs"}"#);
  assert_eq!(serde_json::to_string(&Request::Slots).expect("json"), r#"{"request":"slots"}"#);

  let reply = Reply::Output {
    stream: Stream::Stderr,
    text: "a line".to_owned(),
    end: 9,
  };
  assert_eq!(
    serde_json::to_string(&reply).expect("json"),
    r#"{"reply":"output","stream":"stderr","text":"a line","end":9}"#
  );
  let refused = Reply::refused(Code::SlotTimeout, "no slot within an hour");
  assert_eq!(
    serde_json::to_string(&refused).expect("json"),
    r#"{"reply":"refused","code":"slot_timeout","message":"no slot within an hour"}"#
  );
}

#[test]
fn a_pack_names_a_run_or_a_base_and_never_a_ref() {
  let pack = Request::Pack {
    repo: "~/projects/idea-1".to_owned(),
    commit: "abc".to_owned(),
    target: PackTarget::Run("1-2".to_owned()),
  };
  let json = serde_json::to_string(&pack).expect("json");
  assert_eq!(json, r#"{"request":"pack","repo":"~/projects/idea-1","commit":"abc","target":{"run":"1-2"}}"#);
  assert_eq!(serde_json::from_str::<Request>(&json).expect("parsed"), pack);
  let base: PackTarget = serde_json::from_str(r#"{"base":"checkout-0123456789ab"}"#).expect("a base");
  assert_eq!(base, PackTarget::Base("checkout-0123456789ab".to_owned()));
  assert_eq!(base.name(), "checkout-0123456789ab");
  assert!(base.is_valid());

  // a ref name is no target, and a name outside the rules of a host ref name is no valid one
  serde_json::from_str::<Request>(r#"{"request":"pack","repo":"r","commit":"c","reference":"refs/heads/master"}"#).expect_err("a ref name");
  serde_json::from_str::<PackTarget>(r#"{"ref":"refs/heads/master"}"#).expect_err("another kind");
  for bad in ["", "-a", ".a", "a/b", "a..b", "a.lock", "../master"] {
    assert!(!PackTarget::Base(bad.to_owned()).is_valid(), "{bad:?}");
    assert!(!PackTarget::Run(bad.to_owned()).is_valid(), "{bad:?}");
  }
}

#[test]
fn a_start_carries_the_run_spec_inline_without_a_ref() {
  let spec = RunSpec {
    run: "1-2".to_owned(),
    repo: "~/projects/idea-1".to_owned(),
    affinity: "checkout-0123456789ab".to_owned(),
    commit: "abc".to_owned(),
    command: vec!["./community/tools/bt.cmd".to_owned(), "--lane".to_owned(), "fast".to_owned()],
    fetch: vec!["out/bt-runs".to_owned()],
    ..RunSpec::default()
  };
  let json = serde_json::to_string(&Request::Start(spec.clone())).expect("json");
  assert_eq!(
    json,
    r#"{"request":"start","run":"1-2","repo":"~/projects/idea-1","affinity":"checkout-0123456789ab","commit":"abc","command":["./community/tools/bt.cmd","--lane","fast"],"fetch":["out/bt-runs"]}"#
  );
  assert_eq!(serde_json::from_str::<Request>(&json).expect("parsed"), Request::Start(spec));
}

#[test]
fn a_run_id_names_one_directory_and_one_ref() {
  for good in ["1792789527302-61884", "a.b_c-d"] {
    assert!(is_run_id(good), "{good}");
  }
  // git refuses `a..b`, `x.lock` as the last part of a ref, and the first character is a letter or a digit
  for bad in ["", ".", "..", "-1", "a/b", "a b", "a..b", "x.lock", &"x".repeat(129)] {
    assert!(!is_run_id(bad), "{bad:?}");
  }
}

#[test]
fn the_phases_of_a_run_and_a_run_info_without_them_parse() {
  let phases = Reply::Phases {
    phases: vec![Phase {
      name: "slot".to_owned(),
      ms: 1200,
    }],
  };
  let json = serde_json::to_string(&phases).expect("json");
  assert_eq!(json, r#"{"reply":"phases","phases":[{"name":"slot","ms":1200}]}"#);
  assert_eq!(serde_json::from_str::<Reply>(&json).expect("parsed"), phases);
  let info: RunInfo = serde_json::from_str(r#"{"run":"1-2","affinity":"a","commit":"c","command":["true"],"started_ms":1,"exit":null}"#).expect("an info");
  assert_eq!((info.phases.len(), info.fetch.len()), (0, 0));
}

#[test]
fn the_optional_fields_of_a_start_stay_out_of_the_json_and_a_spec_without_them_parses() {
  let plain = r#"{"request":"start","run":"1-2","repo":"~/r","affinity":"a","commit":"c","command":["true"],"fetch":[]}"#;
  let Request::Start(spec) = serde_json::from_str::<Request>(plain).expect("a start without the optional fields") else {
    panic!("not a start");
  };
  assert_eq!((spec.nice, spec.outputs, spec.changes), (None, false, false));
  assert_eq!(serde_json::to_string(&Request::Start(spec.clone())).expect("json"), plain);

  let new = RunSpec {
    nice: Some(10),
    outputs: true,
    changes: true,
    ..spec
  };
  let json = serde_json::to_string(&Request::Start(new.clone())).expect("json");
  assert!(json.ends_with(r#""fetch":[],"nice":10,"outputs":true,"changes":true}"#), "{json}");
  assert_eq!(serde_json::from_str::<Request>(&json).expect("parsed"), Request::Start(new));
}

#[test]
fn a_hello_without_features_and_a_file_without_a_kind_parse() {
  let hello: Reply = serde_json::from_str(r#"{"reply":"hello","schema_version":1,"agent_version":"0.37.0","os":"linux","arch":"x86_64"}"#).expect("a hello without features");
  assert!(matches!(&hello, Reply::Hello { features, binary_sha256: None, .. } if features.is_empty()), "{hello:?}");

  let file: Reply = serde_json::from_str(r#"{"reply":"file","path":"out/a","size":1,"executable":false}"#).expect("a file");
  assert!(matches!(&file, Reply::File { kind: FileKind::Staged, .. }), "{file:?}");
  assert_eq!(
    serde_json::to_string(&file).expect("json"),
    r#"{"reply":"file","path":"out/a","size":1,"executable":false}"#
  );

  let output = Reply::File {
    path: "tests/results/a.txt".to_owned(),
    size: 1,
    executable: false,
    kind: FileKind::Output { root: "tests/results".to_owned() },
  };
  let json = serde_json::to_string(&output).expect("json");
  assert!(json.ends_with(r#""kind":{"output":{"root":"tests/results"}}}"#), "{json}");
  assert_eq!(serde_json::from_str::<Reply>(&json).expect("parsed"), output);
  let changes = r#"{"reply":"file","path":"changes.patch","size":3,"executable":false,"kind":"changes"}"#;
  assert!(matches!(
    serde_json::from_str::<Reply>(changes).expect("parsed"),
    Reply::File { kind: FileKind::Changes, .. }
  ));
}

#[test]
fn a_hello_names_the_binary_and_the_features_of_this_build() {
  let hello = Reply::hello("0.37.0", Some("ab".repeat(32)));
  let json = serde_json::to_string(&hello).expect("json");
  assert!(json.starts_with(r#"{"reply":"hello","schema_version":1,"agent_version":"0.37.0","#), "{json}");
  assert!(json.ends_with(&format!(r#""binary_sha256":"{}"}}"#, "ab".repeat(32))), "{json}");
  assert_eq!(serde_json::from_str::<Reply>(&json).expect("parsed"), hello);
  let Reply::Hello { features, .. } = hello else {
    panic!("not a hello");
  };
  assert_eq!(features.contains(&feature::UPGRADE.to_owned()), cfg!(target_os = "macos"), "{features:?}");
  assert_eq!(
    serde_json::to_string(&Request::hello("0.37.0")).expect("json"),
    r#"{"request":"hello","schema_version":1,"client_version":"0.37.0"}"#
  );
}

#[test]
fn an_upgrade_and_its_reply_have_a_tagged_json_shape() {
  let request = Request::Upgrade {
    size: 9,
    sha256: "cd".repeat(32),
    force: false,
  };
  let json = serde_json::to_string(&request).expect("json");
  assert_eq!(json, format!(r#"{{"request":"upgrade","size":9,"sha256":"{}"}}"#, "cd".repeat(32)));
  assert_eq!(serde_json::from_str::<Request>(&json).expect("parsed"), request);
  let forced = Request::Upgrade {
    size: 9,
    sha256: "cd".repeat(32),
    force: true,
  };
  let json = serde_json::to_string(&forced).expect("json");
  assert!(json.ends_with(r#","force":true}"#), "{json}");
  assert_eq!(serde_json::from_str::<Request>(&json).expect("parsed"), forced);

  let reply = Reply::Upgraded {
    previous_sha256: "ab".repeat(32),
    sha256: "cd".repeat(32),
    version: "0.37.0".to_owned(),
  };
  let json = serde_json::to_string(&reply).expect("json");
  assert!(json.starts_with(r#"{"reply":"upgraded","previous_sha256":"#), "{json}");
  assert_eq!(serde_json::from_str::<Reply>(&json).expect("parsed"), reply);
}

#[test]
fn the_version_line_names_the_build_and_a_check_adds_the_launch_agent() {
  let line = VersionLine::of_this_build("0.37.0", Some("ab".repeat(32)));
  let json = serde_json::to_string(&line).expect("json");
  let features = if cfg!(target_os = "macos") { r#"["upgrade"]"# } else { "[]" };
  assert_eq!(json, format!(r#"{{"version":"0.37.0","schema":1,"features":{features},"sha256":"{}"}}"#, "ab".repeat(32)));
  let checked: VersionLine = serde_json::from_str(r#"{"version":"0.37.0","schema":1,"features":["upgrade"],"sha256":null,"plist":"same"}"#).expect("a check line");
  assert_eq!((checked.plist.as_deref(), checked.sha256), (Some("same"), None));
}

#[test]
fn the_slots_of_the_agent_carry_the_lend_table_as_the_pool_writes_it() {
  let reply = Reply::Slots {
    repositories: vec![RepositorySlots {
      repo: "~/projects/idea-1".to_owned(),
      table: serde_json::json!({"slots": [], "queue": []}),
    }],
  };
  let json = serde_json::to_string(&reply).expect("json");
  assert_eq!(json, r#"{"reply":"slots","repositories":[{"repo":"~/projects/idea-1","table":{"queue":[],"slots":[]}}]}"#);
  assert_eq!(serde_json::from_str::<Reply>(&json).expect("parsed"), reply);
}

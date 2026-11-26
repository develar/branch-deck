use crate::proto::{
  DEFAULT_PORT, Estimate, EstimateAsk, FileKind, PackTarget, Phase, Piece, Reply, RepositorySlots, Request, RunInfo, RunRefusal, RunSlot, RunSpec, SCHEMA_VERSION, Stream,
  VersionLine, feature, features, is_run_id,
};
use crate::refusal::Code;
use pretty_assertions::assert_eq;

#[test]
fn an_agent_on_macos_has_the_upgrade_and_every_agent_the_disk_the_trim_and_the_heavy_turn() {
  if cfg!(target_os = "macos") {
    assert_eq!(
      features(),
      [
        feature::UPGRADE,
        feature::DISK,
        feature::TRIM,
        feature::HEAVY,
        feature::EXCLUSIVE,
        feature::STATS,
        feature::ESTIMATE,
        feature::MEASURE_AS,
        feature::GROUP
      ]
    );
  } else {
    assert_eq!(
      features(),
      [
        feature::DISK,
        feature::TRIM,
        feature::HEAVY,
        feature::EXCLUSIVE,
        feature::STATS,
        feature::ESTIMATE,
        feature::MEASURE_AS,
        feature::GROUP
      ]
    );
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
  assert_eq!(serde_json::to_string(&Request::Runs { run: None }).expect("json"), r#"{"request":"runs"}"#);
  assert_eq!(serde_json::to_string(&Request::Slots).expect("json"), r#"{"request":"slots"}"#);
  assert_eq!(serde_json::to_string(&Request::Disk { fresh: false }).expect("json"), r#"{"request":"disk"}"#);
  assert_eq!(
    serde_json::from_str::<Request>(r#"{"request":"disk","fresh":true}"#).expect("parsed"),
    Request::Disk { fresh: true }
  );
  let disk = Reply::Disk {
    report: serde_json::json!({"bytes": 7}),
  };
  assert_eq!(serde_json::to_string(&disk).expect("json"), r#"{"reply":"disk","report":{"bytes":7}}"#);
  assert_eq!(serde_json::to_string(&Request::Stats { days: 7 }).expect("json"), r#"{"request":"stats","days":7}"#);
  let stats = Reply::Stats {
    report: serde_json::json!({"runs": 3}),
  };
  assert_eq!(serde_json::to_string(&stats).expect("json"), r#"{"reply":"stats","report":{"runs":3}}"#);
  assert_eq!(serde_json::to_string(&Request::Trim { max: None, plan: false }).expect("json"), r#"{"request":"trim"}"#);
  assert_eq!(
    serde_json::from_str::<Request>(r#"{"request":"trim","max":6,"plan":true}"#).expect("parsed"),
    Request::Trim { max: Some(6), plan: true }
  );
  let trimmed = Reply::Trimmed {
    report: serde_json::json!({"freed_bytes": 7}),
  };
  assert_eq!(serde_json::to_string(&trimmed).expect("json"), r#"{"reply":"trimmed","report":{"freed_bytes":7}}"#);

  let reply = Reply::Output {
    stream: Stream::Stderr,
    piece: Piece::of(b"a line", false),
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

/// The reply of a piece as an older client reads it: before `partial` and `base64`
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(tag = "reply", rename_all = "snake_case")]
enum OlderReply {
  Output { stream: Stream, text: String, end: u64 },
}

/// A piece of UTF-8 is its text, and a piece that is not UTF-8 has its bytes in base64 beside its lossy text. A partial
/// piece has no line end after it in its stream. The fields are optional: a piece of an older agent is a whole line,
/// and an older client reads the text of a piece of a newer agent
#[test]
fn a_piece_keeps_its_bytes_and_an_older_end_reads_it() {
  let partial = Reply::Output {
    stream: Stream::Stdout,
    piece: Piece::of(b"\xff\xfe a", true),
    end: 12,
  };
  let json = serde_json::to_string(&partial).expect("json");
  assert_eq!(
    serde_json::from_str::<serde_json::Value>(&json).expect("parsed"),
    serde_json::json!({"reply": "output", "stream": "stdout", "text": "\u{fffd}\u{fffd} a", "partial": true, "base64": "//4gYQ==", "end": 12})
  );
  assert_eq!(serde_json::from_str::<Reply>(&json).expect("parsed"), partial);
  assert_eq!(
    serde_json::from_str::<OlderReply>(&json).expect("an older client reads it"),
    OlderReply::Output {
      stream: Stream::Stdout,
      text: "\u{fffd}\u{fffd} a".to_owned(),
      end: 12
    }
  );
  let Reply::Output { piece, .. } = partial else { unreachable!() };
  assert_eq!(piece.into_stream_bytes().expect("the bytes"), b"\xff\xfe a", "no line end after a partial piece");

  let older: Reply = serde_json::from_str(r#"{"reply":"output","stream":"stderr","text":"a line","end":7}"#).expect("a piece of an older agent");
  let Reply::Output { piece, .. } = older else { panic!("{older:?}") };
  assert_eq!(piece, Piece::of(b"a line", false));
  assert_eq!(piece.into_stream_bytes().expect("the bytes"), b"a line\n", "a whole line");

  let bad = Piece {
    base64: Some("not base64!".to_owned()),
    ..Piece::default()
  };
  assert!(bad.into_stream_bytes().is_err(), "base64 that does not decode is an error of the protocol");
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
  assert_eq!((info.ended_ms, info.forgotten), (None, false), "an older agent sends no end and no forget");
}

/// The end and the forget of a kept run are optional fields within the schema: a run that goes on sends neither, and a
/// forgotten run both
#[test]
fn the_end_and_the_forget_of_a_run_info_are_optional_fields_within_the_schema() {
  let going = r#"{"run":"1-2","affinity":"a","commit":"c","command":["true"],"started_ms":1,"exit":null,"phases":[],"fetch":[]}"#;
  let info: RunInfo = serde_json::from_str(going).expect("an info");
  assert_eq!(serde_json::to_string(&info).expect("json"), going);
  let forgotten = RunInfo {
    ended_ms: Some(5),
    exit: Some(0),
    forgotten: true,
    ..info
  };
  let json = serde_json::to_string(&forgotten).expect("json");
  assert_eq!(
    json,
    r#"{"run":"1-2","affinity":"a","commit":"c","command":["true"],"started_ms":1,"ended_ms":5,"exit":0,"phases":[],"fetch":[],"forgotten":true}"#
  );
  assert_eq!(serde_json::from_str::<RunInfo>(&json).expect("parsed"), forgotten);
  let superseded = RunInfo {
    exit: Some(255),
    refused: Some(RunRefusal {
      code: "superseded".to_owned(),
      message: "run 1-2 was superseded".to_owned(),
    }),
    ..forgotten
  };
  let json = serde_json::to_string(&superseded).expect("json");
  assert!(
    json.ends_with(r#""forgotten":true,"refused":{"code":"superseded","message":"run 1-2 was superseded"}}"#),
    "{json}"
  );
  assert_eq!(serde_json::from_str::<RunInfo>(&json).expect("parsed"), superseded);
  let lent = RunInfo {
    repo: Some("/Users/u/idea".to_owned()),
    slot: Some(RunSlot {
      n: 3,
      choice: "affinity".to_owned(),
      waited: true,
      cleaned: true,
      missed: None,
    }),
    ..superseded
  };
  let json = serde_json::to_string(&lent).expect("json");
  assert!(
    json.ends_with(r#""repo":"/Users/u/idea","slot":{"n":3,"choice":"affinity","waited":true,"cleaned":true}}"#),
    "{json}"
  );
  assert_eq!(serde_json::from_str::<RunInfo>(&json).expect("parsed"), lent);
  let missed: RunSlot = serde_json::from_str(r#"{"n":1,"choice":"idle","missed":"lent"}"#).expect("a slot");
  assert_eq!(missed.missed.as_deref(), Some("lent"));
  let at_once: RunSlot = serde_json::from_str(r#"{"n":1,"choice":"a choice of a later build"}"#).expect("a slot");
  assert_eq!(
    (at_once.choice.as_str(), at_once.waited, at_once.cleaned, at_once.missed),
    ("a choice of a later build", false, false, None)
  );
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
  let features = if cfg!(target_os = "macos") {
    r#"["upgrade","disk","trim","heavy","exclusive","stats","estimate","measure_as","group"]"#
  } else {
    r#"["disk","trim","heavy","exclusive","stats","estimate","measure_as","group"]"#
  };
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
      abandoned: Vec::new(),
    }],
  };
  let json = serde_json::to_string(&reply).expect("json");
  assert_eq!(json, r#"{"reply":"slots","repositories":[{"repo":"~/projects/idea-1","table":{"queue":[],"slots":[]}}]}"#);
  assert_eq!(serde_json::from_str::<Reply>(&json).expect("parsed"), reply);
}

/// The agent judges the abandoned slots by the processes of its host and names them beside the table; the reply of an
/// older agent, without them, parses and marks no slot
#[test]
fn the_abandoned_slots_come_from_the_agent_and_an_older_agent_names_none() {
  let repository = RepositorySlots {
    repo: "~/projects/idea-1".to_owned(),
    table: serde_json::json!({"slots": []}),
    abandoned: vec![2],
  };
  let json = serde_json::to_string(&repository).expect("json");
  assert_eq!(json, r#"{"repo":"~/projects/idea-1","table":{"slots":[]},"abandoned":[2]}"#);
  assert_eq!(serde_json::from_str::<RepositorySlots>(&json).expect("parsed"), repository);
  let older: RepositorySlots = serde_json::from_str(r#"{"repo":"~/projects/idea-1","table":{"slots":[]}}"#).expect("an older reply");
  assert_eq!(older.abandoned, Vec::<u32>::new());
}

/// `RunSpec::heavy` and the heavy turn of `Reply::Runs` are optional fields: a run that is not heavy and an agent without
/// the feature `heavy` write neither, and an end that does not know them reads the message as before
#[test]
fn a_heavy_run_and_the_heavy_turn_are_optional_fields_within_the_schema() {
  let spec = RunSpec {
    run: "1-2".to_owned(),
    heavy: true,
    ..RunSpec::default()
  };
  let json = serde_json::to_string(&Request::Start(spec.clone())).expect("json");
  assert!(json.ends_with(r#","heavy":true}"#), "{json}");
  assert_eq!(serde_json::from_str::<Request>(&json).expect("parsed"), Request::Start(spec));
  let light = serde_json::to_string(&Request::Start(RunSpec::default())).expect("json");
  assert!(!light.contains("heavy"), "{light}");

  let older = serde_json::from_str::<Reply>(r#"{"reply":"runs","runs":[]}"#).expect("the runs of an older agent");
  assert_eq!(older, Reply::Runs { runs: Vec::new(), heavy: None });
  let reply = Reply::Runs {
    runs: Vec::new(),
    heavy: Some(serde_json::json!({"queue": []})),
  };
  let json = serde_json::to_string(&reply).expect("json");
  assert_eq!(json, r#"{"reply":"runs","runs":[],"heavy":{"queue":[]}}"#);
  assert_eq!(serde_json::from_str::<Reply>(&json).expect("parsed"), reply);
}

/// The request of the runs of an agent of the build before `estimate`: a unit variant, which reads a request of this
/// build with `run` as its own and ignores the field
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(tag = "request", rename_all = "snake_case")]
enum OlderRequest {
  Runs,
}

/// `Request::Runs` of one run is an optional field within the schema: an older agent reads it as the request of every
/// run and answers them all, so the client finds its run in the list; the request of every run has the shape of before
#[test]
fn the_runs_of_one_run_are_an_optional_field_that_an_older_agent_ignores() {
  let one = Request::Runs { run: Some("1-2".to_owned()) };
  let json = serde_json::to_string(&one).expect("json");
  assert_eq!(json, r#"{"request":"runs","run":"1-2"}"#);
  assert_eq!(serde_json::from_str::<Request>(&json).expect("parsed"), one);
  assert_eq!(serde_json::from_str::<OlderRequest>(&json).expect("an older agent reads it"), OlderRequest::Runs);
  assert_eq!(
    serde_json::from_str::<Request>(r#"{"request":"runs"}"#).expect("the request of an older client"),
    Request::Runs { run: None }
  );
}

/// The estimate of a run and its answer are a request and a reply of their own, behind the feature `estimate`; the
/// fields of a measure are left out without one, and `admit_now` of a start stays out of its JSON when it is off
#[test]
fn an_estimate_and_its_reply_have_a_tagged_json_shape() {
  let ask = Request::Estimate(EstimateAsk {
    repo: "~/projects/idea-1".to_owned(),
    affinity: "checkout-0123456789ab".to_owned(),
    commit: "abc".to_owned(),
    command: vec!["pnpm".to_owned(), "e2e".to_owned()],
    fetch: Vec::new(),
    heavy: true,
    measure_as: None,
    group: None,
  });
  let json = serde_json::to_string(&ask).expect("json");
  assert_eq!(
    json,
    r#"{"request":"estimate","repo":"~/projects/idea-1","affinity":"checkout-0123456789ab","commit":"abc","command":["pnpm","e2e"],"fetch":[],"heavy":true}"#
  );
  assert_eq!(serde_json::from_str::<Request>(&json).expect("parsed"), ask);

  let unmeasured = Reply::Estimate(Estimate {
    has_commit: false,
    warm: true,
    cores: 20,
    expected_cores: None,
    expected_wall_ms: None,
    start_in_ms: 720_000,
    unknown_ahead: 2,
    os: "macos".to_owned(),
    slots: None,
  });
  let json = serde_json::to_string(&unmeasured).expect("json");
  assert_eq!(
    json,
    r#"{"reply":"estimate","has_commit":false,"warm":true,"cores":20,"start_in_ms":720000,"unknown_ahead":2,"os":"macos"}"#
  );
  assert_eq!(serde_json::from_str::<Reply>(&json).expect("parsed"), unmeasured);
  let measured = Reply::Estimate(Estimate {
    expected_cores: Some(sync_utils::cores::Cores(90)),
    expected_wall_ms: Some(240_000),
    ..match unmeasured {
      Reply::Estimate(estimate) => estimate,
      _ => unreachable!("an estimate"),
    }
  });
  let json = serde_json::to_string(&measured).expect("json");
  assert!(json.contains(r#""cores":20,"expected_cores":9.0,"expected_wall_ms":240000,"#), "{json}");
  assert_eq!(serde_json::from_str::<Reply>(&json).expect("parsed"), measured);
  let without_slots = Reply::Estimate(Estimate {
    slots: Some(0),
    ..match measured {
      Reply::Estimate(estimate) => estimate,
      _ => unreachable!("an estimate"),
    }
  });
  let json = serde_json::to_string(&without_slots).expect("json");
  assert!(
    json.ends_with(r#""os":"macos","slots":0}"#),
    "the slot limit, which an agent before it sends none of: {json}"
  );
  assert_eq!(serde_json::from_str::<Reply>(&json).expect("parsed"), without_slots);

  let admitting = RunSpec {
    run: "1-2".to_owned(),
    admit_now: true,
    ..RunSpec::default()
  };
  let json = serde_json::to_string(&Request::Start(admitting.clone())).expect("json");
  assert!(json.ends_with(r#","admit_now":true}"#), "{json}");
  assert_eq!(serde_json::from_str::<Request>(&json).expect("parsed"), Request::Start(admitting));
}

/// The class of the measures of a run (`--measure-as`) is an optional field of a start and of an estimate, behind the
/// feature `measure_as`: it stays out of the JSON without a class, so an older agent reads the start as before
#[test]
fn the_class_of_the_measures_is_an_optional_field_of_a_start_and_an_estimate() {
  assert!(features().contains(&feature::MEASURE_AS));
  let classed = RunSpec {
    run: "1-2".to_owned(),
    heavy: true,
    measure_as: Some("accio-typing-gate".to_owned()),
    ..RunSpec::default()
  };
  let json = serde_json::to_string(&Request::Start(classed.clone())).expect("json");
  assert!(json.ends_with(r#","heavy":true,"measure_as":"accio-typing-gate"}"#), "{json}");
  assert_eq!(serde_json::from_str::<Request>(&json).expect("parsed"), Request::Start(classed));
  let plain = serde_json::to_string(&Request::Start(RunSpec::default())).expect("json");
  assert!(!plain.contains("measure_as"), "{plain}");

  let ask = Request::Estimate(EstimateAsk {
    heavy: true,
    measure_as: Some("accio-typing-gate".to_owned()),
    ..EstimateAsk::default()
  });
  let json = serde_json::to_string(&ask).expect("json");
  assert!(json.ends_with(r#""heavy":true,"measure_as":"accio-typing-gate"}"#), "{json}");
  assert_eq!(serde_json::from_str::<Request>(&json).expect("parsed"), ask);
}

/// The group of a run in the heavy queue (`--group`) is an optional field of a start and of an estimate: it stays out of
/// the JSON without a group, so an older agent reads the start as before, and one that gets it ignores it
#[test]
fn the_group_of_a_run_is_an_optional_field_of_a_start_and_an_estimate() {
  assert!(features().contains(&feature::GROUP));
  let grouped = RunSpec {
    run: "1-2".to_owned(),
    heavy: true,
    group: Some("typing-session".to_owned()),
    ..RunSpec::default()
  };
  let json = serde_json::to_string(&Request::Start(grouped.clone())).expect("json");
  assert!(json.ends_with(r#","heavy":true,"group":"typing-session"}"#), "{json}");
  assert_eq!(serde_json::from_str::<Request>(&json).expect("parsed"), Request::Start(grouped));
  let plain = serde_json::to_string(&Request::Start(RunSpec::default())).expect("json");
  assert!(!plain.contains("group"), "{plain}");

  let ask = Request::Estimate(EstimateAsk {
    heavy: true,
    group: Some("typing-session".to_owned()),
    ..EstimateAsk::default()
  });
  let json = serde_json::to_string(&ask).expect("json");
  assert!(json.ends_with(r#""heavy":true,"group":"typing-session"}"#), "{json}");
  assert_eq!(serde_json::from_str::<Request>(&json).expect("parsed"), ask);
}

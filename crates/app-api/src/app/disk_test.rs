use crate::app::disk::{DiskOverview, DiskUsageEvent, hosts_of};

/// The output of `bd-host disk --json`: a host with its report, a host that refused, a host that answered nothing
#[test]
fn the_hosts_of_bd_host_disk_are_its_reports_and_its_refusals() {
  let stdout = br#"{"schema":1,"hosts":[
    {"alias":"studio","report":{"free_bytes":1048576,"pools":[],"bytes":0,"ms":3}},
    {"alias":"mini","refusal":{"code":"agent_failed","message":"the agent on mini has no feature disk"}},
    {"alias":"air"}
  ]}"#;
  let hosts = hosts_of(stdout).unwrap();
  assert_eq!(hosts.iter().map(|host| host.alias.as_str()).collect::<Vec<_>>(), ["studio", "mini", "air"]);
  assert_eq!(hosts[0].report.as_ref().unwrap().free_bytes, Some(1_048_576));
  assert_eq!(hosts[1].report.as_ref().unwrap_err(), "the agent on mini has no feature disk");
  assert_eq!(hosts[2].report.as_ref().unwrap_err(), "the host answered no report");
  hosts_of(b"bd-host: not json").unwrap_err();
}

#[test]
fn disk_usage_events_serialize_their_channel_type() {
  let event = DiskUsageEvent::Cached {
    overview: DiskOverview { machines: Vec::new() },
    measuring: 2,
  };

  assert_eq!(
    serde_json::to_value(event).unwrap(),
    serde_json::json!({"type": "cached", "overview": {"machines": []}, "measuring": 2})
  );
  assert_eq!(
    serde_json::to_value(DiskUsageEvent::Done { ms: 12 }).unwrap(),
    serde_json::json!({"type": "done", "ms": 12})
  );
  assert_eq!(
    serde_json::to_value(DiskUsageEvent::Planning { measuring: 3 }).unwrap(),
    serde_json::json!({"type": "planning", "measuring": 3})
  );
}

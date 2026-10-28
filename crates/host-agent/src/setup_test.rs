use std::path::PathBuf;

use pretty_assertions::assert_eq;

use super::*;
use std::path::Path;
use std::time::{Duration, Instant};

#[test]
fn launchd_loads_restarts_or_reloads() {
  assert_eq!(LaunchdStep::of(false, true), LaunchdStep::Bootstrap);
  assert_eq!(LaunchdStep::of(false, false), LaunchdStep::Bootstrap);
  assert_eq!(LaunchdStep::of(true, false), LaunchdStep::Kickstart);
  assert_eq!(LaunchdStep::of(true, true), LaunchdStep::Reload);
}

#[test]
fn the_agent_path_puts_the_tool_directories_first_once() {
  assert_eq!(
    agent_path(Path::new("/Users/me"), "/usr/bin:/Users/me/.local/bin:/opt/x::/bin"),
    "/Users/me/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/opt/x:/bin:/usr/sbin:/sbin"
  );
}

#[test]
fn the_plist_runs_serve_on_the_port_and_escapes_its_strings() {
  let plist = plist_text(
    &PathBuf::from("/Users/a&b/.local/bin/bd-host"),
    7742,
    Path::new("/Users/a&b/Library/Logs/bd-host.log"),
    "/usr/bin",
  );
  assert!(plist.contains("<string>com.jetbrains.bd-host</string>"), "{plist}");
  assert!(
    plist.contains("<string>/Users/a&amp;b/.local/bin/bd-host</string>\n    <string>serve</string>\n    <string>--port</string>\n    <string>7742</string>"),
    "{plist}"
  );
  assert!(plist.contains("<key>KeepAlive</key>\n  <true/>"), "{plist}");
  assert!(plist.contains("<key>RunAtLoad</key>\n  <true/>"), "{plist}");
  assert!(!plist.contains("a&b"), "{plist}");
  let values = PlistValues::parse(&plist).expect("the values");
  assert_eq!(
    values,
    PlistValues {
      exe: PathBuf::from("/Users/a&b/.local/bin/bd-host"),
      port: 7742,
      log: PathBuf::from("/Users/a&b/Library/Logs/bd-host.log"),
      path: "/usr/bin".to_owned(),
    }
  );
  assert_eq!(values.text(), plist);
  assert_eq!(PlistValues::parse("<plist><dict></dict></plist>"), None);
  let other = plist.replace("<string>serve</string>", "<string>serve</string>\n    <string>--bind</string>");
  assert_eq!(PlistValues::parse(&other), None, "another argument list");
}

#[test]
fn a_setup_line_has_the_json_shape_of_the_spec() {
  let line = SetupLine {
    fingerprint: "ab".to_owned(),
    port: 7741,
    launchd: "restarted",
  };
  assert_eq!(serde_json::to_string(&line).expect("json"), r#"{"fingerprint":"ab","port":7741,"launchd":"restarted"}"#);
}

#[test]
fn the_listen_wait_ends_when_the_port_takes_connections_and_refuses_after_its_time() {
  let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
  let port = listener.local_addr().expect("the address").port();
  assert_eq!(wait_for_listen(port, Duration::from_secs(5)), Ok(()));
  drop(listener);
  // on macOS a child of another test thread can get the listening socket before its close-on-exec flag is set, and
  // the socket then lives as long as that child. The check starts once the port refuses.
  let closed = Instant::now();
  while TcpStream::connect(("127.0.0.1", port)).is_ok() {
    assert!(closed.elapsed() < Duration::from_secs(60), "the port still takes connections");
    std::thread::sleep(Duration::from_millis(50));
  }
  let started = Instant::now();
  let refused = wait_for_listen(port, Duration::from_millis(300)).expect_err("nothing listens");
  assert!(refused.contains("nothing listens on 127.0.0.1"), "{refused}");
  assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
}

/// A fake launchctl that appends its arguments to `calls`: `print` answers that no service is loaded
fn fake_launchctl(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
  let calls = dir.join("launchctl-calls");
  let program = dir.join("launchctl");
  fs::write(&program, format!("#!/bin/sh\necho \"$*\" >>'{}'\n[ \"$1\" != print ]\n", calls.display())).expect("the fake");
  fs::set_permissions(&program, std::os::unix::fs::PermissionsExt::from_mode(0o755)).expect("executable");
  (program, calls)
}

fn read(path: &Path) -> String {
  fs::read_to_string(path).unwrap_or_default()
}

#[test]
fn the_agent_setup_writes_the_identity_the_allow_list_and_the_launch_agent() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let home = Home::new(dir.path().join("home"));
  let (launchctl, calls) = fake_launchctl(dir.path());
  let client = Identity::generate().expect("a client").fingerprint();
  let exe = dir.path().join("bd-host");
  fs::write(&exe, "the build of the setup").expect("the executable");
  let setup = Setup {
    client,
    name: "laptop",
    port: 7742,
    exe: &exe,
    launchctl: &launchctl,
    path: "/usr/bin",
    listen_wait: Duration::ZERO,
  };
  let mut out = Vec::new();
  setup_agent(&home, &setup, &mut out).expect("set up");
  let fingerprint = Identity::load(&home.identity_dir()).expect("the identity").fingerprint();
  let launchd = if cfg!(target_os = "macos") { "bootstrapped" } else { "none" };
  assert_eq!(
    String::from_utf8(out).expect("UTF-8"),
    format!("{{\"fingerprint\":\"{fingerprint}\",\"port\":7742,\"launchd\":\"{launchd}\"}}\n")
  );
  assert_eq!(AllowList::read(&home.allow_file()).expect("the allow list").name(&client), Some("laptop"));
  if cfg!(target_os = "macos") {
    let plist = read(&home.launch_agent_file());
    assert_eq!(PlistValues::parse(&plist).map(|values| (values.exe, values.port)), Some((home.binary_file(), 7742)));
    assert_eq!(read(&home.binary_file()), "the build of the setup", "launchd runs a copy, never the file of a build");
    let (domain, service) = service(LAUNCHD_LABEL);
    assert_eq!(read(&calls), format!("print {service}\nbootstrap {domain} {}\n", home.launch_agent_file().display()));
    fs::write(&exe, "another build").expect("the executable");
    setup_agent(&home, &setup, &mut Vec::new()).expect("set up again");
    assert_eq!(read(&home.binary_file()), "the build of the setup", "the binary of an upgrade stays");
  } else {
    assert!(!home.launch_agent_file().exists());
    assert_eq!(read(&calls), "");
  }
  let refused = setup_agent(&home, &Setup { name: "bad\nname", ..setup }, &mut Vec::new()).expect_err("a control character");
  assert_eq!(refused.code, Code::BadRequest);
}

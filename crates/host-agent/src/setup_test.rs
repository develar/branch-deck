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
    &PathBuf::from("/Users/a&b/.local/bin/brd-host"),
    7742,
    Path::new("/Users/a&b/Library/Logs/brd-host.log"),
    "/usr/bin",
  );
  assert!(plist.contains("<string>com.jetbrains.brd-host</string>"), "{plist}");
  assert!(
    plist.contains("<string>/Users/a&amp;b/.local/bin/brd-host</string>\n    <string>serve</string>\n    <string>--port</string>\n    <string>7742</string>"),
    "{plist}"
  );
  assert!(plist.contains("<key>KeepAlive</key>\n  <true/>"), "{plist}");
  assert!(plist.contains("<key>RunAtLoad</key>\n  <true/>"), "{plist}");
  assert!(!plist.contains("a&b"), "{plist}");
  let values = PlistValues::parse(&plist).expect("the values");
  assert_eq!(
    values,
    PlistValues {
      exe: PathBuf::from("/Users/a&b/.local/bin/brd-host"),
      port: 7742,
      log: PathBuf::from("/Users/a&b/Library/Logs/brd-host.log"),
      path: "/usr/bin".to_owned(),
    }
  );
  assert_eq!(values.text(), plist);
  assert_eq!(PlistValues::parse("<plist><dict></dict></plist>"), None);
  let other = plist.replace("<string>serve</string>", "<string>serve</string>\n    <string>--bind</string>");
  assert_eq!(PlistValues::parse(&other), None, "another argument list");
}

#[cfg(target_os = "macos")]
#[test]
fn only_the_old_setup_plist_is_eligible_for_migration() {
  let dir = tempfile::tempdir().expect("a temporary home");
  let home = Home::new(dir.path().join("home"));
  let legacy = plist_text_with_label(LEGACY_LAUNCHD_LABEL, &home.legacy_binary_file(), 7742, &home.legacy_launchd_log_file(), "/usr/bin");
  assert!(legacy_launch_agent_matches(&home, &legacy));
  assert!(!legacy_launch_agent_matches(&home, &legacy.replace("com.jetbrains.bd-host", "com.example.other")));
  assert!(!legacy_launch_agent_matches(
    &home,
    &legacy.replace("<true/>\n  <key>KeepAlive", "<false/>\n  <key>KeepAlive")
  ));
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
  // nothing listens again, so only the end of the wait ends it: the refusal is its word
  let started = Instant::now();
  let refused = wait_for_listen(port, Duration::from_millis(300)).expect_err("nothing listens");
  assert!(refused.contains("nothing listens on 127.0.0.1"), "{refused}");
  assert!(started.elapsed() >= Duration::from_millis(300), "{:?}", started.elapsed());
}

fn fake_launchctl_with_script(dir: &Path, script: &str) -> (std::path::PathBuf, std::path::PathBuf) {
  let calls = dir.join("launchctl-calls");
  let program = dir.join("launchctl");
  fs::write(&program, format!("#!/bin/sh\necho \"$*\" >>'{}'\n{script}\n", calls.display())).expect("the fake");
  fs::set_permissions(&program, std::os::unix::fs::PermissionsExt::from_mode(0o755)).expect("executable");
  (program, calls)
}

/// A fake launchctl that appends its arguments to `calls`: `print` answers that no service is loaded
fn fake_launchctl(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
  fake_launchctl_with_script(dir, "[ \"$1\" != print ]")
}

#[cfg(target_os = "macos")]
fn fake_launchctl_with_legacy_loaded(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
  let unloaded = dir.join("legacy-unloaded");
  let script = format!(
    "if [ \"$1\" = print ]; then\n  case \"$2\" in */{LEGACY_LAUNCHD_LABEL}) [ ! -e '{}' ];; *) exit 1;; esac\n  exit $?\nelif [ \"$1\" = bootout ]; then\n  touch '{}'\nfi\nexit 0",
    unloaded.display(),
    unloaded.display()
  );
  fake_launchctl_with_script(dir, &script)
}

#[cfg(target_os = "macos")]
fn migration_setup<'a>(exe: &'a Path, launchctl: &'a Path) -> Setup<'a> {
  Setup {
    client: Identity::generate().expect("a client").fingerprint(),
    name: "laptop",
    port: 7742,
    exe,
    launchctl,
    path: "/usr/bin",
    listen_wait: Duration::ZERO,
  }
}

#[cfg(target_os = "macos")]
fn legacy_plist(home: &Home) -> String {
  plist_text_with_label(LEGACY_LAUNCHD_LABEL, &home.legacy_binary_file(), 7742, &home.legacy_launchd_log_file(), "/usr/bin")
}

#[cfg(target_os = "macos")]
fn write_legacy_plist(home: &Home, text: &str) -> PathBuf {
  let file = home.legacy_launch_agent_file();
  fs::create_dir_all(file.parent().unwrap()).expect("the launch agent directory");
  fs::write(&file, text).expect("the legacy launch agent");
  file
}

#[cfg(target_os = "macos")]
fn migration_executable(dir: &Path) -> PathBuf {
  let exe = dir.join("brd-host");
  fs::write(&exe, "the build of the setup").expect("the executable");
  exe
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
  let exe = dir.path().join("brd-host");
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

#[cfg(target_os = "macos")]
#[test]
fn setup_removes_the_legacy_launch_agent_before_starting_the_new_one() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let home = Home::new(dir.path().join("home"));
  let legacy_file = write_legacy_plist(&home, &legacy_plist(&home));
  let (launchctl, calls) = fake_launchctl_with_legacy_loaded(dir.path());
  let exe = migration_executable(dir.path());
  let setup = migration_setup(&exe, &launchctl);

  setup_agent(&home, &setup, &mut Vec::new()).expect("set up");
  assert!(!legacy_file.exists(), "the old plist is removed");
  let (_, old_service) = service(LEGACY_LAUNCHD_LABEL);
  let (domain, new_service) = service(LAUNCHD_LABEL);
  assert_eq!(
    read(&calls),
    format!(
      "print {old_service}\nbootout {old_service}\nprint {old_service}\nprint {new_service}\nbootstrap {domain} {}\n",
      home.launch_agent_file().display()
    )
  );
}

#[cfg(target_os = "macos")]
#[test]
fn setup_refuses_to_remove_an_unrecognized_legacy_launch_agent() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let home = Home::new(dir.path().join("home"));
  let legacy = legacy_plist(&home).replace("com.jetbrains.bd-host", "com.example.other");
  let legacy_file = write_legacy_plist(&home, &legacy);
  let (launchctl, calls) = fake_launchctl(dir.path());
  let exe = migration_executable(dir.path());
  let setup = migration_setup(&exe, &launchctl);

  let refused = setup_agent(&home, &setup, &mut Vec::new()).expect_err("an unrecognized plist");
  assert_eq!(refused.code, Code::LaunchdFailed);
  assert_eq!(read(&legacy_file), legacy);
  assert_eq!(read(&calls), "");
}

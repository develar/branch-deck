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

#[test]
fn import_yonder_takes_the_identities_the_hosts_and_the_allow_list() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let home = Home::new(dir.path().join("home"));
  let (launchctl, _) = fake_launchctl(dir.path());
  let exe = dir.path().join("bd-host");
  fs::write(&exe, "the build of the import").expect("the executable");
  let import = Import {
    exe: &exe,
    launchctl: &launchctl,
    path: "/usr/bin",
    listen_wait: Duration::ZERO,
  };
  assert_eq!(import_yonder(&home, &import, &mut Vec::new()).expect_err("no yonder").code, Code::NoYonder);

  let yonder_client = home.dir().join(identity::yonder::CLIENT_DIR);
  let yonder_agent = home.dir().join(identity::yonder::AGENT_DIR);
  let client = Identity::load_or_create(&yonder_client).expect("the client of yonder");
  let agent = Identity::load_or_create(&yonder_agent).expect("the agent of yonder");
  private_dir(&yonder_client.join("hosts")).expect("the hosts");
  fs::write(
    yonder_client.join("hosts/studio.json"),
    r#"{"alias":"studio","port":7741,"hostname":"studio.local","addresses":["192.168.1.2:7741","[fe80::1%15]:7741"]}"#,
  )
  .expect("a host file");
  let mut theirs = AllowList::default();
  let laptop = Identity::generate().expect("a laptop").fingerprint();
  theirs.add(laptop, "laptop").expect("a name");
  theirs.write(&yonder_agent.join(allow::CLIENTS_FILE)).expect("the allow list of yonderd");

  let mut out = Vec::new();
  import_yonder(&home, &import, &mut out).expect("imported");
  let launchd = if cfg!(target_os = "macos") { "bootstrapped" } else { "none" };
  assert_eq!(
    String::from_utf8(out).expect("UTF-8"),
    format!("{{\"client_identity\":\"copied\",\"hosts\":[\"studio\"],\"agent_identity\":\"copied\",\"clients\":1,\"launchd\":\"{launchd}\"}}\n")
  );
  assert_eq!(Identity::load(&home.client_dir()).expect("the client").fingerprint(), client.fingerprint());
  assert_eq!(Identity::load(&home.identity_dir()).expect("the agent").fingerprint(), agent.fingerprint());
  let host: serde_json::Value = serde_json::from_str(&read(&home.client_dir().join("hosts/studio.json"))).expect("the host file");
  assert_eq!((host["port"].as_u64(), host["hostname"].as_str()), (Some(u64::from(DEFAULT_PORT)), Some("studio.local")));
  assert_eq!(
    host["addresses"],
    serde_json::json!([format!("192.168.1.2:{DEFAULT_PORT}"), format!("[fe80::1%15]:{DEFAULT_PORT}")]),
    "the cached addresses get the port of bd-host, or the client reaches yonderd"
  );
  assert_eq!(AllowList::read(&home.allow_file()).expect("the allow list").name(&laptop), Some("laptop"));

  let mut again = Vec::new();
  import_yonder(&home, &import, &mut again).expect("imported again");
  assert!(
    String::from_utf8(again)
      .expect("UTF-8")
      .starts_with(r#"{"client_identity":"same","hosts":[],"agent_identity":"same","clients":0,"#)
  );
  assert!(yonder_client.join("identity.key").exists(), "yonder keeps working");
}

#[test]
fn retire_yonderd_waits_for_its_runs_and_removes_its_refs_and_files() {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let home = Home::new(dir.path().join("home"));
  let (launchctl, calls) = fake_launchctl(dir.path());
  let repo = test_utils::git_test_utils::TestRepo::new();
  let head = repo.create_commit("first", "a.txt", "a");
  repo.git(&["update-ref", "refs/yonder/1-1", &head]);
  repo.git(&["update-ref", "refs/yonder/base/checkout-1", &head]);
  let cache = home.dir().join(yonder::CACHE_DIR);
  let run = cache.join("runs/1-1");
  private_dir(&run).expect("a run of yonderd");
  fs::write(cache.join("repos"), format!("{}\n", repo.path().display())).expect("the repository list");
  private_dir(&home.dir().join(identity::yonder::AGENT_DIR)).expect("the identity of yonderd");
  let plist = home.dir().join(format!("Library/LaunchAgents/{}.plist", yonder::LAUNCHD_LABEL));
  fs::create_dir_all(plist.parent().expect("the LaunchAgents directory")).expect("the LaunchAgents directory");
  fs::write(&plist, "<plist/>").expect("the plist of yonderd");

  let lock = crate::files::open_private(&run.join("lock")).expect("the lock of the run");
  assert!(crate::files::try_flock(&lock).expect("the flock"));
  let waited = wait_for_yonderd_runs(&cache.join("runs"), Duration::from_millis(300)).expect_err("a run goes on");
  assert!(waited.message.contains("1-1"), "{}", waited.message);

  // yonderd goes before the wait for its runs, so no run of it starts during the wait
  let mut out = Vec::new();
  std::thread::scope(|scope| {
    let retiring = scope.spawn(|| retire_yonderd(&home, &launchctl, &mut out));
    if cfg!(target_os = "macos") {
      let started = Instant::now();
      while !read(&calls).contains("bootout") {
        assert!(started.elapsed() < Duration::from_secs(60), "no bootout during the wait");
        std::thread::sleep(Duration::from_millis(20));
      }
    }
    assert!(run.exists(), "the run directory stays while its run goes on");
    drop(lock);
    retiring.join().expect("the retire thread").expect("retired");
  });
  let line: serde_json::Value = serde_json::from_slice(&out).expect("one JSON line");
  assert_eq!(line["deleted_refs"], 2);
  assert_eq!(line["removed"], serde_json::json!([plist.display().to_string(), cache.display().to_string()]));
  assert!(!plist.exists(), "launchd does not start yonderd again at the next login");
  assert_eq!(line["kept"], serde_json::json!([home.dir().join(identity::yonder::AGENT_DIR).display().to_string()]));
  assert_eq!(repo.git(&["for-each-ref", "refs/yonder/"]).trim(), "");
  assert!(!cache.exists());
  let booted_out = cfg!(target_os = "macos");
  assert_eq!(line["booted_out"], booted_out);
  assert_eq!(read(&calls).contains("bootout"), booted_out);
}

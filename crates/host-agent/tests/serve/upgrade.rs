//! The upgrade of an agent under launchd: a refused binary leaves no file, a changed `LaunchAgent` stays with `setup`,
//! and an accepted binary ends `serve` while a run streams on, and the watchdog sees the next `serve`

use super::{AGENT_BINARY, Agent, World, eventually, wait_for, write_script};
use host_link::proto::{Reply, Request, Stream};
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::thread;
use std::time::{Duration, Instant};
use sync_utils::fs_retry;

/// The fake `launchctl` of `setup` and of the watchdog: it writes each call to `$FAKE_LAUNCHCTL_DIR/calls`, and `print`
/// answers whether `bootstrap` loaded the service
const FAKE_LAUNCHCTL: &str = r#"#!/bin/sh
echo "$*" >>"$FAKE_LAUNCHCTL_DIR/calls"
case $1 in
  print) [ -e "$FAKE_LAUNCHCTL_DIR/loaded" ] ;;
  bootstrap) : >"$FAKE_LAUNCHCTL_DIR/loaded" ;;
  bootout) rm -f "$FAKE_LAUNCHCTL_DIR/loaded" ;;
  kickstart) exit 0 ;;
  *) exit 2 ;;
esac
"#;

/// The `LaunchAgent` that `setup --agent` wrote with a fake launchctl: its binary and its port
struct Launched {
  target: PathBuf,
  port: u16,
  launchctl: PathBuf,
  launchctl_dir: PathBuf,
}

impl Launched {
  /// Runs `setup --agent` of the test binary on a free port
  fn setup(world: &World) -> Self {
    let launchctl = world.home.path().join("fake/launchctl");
    let launchctl_dir = world.home.path().join("fake/launchctl.d");
    fs::create_dir_all(&launchctl_dir).expect("the launchctl directory");
    write_script(&launchctl, FAKE_LAUNCHCTL);
    let port = std::net::TcpListener::bind("127.0.0.1:0")
      .and_then(|listener| listener.local_addr())
      .expect("a free port")
      .port();
    let output = Command::new(AGENT_BINARY)
      .env("HOME", world.home.path())
      .env("FAKE_LAUNCHCTL_DIR", &launchctl_dir)
      .args(["setup", "--agent", "--client", &world.client.fingerprint().to_string(), "--name", "test"])
      .args(["--port", &port.to_string(), "--listen-wait-ms", "0", "--launchctl"])
      .arg(&launchctl)
      .output()
      .expect("setup");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let _ = fs_retry::remove_file(launchctl_dir.join("calls"));
    let plist = fs::read_to_string(world.plist()).expect("the plist");
    let target = plist
      .split_once("<key>ProgramArguments</key>")
      .and_then(|(_, rest)| rest.split_once("<string>"))
      .and_then(|(_, rest)| rest.split_once("</string>"))
      .map_or_else(|| panic!("no binary in {plist}"), |(exe, _)| PathBuf::from(exe));
    Self {
      target,
      port,
      launchctl,
      launchctl_dir,
    }
  }

  fn calls(&self) -> String {
    fs::read_to_string(self.launchctl_dir.join("calls")).unwrap_or_default()
  }

  /// Starts `serve` of the binary of the `LaunchAgent` on its port, with no drain and a watchdog that waits 30 s
  fn serve<'w>(&self, world: &'w World) -> Agent<'w> {
    let launchctl = self.launchctl.to_str().expect("a text path");
    let launchctl_dir = self.launchctl_dir.to_str().expect("a text path");
    let env = [
      ("BD_HOST_TEST_UPGRADE_DRAIN_MS", "0"),
      ("BD_HOST_TEST_UPGRADE_WAIT_MS", "30000"),
      ("FAKE_LAUNCHCTL_DIR", launchctl_dir),
    ];
    world.serve_program(&self.target, self.port, &["--launchctl", launchctl], &env)
  }
}

impl World {
  fn plist(&self) -> PathBuf {
    self.home.path().join("Library/LaunchAgents/com.jetbrains.bd-host.plist")
  }
}

impl Agent<'_> {
  /// Waits up to 30 s for the end of `serve`
  fn wait_for_exit(&mut self) -> ExitStatus {
    let started = Instant::now();
    loop {
      if let Some(status) = self.child.try_wait().expect("the status of serve") {
        return status;
      }
      assert!(started.elapsed() < Duration::from_secs(30), "serve did not exit in 30 s");
      thread::sleep(Duration::from_millis(50));
    }
  }

  /// Sends `binary` as an upgrade of `size` bytes with `sha256`, and answers the last reply
  fn upgrade(&self, binary: &[u8], size: u64, sha256: &str) -> Reply {
    let mut channel = self.ask(&Request::Upgrade {
      size,
      sha256: sha256.to_owned(),
      force: false,
    });
    channel.send_stream(&mut &binary[..], &mut |_| {}).expect("the binary");
    loop {
      match channel.recv().expect("the reply") {
        Reply::Heartbeat { .. } => {}
        reply => return reply,
      }
    }
  }
}

/// The names in `dir`, sorted
fn names(dir: &Path) -> Vec<String> {
  let mut names: Vec<String> = fs::read_dir(dir)
    .expect("the directory")
    .map(|entry| entry.expect("an entry").file_name().to_string_lossy().into_owned())
    .collect();
  names.sort();
  names
}

#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "only an agent under launchd upgrades itself")]
fn an_upgrade_swaps_the_binary_and_ends_serve_and_a_run_and_the_watchdog_see_the_next_serve() {
  let world = World::new(1, &[]);
  let launched = Launched::setup(&world);
  let target = launched.target.clone();
  let agent_dir = target.parent().expect("a parent").to_path_buf();
  let file_name = target.file_name().expect("a name").to_string_lossy().into_owned();
  let mut agent = launched.serve(&world);
  let binary = fs::read(&target).expect("the agent binary");
  let size = u64::try_from(binary.len()).expect("a size");
  let sha256 = host_link::digest::sha256_file(&target).expect("the SHA-256");

  // a refused binary leaves no file: another SHA-256, and fewer bytes than the request names
  let refused = agent.upgrade(b"short", 5, &"0".repeat(64));
  assert!(matches!(&refused, Reply::Refused { code, .. } if code == "bad_binary"), "{refused:?}");
  let refused = agent.upgrade(b"short", size, &sha256);
  assert!(matches!(&refused, Reply::Refused { code, .. } if code == "bad_binary"), "{refused:?}");
  assert_eq!(names(&agent_dir), std::slice::from_ref(&file_name));

  // a LaunchAgent that the new binary would write in another way stays with `setup`
  let plist = fs::read_to_string(world.plist()).expect("the plist");
  let changed = plist.replace("<key>RunAtLoad</key>\n  <true/>", "<key>RunAtLoad</key>\n  <false/>");
  assert_ne!(changed, plist);
  fs::write(world.plist(), changed).expect("another plist");
  let refused = agent.upgrade(&binary, size, &sha256);
  assert!(matches!(&refused, Reply::Refused { code, .. } if code == "plist_changed"), "{refused:?}");
  fs::write(world.plist(), &plist).expect("the plist of setup");
  assert_eq!(names(&agent_dir), std::slice::from_ref(&file_name));

  // the run streams through the upgrade: its follow breaks with the old serve and goes on with the new one
  let gate = world.home.path().join("gate");
  agent.start(world.spec("1-upgrade", &format!("echo before; {}; echo after", wait_for(&gate))));
  let mut streaming = agent.ask(&Request::Follow {
    run: "1-upgrade".to_owned(),
    offset: 0,
  });
  let offset = loop {
    match streaming.recv::<Reply>().expect("a reply of the follow") {
      Reply::Output { text, end, .. } if text == "before" => break end,
      Reply::Output { .. } | Reply::Heartbeat { .. } => {}
      end => panic!("the run ended before its first line: {end:?}"),
    }
  };

  let before = fs::metadata(&target).expect("the binary").ino();
  let upgraded = agent.upgrade(&binary, size, &sha256);
  assert_eq!(
    upgraded,
    Reply::Upgraded {
      previous_sha256: sha256.clone(),
      sha256: sha256.clone(),
      version: env!("CARGO_PKG_VERSION").to_owned(),
    }
  );
  assert!(agent.wait_for_exit().success(), "{}", agent.dump("1-upgrade"));
  let broken = loop {
    match streaming.recv::<Reply>() {
      Ok(Reply::Heartbeat { .. }) => {}
      Ok(reply) => panic!("the old serve answered after its exit: {reply:?}"),
      Err(error) => break error,
    }
  };
  drop(streaming);
  assert_eq!(names(&agent_dir), [file_name.clone(), format!("{file_name}.previous")], "{broken}");
  assert_eq!(fs::metadata(agent_dir.join(format!("{file_name}.previous"))).expect("the previous binary").ino(), before);
  assert_ne!(fs::metadata(&target).expect("the binary").ino(), before);
  assert!(!world.home.path().join(".cache/bd-host/listening.json").exists());

  // launchd would start the binary again
  let renewed = launched.serve(&world);
  let short = &sha256[..12];
  let agent_log = world.home.path().join("Library/Logs/bd-host.log");
  eventually("the watchdog sees the new serve", || {
    fs::read_to_string(&agent_log).is_ok_and(|log| log.contains(&format!("watchdog: the binary {short} listens with the pid")))
  });
  // the watchdog writes its log line before its outcome
  let outcome_file = world.home.path().join(".cache/bd-host/upgrade.json");
  eventually("the outcome of the watchdog", || outcome_file.exists());
  let outcome: Value = serde_json::from_slice(&fs::read(&outcome_file).expect("the outcome")).expect("JSON");
  assert_eq!(outcome["outcome"], "listened");
  fs::write(&gate, "").expect("the gate");
  let followed = renewed.follow_from("1-upgrade", offset);
  assert_eq!(followed.end, Reply::Exit { code: 0 }, "{followed:?}");
  assert_eq!(followed.of(Stream::Stdout), ["after"]);
  assert_eq!(launched.calls(), "");
}

use crate::discovery::{HostedInstance, Instance, Platform, ask_hosts, find_instance, instance_dir_of, pick, read_instances};
use crate::fake_ide_test::{FakeIde, Step, exchange, refusal, result};
use pretty_assertions::assert_eq;
use serde_json::json;
use std::ffi::OsString;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The absolute path of `path` on this platform: `/work/idea` stays on Unix, becomes `C:\work\idea` on Windows
fn absolute(path: &str) -> PathBuf {
  if cfg!(windows) {
    PathBuf::from(format!("C:{}", path.replace('/', "\\")))
  } else {
    PathBuf::from(path)
  }
}

/// The instance file of the IDE process `pid` with its socket at `socket`, written last at `modified_at` (Unix seconds)
fn write_instance(dir: &Path, name: &str, pid: u32, socket: &Path, modified_at: u64) -> PathBuf {
  let file = dir.join(format!("{name}.json"));
  let text = json!({"schema": 2, "pid": pid, "product": "IU", "build": "IU-263.1", "socket": socket});
  std::fs::write(&file, text.to_string()).unwrap();
  File::options()
    .write(true)
    .open(&file)
    .unwrap()
    .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(modified_at))
    .unwrap();
  file
}

fn instance(pid: u32, socket: &Path) -> Instance {
  Instance {
    file: PathBuf::from(format!("{pid}.json")),
    pid,
    product: "IU".to_owned(),
    build: "IU-263.1".to_owned(),
    socket: socket.to_path_buf(),
    modified: SystemTime::UNIX_EPOCH,
  }
}

/// The IDE `pid` that hosts `hosts`, whose file the IDE wrote at `modified_at` (Unix seconds) and whose window got the
/// focus at `focused_at` (Unix milliseconds)
fn hosted(pid: u32, hosts: &[&str], modified_at: u64, focused_at: Option<u64>) -> HostedInstance {
  HostedInstance {
    instance: Instance {
      modified: SystemTime::UNIX_EPOCH + Duration::from_secs(modified_at),
      ..instance(pid, &PathBuf::from(format!("/tmp/air-{pid}/ahp.sock")))
    },
    hosts: hosts.iter().map(|host| absolute(host)).collect(),
    focused_at,
  }
}

/// A connection that answers `x-air-ij/hosts` with `hosts` and the last focus `focused_at`
fn hosts_answer(hosts: &[&str], focused_at: Option<u64>) -> Vec<Step> {
  let hosts: Vec<_> = hosts.iter().map(|host| json!({"projectDirectory": absolute(host)})).collect();
  exchange(
    "x-air-ij/hosts",
    &json!({}),
    result(2, &json!({"product": "IU", "build": "271.1", "hosts": hosts, "focusedAt": focused_at})),
  )
}

fn pids(instances: &[Instance]) -> Vec<u32> {
  let mut pids: Vec<u32> = instances.iter().map(|instance| instance.pid).collect();
  pids.sort_unstable();
  pids
}

#[test]
fn reads_the_files_of_running_processes_and_skips_dead_garbled_and_later_ones() {
  let dir = tempfile::tempdir().unwrap();
  write_instance(dir.path(), "10", 10, Path::new("/tmp/air-10/ahp.sock"), 1_000);
  write_instance(dir.path(), "11", 11, Path::new("/tmp/air-11/ahp.sock"), 1_000);
  std::fs::write(dir.path().join("12.json"), "{not json").unwrap();
  std::fs::write(dir.path().join("13.json"), json!({"schema": 3, "pid": 13, "socket": "/s"}).to_string()).unwrap();
  std::fs::write(dir.path().join("14.json"), json!({"schema": 2, "pid": 14}).to_string()).unwrap();
  // a file of schema 1, whose key is `schemaVersion`
  std::fs::write(dir.path().join("15.json"), json!({"schemaVersion": 1, "pid": 15, "socket": "/s"}).to_string()).unwrap();
  std::fs::write(dir.path().join("notes.txt"), "not an instance").unwrap();

  let instances = read_instances(dir.path(), |pid| pid != 11);
  assert_eq!(pids(&instances), vec![10]);
  let instance = &instances[0];
  assert_eq!(instance.product, "IU");
  assert_eq!(instance.build, "IU-263.1");
  assert_eq!(instance.socket, PathBuf::from("/tmp/air-10/ahp.sock"));
  assert_eq!(instance.modified, SystemTime::UNIX_EPOCH + Duration::from_secs(1_000));
}

#[test]
fn the_directory_of_the_instance_files_is_the_common_data_directory_of_the_ide_on_each_platform() {
  let home = absolute("/home/u");
  let dir = |platform: Platform, vars: &[(&str, &str)]| {
    let vars: Vec<(String, OsString)> = vars.iter().map(|(name, value)| ((*name).to_owned(), OsString::from(value))).collect();
    instance_dir_of(platform, Some(&home), |name| vars.iter().find(|(it, _)| it == name).map(|(_, value)| value.clone()))
  };
  let leaf = |base: PathBuf| Some(base.join("JetBrains").join("air").join("ahp"));
  let data = absolute("/data");
  let roaming = absolute("/roaming");
  assert_eq!(dir(Platform::MacOs, &[("XDG_DATA_HOME", "/data")]), leaf(home.join("Library").join("Application Support")));
  assert_eq!(dir(Platform::Windows, &[("APPDATA", roaming.to_str().unwrap())]), leaf(roaming.clone()));
  // an empty variable is unset, as PathManager of the IDE reads it
  assert_eq!(dir(Platform::Windows, &[("APPDATA", "")]), leaf(home.join("AppData").join("Roaming")));
  assert_eq!(dir(Platform::Xdg, &[("XDG_DATA_HOME", data.to_str().unwrap())]), leaf(data.clone()));
  assert_eq!(dir(Platform::Xdg, &[("XDG_DATA_HOME", "")]), leaf(home.join(".local").join("share")));
  assert_eq!(dir(Platform::Xdg, &[("XDG_DATA_HOME", "relative")]), leaf(home.join(".local").join("share")));
  assert_eq!(instance_dir_of(Platform::Xdg, None, |_| None), None);
}

#[test]
fn no_directory_is_no_instance() {
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(read_instances(&dir.path().join("missing"), |_| true), Vec::new());
}

#[test]
fn each_ide_names_its_hosts_and_one_that_refuses_fails_or_does_not_answer_in_time_is_skipped() {
  let answers = FakeIde::serve(vec![hosts_answer(&["/work/idea", "/work/other"], Some(5_000))]);
  let refuses = FakeIde::serve(vec![exchange("x-air-ij/hosts", &json!({}), refusal(2, "RATE_LIMITED", "Too many"))]);
  let silent = FakeIde::serve(vec![vec![Step::Read(crate::fake_ide_test::initialize())]]);
  let dir = tempfile::tempdir().unwrap();
  let instances = vec![
    instance(70, &answers.socket),
    instance(71, &refuses.socket),
    instance(72, &silent.socket),
    instance(73, &dir.path().join("gone.sock")),
  ];

  let hosted = ask_hosts(instances, Duration::from_millis(200));
  assert_eq!(
    hosted,
    vec![HostedInstance {
      instance: instance(70, &answers.socket),
      hosts: vec![absolute("/work/idea"), absolute("/work/other")],
      focused_at: Some(5_000),
    }]
  );
  answers.finish();
  refuses.finish();
  silent.finish();
}

#[test]
fn the_ide_of_a_directory_has_it_or_a_parent_open_and_the_deepest_project_wins() {
  let instances = [hosted(20, &["/work"], 3_000, None), hosted(21, &["/elsewhere", "/work/idea"], 1_000, None)];

  let found = pick(&instances, &absolute("/work/idea/community")).unwrap();
  assert_eq!((found.instance.instance.pid, found.host), (21, absolute("/work/idea").as_path()));
  let found = pick(&instances, &absolute("/work/idea")).unwrap();
  assert_eq!(found.instance.instance.pid, 21);
  // a parent of the directory, not a sibling with the same start of its name
  let found = pick(&instances, &absolute("/work/idea-2")).unwrap();
  assert_eq!((found.instance.instance.pid, found.host), (20, absolute("/work").as_path()));
  assert_eq!(pick(&instances, &absolute("/other")), None);
  // a host that is not absolute holds nothing
  let relative = HostedInstance {
    hosts: vec![PathBuf::from("work")],
    ..hosted(22, &[], 1_000, None)
  };
  assert_eq!(pick(&[relative], Path::new("work/idea")), None);
}

#[test]
fn of_two_ides_with_the_same_project_the_newest_file_wins() {
  let instances = [
    hosted(30, &["/work/idea"], 1_000, None),
    hosted(31, &["/work/idea"], 2_000, None),
    hosted(32, &["/work/idea"], 1_500, None),
  ];
  assert_eq!(pick(&instances, &absolute("/work/idea")).unwrap().instance.instance.pid, 31);
}

#[test]
fn of_two_ides_with_the_same_project_the_last_focused_wins_and_one_without_a_focus_is_older_than_any() {
  let instances = [
    // the newest file, but no focus
    hosted(50, &["/work/idea"], 3_000, None),
    hosted(51, &["/work/idea"], 1_000, Some(2_000_000)),
    hosted(52, &["/work/idea"], 2_000, Some(1_000_000)),
  ];
  let found = pick(&instances, &absolute("/work/idea")).unwrap();
  assert_eq!((found.instance.instance.pid, found.instance.focused_at), (51, Some(2_000_000)));
  // the same focus: the newest file
  let instances = [hosted(51, &["/work/idea"], 1_000, Some(2_000_000)), hosted(52, &["/work/idea"], 2_000, Some(2_000_000))];
  assert_eq!(pick(&instances, &absolute("/work/idea")).unwrap().instance.instance.pid, 52);
}

#[test]
fn the_deepest_project_wins_over_the_last_focus() {
  let instances = [hosted(60, &["/work"], 1_000, Some(9_000_000)), hosted(61, &["/work/idea"], 1_000, None)];
  assert_eq!(pick(&instances, &absolute("/work/idea/community")).unwrap().instance.instance.pid, 61);
}

#[test]
fn a_dead_ide_is_never_picked_though_its_file_is_the_newest() {
  let dir = tempfile::tempdir().unwrap();
  // this process runs; no process has a pid past the limit of every platform
  let alive = FakeIde::serve(vec![hosts_answer(&["/work/idea"], None)]);
  write_instance(dir.path(), "alive", std::process::id(), &alive.socket, 1_000);
  let dead = FakeIde::serve(vec![hosts_answer(&["/work/idea/community"], Some(9_000_000))]);
  write_instance(dir.path(), "dead", 999_999_999, &dead.socket, 2_000);

  let (instance, host) = find_instance(dir.path(), &absolute("/work/idea/community")).unwrap();
  assert_eq!((instance.pid, host), (std::process::id(), absolute("/work/idea")));
  assert_eq!(alive.finish().len(), 3, "the opener, the envelope and x-air-ij/hosts");
  // the dead IDE is not asked: its socket would name the deeper host
  drop(dead);
}

#[test]
fn find_instance_checks_the_processes_of_the_files() {
  let dir = tempfile::tempdir().unwrap();
  let ide = FakeIde::serve(vec![hosts_answer(&["/work/idea"], None)]);
  write_instance(dir.path(), "ide", std::process::id(), &ide.socket, 1_000);
  // the file names a socket nobody listens on: the IDE is skipped
  write_instance(dir.path(), "gone", std::process::id(), &dir.path().join("gone.sock"), 2_000);
  let (instance, host) = find_instance(dir.path(), &absolute("/work/idea/community")).unwrap();
  assert_eq!((instance.pid, instance.socket, host), (std::process::id(), ide.socket.clone(), absolute("/work/idea")));
  ide.finish();
}

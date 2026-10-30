use crate::discovery::{Instance, find_instance, pick, read_instances};
use pretty_assertions::assert_eq;
use serde_json::json;
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

fn write_instance(dir: &Path, pid: u32, projects: &[&str], modified_at: u64) -> PathBuf {
  let file = dir.join(format!("{pid}.json"));
  let projects: Vec<PathBuf> = projects.iter().map(|project| absolute(project)).collect();
  let text = json!({"schemaVersion": 1, "pid": pid, "product": "IU", "build": "IU-263.1", "socket": format!("/tmp/air-{pid}/launch.sock"), "projects": projects});
  std::fs::write(&file, text.to_string()).unwrap();
  set_modified(&file, modified_at);
  file
}

fn set_modified(file: &Path, seconds: u64) {
  File::options()
    .write(true)
    .open(file)
    .unwrap()
    .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
    .unwrap();
}

fn pids(instances: &[Instance]) -> Vec<u32> {
  let mut pids: Vec<u32> = instances.iter().map(|instance| instance.pid).collect();
  pids.sort_unstable();
  pids
}

#[test]
fn reads_the_files_of_running_processes_and_skips_dead_garbled_and_later_ones() {
  let dir = tempfile::tempdir().unwrap();
  write_instance(dir.path(), 10, &["/work/idea"], 1_000);
  write_instance(dir.path(), 11, &["/work/other"], 1_000);
  std::fs::write(dir.path().join("12.json"), "{not json").unwrap();
  std::fs::write(
    dir.path().join("13.json"),
    json!({"schemaVersion": 2, "pid": 13, "socket": "/s", "projects": []}).to_string(),
  )
  .unwrap();
  std::fs::write(dir.path().join("14.json"), json!({"schemaVersion": 1, "pid": 14}).to_string()).unwrap();
  std::fs::write(dir.path().join("notes.txt"), "not an instance").unwrap();

  let instances = read_instances(dir.path(), |pid| pid != 11);
  assert_eq!(pids(&instances), vec![10]);
  let instance = &instances[0];
  assert_eq!(instance.product, "IU");
  assert_eq!(instance.build, "IU-263.1");
  assert_eq!(instance.socket, PathBuf::from("/tmp/air-10/launch.sock"));
  assert_eq!(instance.projects, vec![absolute("/work/idea")]);
  assert_eq!(instance.modified, SystemTime::UNIX_EPOCH + Duration::from_secs(1_000));
}

#[test]
fn no_directory_is_no_instance() {
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(read_instances(&dir.path().join("missing"), |_| true), Vec::new());
}

#[test]
fn the_ide_of_a_directory_has_it_or_a_parent_open_and_the_deepest_project_wins() {
  let dir = tempfile::tempdir().unwrap();
  write_instance(dir.path(), 20, &["/work"], 3_000);
  write_instance(dir.path(), 21, &["/elsewhere", "/work/idea"], 1_000);
  let instances = read_instances(dir.path(), |_| true);

  let found = pick(&instances, &absolute("/work/idea/community")).unwrap();
  assert_eq!((found.instance.pid, found.project), (21, absolute("/work/idea").as_path()));
  let found = pick(&instances, &absolute("/work/idea")).unwrap();
  assert_eq!(found.instance.pid, 21);
  // a parent of the directory, not a sibling with the same start of its name
  let found = pick(&instances, &absolute("/work/idea-2")).unwrap();
  assert_eq!((found.instance.pid, found.project), (20, absolute("/work").as_path()));
  assert_eq!(pick(&instances, &absolute("/other")), None);
}

#[test]
fn of_two_ides_with_the_same_project_the_newest_file_wins() {
  let dir = tempfile::tempdir().unwrap();
  write_instance(dir.path(), 30, &["/work/idea"], 1_000);
  write_instance(dir.path(), 31, &["/work/idea"], 2_000);
  write_instance(dir.path(), 32, &["/work/idea"], 1_500);
  let instances = read_instances(dir.path(), |_| true);
  assert_eq!(pick(&instances, &absolute("/work/idea")).unwrap().instance.pid, 31);
}

#[test]
fn a_dead_ide_is_never_picked_though_its_file_is_the_newest() {
  let dir = tempfile::tempdir().unwrap();
  write_instance(dir.path(), 40, &["/work/idea"], 1_000);
  write_instance(dir.path(), 41, &["/work/idea"], 2_000);
  let instances = read_instances(dir.path(), |pid| pid == 40);
  assert_eq!(pick(&instances, &absolute("/work/idea")).unwrap().instance.pid, 40);
}

#[test]
fn find_instance_checks_the_processes_of_the_files() {
  let dir = tempfile::tempdir().unwrap();
  // this process runs; no process has a pid past the limit of every platform
  write_instance(dir.path(), std::process::id(), &["/work/idea"], 1_000);
  write_instance(dir.path(), 999_999_999, &["/work/idea/community"], 2_000);
  let (instance, project) = find_instance(dir.path(), &absolute("/work/idea/community")).unwrap();
  assert_eq!((instance.pid, project), (std::process::id(), absolute("/work/idea")));
}

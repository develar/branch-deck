use crate::package_manager::{FoundManager, installed_by, manager_commands, winget_id};
use crate::test_support::fakes;
use domain_types::cli_tools::{CliTool, PackageManager};
use pretty_assertions::assert_eq;
use std::path::PathBuf;

fn args(command: &std::process::Command) -> Vec<String> {
  command.get_args().map(|arg| arg.to_string_lossy().into_owned()).collect()
}

#[test]
fn winget_installs_for_the_user_without_prompts_and_scoop_adds_its_bucket_first() {
  let (_root, _fakes, environment) = fakes();
  let winget = FoundManager {
    manager: PackageManager::Winget,
    binary: PathBuf::from("winget"),
  };
  let install = manager_commands(&environment, &winget, CliTool::Teamcity, false);
  assert_eq!(
    args(&install[0].0),
    [
      "install",
      "--id",
      "JetBrains.TeamCityCLI",
      "--exact",
      "--scope",
      "user",
      "--silent",
      "--disable-interactivity",
      "--accept-source-agreements",
      "--accept-package-agreements"
    ]
  );
  let upgrade = manager_commands(&environment, &winget, CliTool::Teamcity, true);
  assert_eq!(args(&upgrade[0].0)[..4], ["upgrade", "--id", "JetBrains.TeamCityCLI", "--exact"]);
  // youtrack is in no winget source yet
  assert_eq!(winget_id(CliTool::Youtrack), None);

  let scoop = FoundManager {
    manager: PackageManager::Scoop,
    binary: PathBuf::from("scoop"),
  };
  let install = manager_commands(&environment, &scoop, CliTool::Youtrack, false);
  let steps: Vec<(Vec<String>, bool)> = install.iter().map(|(command, must)| (args(command), *must)).collect();
  assert_eq!(
    steps,
    [
      (
        vec!["bucket".into(), "add".into(), "jetbrains".into(), "https://github.com/JetBrains/scoop-utils".into()],
        false
      ),
      (vec!["install".into(), "jetbrains/youtrack".into()], true),
    ]
  );
  assert_eq!(args(&manager_commands(&environment, &scoop, CliTool::Youtrack, true)[0].0), ["update", "youtrack"]);
}

#[test]
fn the_path_of_a_binary_tells_its_manager_when_the_machine_has_it() {
  let (root, fakes, environment) = fakes();
  let in_dir = |dir: &str| {
    let dir = root.path().join(dir);
    std::fs::create_dir_all(&dir).unwrap();
    test_utils::fake_executable::fake_executable(&dir, "youtrack", "true")
  };
  let winget = in_dir("AppData/Local/Microsoft/WinGet/Packages/JetBrains.YouTrackCLI");
  let scoop = in_dir("scoop/apps/youtrack/current");
  let script = in_dir(".local/bin");
  // no manager on the machine: none, whatever the path
  assert_eq!(installed_by(&environment, &winget), None);
  test_utils::fake_executable::fake_executable(&fakes.bin(), "winget", "true");
  test_utils::fake_executable::fake_executable(&fakes.bin(), "scoop", "true");
  assert_eq!(installed_by(&environment, &winget).map(|found| found.manager), Some(PackageManager::Winget));
  assert_eq!(installed_by(&environment, &scoop).map(|found| found.manager), Some(PackageManager::Scoop));
  assert_eq!(installed_by(&environment, &script), None);
}

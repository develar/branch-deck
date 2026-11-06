#[cfg(unix)]
use crate::fixture_test::assert_covered;
#[cfg(unix)]
use crate::fixture_test::{environment, run_json};
use crate::install::refuse_temporary_location;
#[cfg(unix)]
use crate::install::{InstallStatus, SKILL_DIRS, SKILL_NAME, install_status};
use bd_core::bd_command::app_binary_path;
#[cfg(unix)]
use bd_core::bd_command::{BdCommand, bd_command};
use domain_types::error::ErrorCode;
use pretty_assertions::assert_eq;
#[cfg(unix)]
use serde_json::{Value, json};
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use tempfile::TempDir;

/// A home directory and an app binary in it, with `~/.local/bin` on `PATH`
#[cfg(unix)]
fn home_with_app() -> (TempDir, PathBuf) {
  let home = tempfile::tempdir().unwrap();
  let home_path = std::fs::canonicalize(home.path()).unwrap();
  let app = home_path.join("BranchDeck.app/Contents/MacOS/branch-deck");
  std::fs::create_dir_all(app.parent().unwrap()).unwrap();
  std::fs::write(&app, "app").unwrap();
  (home, app)
}

#[cfg(unix)]
fn run_install(home: &Path, app: &Path, args: &[&str]) -> (i32, Value) {
  let mut environment = environment(home);
  environment.home = Some(home.to_path_buf());
  environment.executable = Some(app.to_path_buf());
  environment.path = Some(std::env::join_paths([home.join(".local/bin"), PathBuf::from("/usr/bin")]).unwrap());
  run_json(&environment, args)
}

#[cfg(unix)]
fn legacy_link(home: &Path, name: &str) -> PathBuf {
  let link = home.join(".local/bin").join(name);
  std::fs::create_dir_all(link.parent().unwrap()).unwrap();
  link
}

#[cfg(unix)]
fn assert_legacy_link_removed(output: &Value, link: &Path) {
  assert_eq!(output["removed_legacy"], json!([link.to_str().unwrap()]));
  assert!(std::fs::symlink_metadata(link).is_err(), "the old symlink is removed");
}

#[cfg(unix)]
fn assert_legacy_file_preserved(home: &Path, app: &Path, path: &Path, contents: &str, args: &[&str]) {
  let (exit_code, output) = run_install(home, app, args);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["removed_legacy"], json!([]));
  assert_eq!(std::fs::read_to_string(path).unwrap(), contents);
}

#[cfg(unix)]
fn skill_files(home: &Path) -> Vec<String> {
  SKILL_DIRS
    .iter()
    .map(|dir| home.join(dir).join(SKILL_NAME).join("SKILL.md").to_string_lossy().into_owned())
    .collect()
}

#[cfg(unix)]
#[test_log::test]
fn installs_the_command_and_the_skills_and_uninstalls_them() {
  let (home, app) = home_with_app();
  let home = std::fs::canonicalize(home.path()).unwrap();
  let link = home.join(".local/bin/brd");
  // the agent rules of the empty global config of a test (`environment`): the defaults
  let mut status_environment = environment(&home);
  status_environment.home = Some(home.clone());
  status_environment.executable = Some(app.clone());
  assert_eq!(install_status(&status_environment), InstallStatus { bd: None, skills: Vec::new() });

  let (exit_code, output) = run_install(&home, &app, &["install"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    output,
    json!({
      "schema": 2,
      "brd": link.to_str().unwrap(),
      "on_path": true,
      "removed_legacy": [],
      "skills": skill_files(&home),
      "uninstalled": false,
      "bd_host": null,
      "agents": { "lease": true, "subject_branch": true }
    })
  );
  assert_eq!(std::fs::read_link(&link).unwrap(), app);
  assert_eq!(
    install_status(&status_environment),
    InstallStatus {
      bd: Some(link.to_str().unwrap().to_string()),
      skills: skill_files(&home),
    }
  );
  let (_, schema) = run_install(&home, &app, &["schema", "--all"]);
  assert_covered(&schema["outputs"]["install"], &schema["outputs"]["install"], &output, "install");
  let skill = std::fs::read_to_string(&skill_files(&home)[0]).unwrap();
  assert!(skill.starts_with("---\nname: branch-deck\ndescription: "), "{skill}");
  assert!(skill.contains(&format!("brd is `{}`", link.display())), "{skill}");
  assert!(skill.contains(&format!("\nallowed-tools: \"Bash({}:*)\"\n---\n", link.display())), "{skill}");
  assert!(skill.contains("run `brd help agent`"), "{skill}");
  assert!(skill.contains("run `brd help worktrees` first"), "{skill}");
  assert!(skill.contains("Before the first edit or commit in a repository, run `brd workspace`"), "{skill}");
  assert_eq!(std::fs::read_to_string(&skill_files(&home)[1]).unwrap(), skill);
  // `brd help skill` prints the same skill
  let mut out = Vec::new();
  let mut environment = environment(&home);
  environment.home = Some(home.clone());
  environment.executable = Some(link.clone());
  assert_eq!(crate::run_with(["help", "skill"].map(std::ffi::OsString::from), &environment, &mut out), 0);
  assert_eq!(String::from_utf8(out).unwrap(), skill);

  // again, from the link: nothing changes; a symlink elsewhere is replaced
  let (exit_code, output) = run_install(&home, &link, &["install"]);
  assert_eq!((exit_code, &output["brd"]), (0, &json!(link.to_str().unwrap())), "{output}");
  std::fs::remove_file(&link).unwrap();
  std::os::unix::fs::symlink("/old/branch-deck", &link).unwrap();
  let (exit_code, output) = run_install(&home, &app, &["install"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(std::fs::read_link(&link).unwrap(), app);

  let legacy = home.join(".local/bin/bd");
  std::os::unix::fs::symlink(&app, &legacy).unwrap();
  let (exit_code, output) = run_install(&home, &app, &["install", "--uninstall"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["removed_legacy"], json!([legacy.to_str().unwrap()]));
  assert_eq!((&output["skills"], &output["uninstalled"]), (&json!(skill_files(&home)), &json!(true)));
  assert!(std::fs::symlink_metadata(&link).is_err(), "the symlink is removed");
  assert!(std::fs::symlink_metadata(&legacy).is_err(), "the old symlink is removed");
  assert!(!home.join(".claude/skills/branch-deck").exists());
  assert_eq!(install_status(&status_environment).skills, Vec::<String>::new());
  assert!(home.join(".claude/skills").exists(), "only the skill's own directory goes");
}

#[cfg(unix)]
#[test_log::test]
fn a_file_in_the_way_is_refused() {
  let (home, app) = home_with_app();
  let home = std::fs::canonicalize(home.path()).unwrap();
  std::fs::create_dir_all(home.join(".local/bin")).unwrap();
  std::fs::write(home.join(".local/bin/brd"), "another brd").unwrap();

  let (exit_code, output) = run_install(&home, &app, &["install"]);
  assert_eq!((exit_code, &output["error"]["code"]), (4, &json!("install_refused")), "{output}");
  assert_eq!(std::fs::read_to_string(home.join(".local/bin/brd")).unwrap(), "another brd");

  // not on PATH: still installed, and the skill names the full path
  std::fs::remove_file(home.join(".local/bin/brd")).unwrap();
  let mut environment = environment(&home);
  environment.home = Some(home);
  environment.executable = Some(app);
  environment.path = Some("/usr/bin".into());
  let (exit_code, output) = run_json(&environment, &["install"]);
  assert_eq!((exit_code, &output["on_path"]), (0, &json!(false)), "{output}");
}

#[cfg(unix)]
#[test_log::test]
fn install_removes_the_legacy_bd_link_only_when_it_points_at_the_app() {
  let (home, app) = home_with_app();
  let home = std::fs::canonicalize(home.path()).unwrap();
  let legacy = legacy_link(&home, "bd");
  std::os::unix::fs::symlink(&app, &legacy).unwrap();

  let (exit_code, output) = run_install(&home, &app, &["install"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_legacy_link_removed(&output, &legacy);
  assert_eq!(std::fs::read_link(home.join(".local/bin/brd")).unwrap(), app);
}

#[cfg(unix)]
#[test_log::test]
fn install_and_uninstall_leave_a_foreign_bd_file_alone() {
  let (home, app) = home_with_app();
  let home = std::fs::canonicalize(home.path()).unwrap();
  let legacy = legacy_link(&home, "bd");
  std::fs::write(&legacy, "beads").unwrap();

  assert_legacy_file_preserved(&home, &app, &legacy, "beads", &["install"]);
  assert_legacy_file_preserved(&home, &app, &legacy, "beads", &["install", "--uninstall"]);
}

/// On macOS the install links `~/.local/bin/brd-host` to the `brd-host` of the app bundle, keeps a binary there that an
/// upgrade wrote, and removes only its own symlink
#[cfg(target_os = "macos")]
#[test_log::test]
fn install_links_the_brd_host_of_the_app_bundle_and_keeps_a_binary_there() {
  let (home, app) = home_with_app();
  let home = std::fs::canonicalize(home.path()).unwrap();
  let link = home.join(".local/bin/brd-host");
  let (exit_code, output) = run_install(&home, &app, &["install"]);
  assert_eq!((exit_code, &output["bd_host"]), (0, &json!(null)), "an app without brd-host: {output}");
  assert!(std::fs::symlink_metadata(&link).is_err(), "no symlink without a brd-host of the app");

  let brd_host = app.with_file_name("brd-host");
  std::fs::write(&brd_host, "brd-host").unwrap();
  let (exit_code, output) = run_install(&home, &app, &["install"]);
  assert_eq!((exit_code, &output["bd_host"]), (0, &json!(link.to_str().unwrap())), "{output}");
  assert_eq!(std::fs::read_link(&link).unwrap(), brd_host);

  let legacy = legacy_link(&home, "bd-host");
  let legacy_target = app.with_file_name("bd-host");
  std::os::unix::fs::symlink(&legacy_target, &legacy).unwrap();
  let (exit_code, output) = run_install(&home, &app, &["install"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_legacy_link_removed(&output, &legacy);

  let (exit_code, output) = run_install(&home, &app, &["install", "--uninstall"]);
  assert_eq!((exit_code, &output["bd_host"]), (0, &json!(link.to_str().unwrap())), "{output}");
  assert!(std::fs::symlink_metadata(&link).is_err(), "the symlink is removed");

  // a foreign old binary and the binary of an upgrade stay
  std::fs::write(&legacy, "foreign").unwrap();
  assert_legacy_file_preserved(&home, &app, &legacy, "foreign", &["install"]);
  assert_legacy_file_preserved(&home, &app, &legacy, "foreign", &["install", "--uninstall"]);

  std::fs::write(&link, "upgraded").unwrap();
  let (exit_code, output) = run_install(&home, &app, &["install"]);
  assert_eq!((exit_code, &output["bd_host"]), (0, &json!(link.to_str().unwrap())), "{output}");
  let (exit_code, output) = run_install(&home, &app, &["install", "--uninstall"]);
  assert_eq!((exit_code, &output["bd_host"]), (0, &json!(null)), "{output}");
  assert_eq!(std::fs::read_to_string(&link).unwrap(), "upgraded");
}

/// The command of the app's prompts: the installed link when it is this app's, else the app binary with `cli`, which
/// without `cli` would start the app
#[cfg(unix)]
#[test_log::test]
fn the_brd_command_is_the_link_to_this_app_or_the_app_with_cli() {
  let (home, app) = home_with_app();
  let home = std::fs::canonicalize(home.path()).unwrap();
  let mut environment = environment(&home);
  environment.home = Some(home.clone());
  environment.executable = Some(app.clone());

  let command = bd_command(&environment).unwrap();
  assert_eq!(
    command,
    BdCommand {
      program: app.clone(),
      subcommand: Some("cli")
    }
  );
  assert_eq!(command.display(), format!("{} cli", app.display()));
  let mut out = Vec::new();
  assert_eq!(crate::run_with(["help", "skill"].map(std::ffi::OsString::from), &environment, &mut out), 0);
  assert!(String::from_utf8(out).unwrap().contains(&format!("brd is `{} cli`", app.display())));

  // a link to another binary is not this app's brd
  let link = home.join(".local/bin/brd");
  std::fs::create_dir_all(link.parent().unwrap()).unwrap();
  std::os::unix::fs::symlink("/old/branch-deck", &link).unwrap();
  assert_eq!(bd_command(&environment).unwrap().subcommand, Some("cli"));
  std::fs::remove_file(&link).unwrap();
  std::os::unix::fs::symlink(&app, &link).unwrap();
  let command = bd_command(&environment).unwrap();
  assert_eq!(
    command,
    BdCommand {
      program: link.clone(),
      subcommand: None
    }
  );
  assert_eq!(command.for_shell(), link.display().to_string());

  // a path with a space or a quote is quoted for a POSIX shell
  let spaced = BdCommand {
    program: "/Applications/Branch Deck.app/Contents/MacOS/it's".into(),
    subcommand: Some("cli"),
  };
  assert_eq!(spaced.for_shell(), r"'/Applications/Branch Deck.app/Contents/MacOS/it'\''s' cli");

  // a binary named brd, e.g. a cargo build, is the CLI itself: no `cli`
  let cargo_brd = home.join("target/debug/brd");
  std::fs::create_dir_all(cargo_brd.parent().unwrap()).unwrap();
  std::fs::write(&cargo_brd, "brd").unwrap();
  environment.executable = Some(cargo_brd.clone());
  assert_eq!(bd_command(&environment).unwrap().for_shell(), cargo_brd.display().to_string());
}

/// An `.AppImage` runs its binary from a mount under `/tmp` that goes when the app quits: the link goes to the `.AppImage` file
#[test_log::test]
fn the_link_target_is_the_appimage_not_its_mount() {
  let mounted = Path::new("/tmp/.mount_BranchXyz/usr/bin/branch-deck");
  let appimage = Path::new("/home/me/Applications/Branch-Deck.AppImage");
  assert_eq!(app_binary_path(mounted, Some(appimage)), appimage);
  assert_eq!(app_binary_path(mounted, None), mounted);
}

/// A translocated app (started from Downloads or a disk image) runs from a copy that goes when it quits
#[test_log::test]
fn a_translocated_app_is_refused() {
  let translocated = Path::new("/private/var/folders/x1/abc/T/AppTranslocation/6A1B-42/d/Branch Deck.app/Contents/MacOS/branch-deck");
  let error = refuse_temporary_location(translocated).unwrap_err();
  assert_eq!(
    (error.code, error.message.as_str()),
    (
      ErrorCode::InstallRefused,
      "Branch Deck runs from a temporary location: move it to Applications, then install the Agent CLI"
    )
  );
  refuse_temporary_location(Path::new("/Applications/Branch Deck.app/Contents/MacOS/branch-deck")).unwrap();
}

#[cfg(unix)]
#[test_log::test]
fn install_links_to_the_appimage() {
  let (home, app) = home_with_app();
  let home = std::fs::canonicalize(home.path()).unwrap();
  let appimage = home.join("Branch-Deck.AppImage");
  std::fs::write(&appimage, "appimage").unwrap();
  let mut environment = environment(&home);
  environment.home = Some(home.clone());
  environment.executable = Some(app);
  environment.appimage = Some(appimage.clone());

  let (exit_code, output) = run_json(&environment, &["install"]);
  assert_eq!((exit_code, &output["brd"]), (0, &json!(home.join(".local/bin/brd").to_str().unwrap())), "{output}");
  assert_eq!(std::fs::read_link(home.join(".local/bin/brd")).unwrap(), appimage);
}

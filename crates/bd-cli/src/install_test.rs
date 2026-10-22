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
  let link = home.join(".local/bin/bd");
  // the agent rules of the global config of this test alone: the defaults
  let global = tempfile::NamedTempFile::new().unwrap();
  let _env = test_utils::env_vars::EnvVars::set(&[("GIT_CONFIG_GLOBAL", global.path())]);
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
      "bd": link.to_str().unwrap(),
      "on_path": true,
      "skills": skill_files(&home),
      "uninstalled": false,
      "agents": { "lease": true, "subject_branch": false }
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
  let (_, schema) = run_install(&home, &app, &["schema"]);
  assert_covered(&schema["outputs"]["install"], &schema["outputs"]["install"], &output, "install");
  let skill = std::fs::read_to_string(&skill_files(&home)[0]).unwrap();
  assert!(skill.starts_with("---\nname: branch-deck\ndescription: "), "{skill}");
  assert!(skill.contains(&format!("bd is `{}`", link.display())), "{skill}");
  assert!(skill.contains(&format!("\nallowed-tools: \"Bash({}:*)\"\n---\n", link.display())), "{skill}");
  assert!(skill.contains("run `bd help agent`"), "{skill}");
  assert!(skill.contains("run `bd help worktrees` first"), "{skill}");
  assert!(skill.contains("Before the first edit or commit in a repository, run `bd workspace`"), "{skill}");
  assert_eq!(std::fs::read_to_string(&skill_files(&home)[1]).unwrap(), skill);
  // `bd help skill` prints the same skill
  let mut out = Vec::new();
  let mut environment = environment(&home);
  environment.home = Some(home.clone());
  environment.executable = Some(link.clone());
  assert_eq!(crate::run_with(["help", "skill"].map(std::ffi::OsString::from), &environment, &mut out), 0);
  assert_eq!(String::from_utf8(out).unwrap(), skill);

  // again, from the link: nothing changes; a symlink elsewhere is replaced
  let (exit_code, output) = run_install(&home, &link, &["install"]);
  assert_eq!((exit_code, &output["bd"]), (0, &json!(link.to_str().unwrap())), "{output}");
  std::fs::remove_file(&link).unwrap();
  std::os::unix::fs::symlink("/old/branch-deck", &link).unwrap();
  let (exit_code, output) = run_install(&home, &app, &["install"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(std::fs::read_link(&link).unwrap(), app);

  let (exit_code, output) = run_install(&home, &app, &["install", "--uninstall"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!((&output["skills"], &output["uninstalled"]), (&json!(skill_files(&home)), &json!(true)));
  assert!(std::fs::symlink_metadata(&link).is_err(), "the symlink is removed");
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
  std::fs::write(home.join(".local/bin/bd"), "another bd").unwrap();

  let (exit_code, output) = run_install(&home, &app, &["install"]);
  assert_eq!((exit_code, &output["error"]["code"]), (4, &json!("install_refused")), "{output}");
  assert_eq!(std::fs::read_to_string(home.join(".local/bin/bd")).unwrap(), "another bd");

  // not on PATH: still installed, and the skill names the full path
  std::fs::remove_file(home.join(".local/bin/bd")).unwrap();
  let mut environment = environment(&home);
  environment.home = Some(home);
  environment.executable = Some(app);
  environment.path = Some("/usr/bin".into());
  let (exit_code, output) = run_json(&environment, &["install"]);
  assert_eq!((exit_code, &output["on_path"]), (0, &json!(false)), "{output}");
}

/// The command of the app's prompts: the installed link when it is this app's, else the app binary with `cli`, which
/// without `cli` would start the app
#[cfg(unix)]
#[test_log::test]
fn the_bd_command_is_the_link_to_this_app_or_the_app_with_cli() {
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
  assert!(String::from_utf8(out).unwrap().contains(&format!("bd is `{} cli`", app.display())));

  // a link to another binary is not this app's bd
  let link = home.join(".local/bin/bd");
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

  // a binary named bd, e.g. a cargo build, is the CLI itself: no `cli`
  let cargo_bd = home.join("target/debug/bd");
  std::fs::create_dir_all(cargo_bd.parent().unwrap()).unwrap();
  std::fs::write(&cargo_bd, "bd").unwrap();
  environment.executable = Some(cargo_bd.clone());
  assert_eq!(bd_command(&environment).unwrap().for_shell(), cargo_bd.display().to_string());
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
  assert_eq!((exit_code, &output["bd"]), (0, &json!(home.join(".local/bin/bd").to_str().unwrap())), "{output}");
  assert_eq!(std::fs::read_link(home.join(".local/bin/bd")).unwrap(), appimage);
}

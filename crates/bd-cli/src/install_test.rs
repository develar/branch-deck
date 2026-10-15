use crate::fixture_test::{environment, run_json};
use crate::install::{BdCommand, SKILL_DIRS, SKILL_NAME, bd_command};
use crate::schema_test::assert_covered;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// A home directory and an app binary in it, with `~/.local/bin` on `PATH`
fn home_with_app() -> (TempDir, PathBuf) {
  let home = tempfile::tempdir().unwrap();
  let home_path = std::fs::canonicalize(home.path()).unwrap();
  let app = home_path.join("BranchDeck.app/Contents/MacOS/branch-deck");
  std::fs::create_dir_all(app.parent().unwrap()).unwrap();
  std::fs::write(&app, "app").unwrap();
  (home, app)
}

fn run_install(home: &Path, app: &Path, args: &[&str]) -> (i32, Value) {
  let mut environment = environment(home);
  environment.home = Some(home.to_path_buf());
  environment.executable = Some(app.to_path_buf());
  environment.path = Some(std::env::join_paths([home.join(".local/bin"), PathBuf::from("/usr/bin")]).unwrap());
  run_json(&environment, args)
}

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

  let (exit_code, output) = run_install(&home, &app, &["install"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(
    output,
    json!({ "schema": 1, "bd": link.to_str().unwrap(), "on_path": true, "skills": skill_files(&home), "uninstalled": false })
  );
  assert_eq!(std::fs::read_link(&link).unwrap(), app);
  let (_, schema) = run_install(&home, &app, &["schema"]);
  assert_covered(&schema["outputs"]["install"], &schema["outputs"]["install"], &output, "install");
  let skill = std::fs::read_to_string(&skill_files(&home)[0]).unwrap();
  assert!(skill.starts_with("---\nname: branch-deck\ndescription: "), "{skill}");
  assert!(skill.contains(&format!("bd is `{}`", link.display())), "{skill}");
  assert!(skill.contains("run `bd help agent`"), "{skill}");
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
  environment.home = Some(home.clone());
  environment.executable = Some(app.clone());
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
}

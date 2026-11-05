use crate::built_command;
use crate::fixture_test::{assert_error, environment, run_json};
use crate::flag_names::{RENAMED, renamed_in};
use pretty_assertions::assert_eq;
use std::ffi::OsString;

/// The command of `bd` at `path`, `lease pick`
fn command_at<'a>(root: &'a clap::Command, path: &str) -> &'a clap::Command {
  path
    .split(' ')
    .fold(root, |command, name| command.find_subcommand(name).unwrap_or_else(|| panic!("no command {path}")))
}

/// Every renamed flag is still a flag of its command (an alias, or a hidden flag of a word of two senses), and every
/// name it is now is a visible flag of that command
#[test]
fn every_renamed_flag_stays_and_names_flags_that_exist() {
  let root = built_command();
  for renamed in RENAMED {
    let command = command_at(&root, renamed.command);
    let old = renamed.old.trim_start_matches('-');
    let stays = command
      .get_arguments()
      .any(|arg| (arg.get_long() == Some(old) && arg.is_hide_set()) || arg.get_all_aliases().is_some_and(|aliases| aliases.contains(&old)));
    assert!(stays, "{} of bd {} is no alias or hidden flag", renamed.old, renamed.command);
    for now in renamed
      .now
      .split_whitespace()
      .filter(|word| word.starts_with("--"))
      .map(|word| word.trim_matches(|c| c == '(' || c == ')'))
    {
      let visible = command.get_arguments().any(|arg| !arg.is_hide_set() && arg.get_long() == Some(now.trim_start_matches('-')));
      assert!(visible, "{now} of bd {} is no visible flag", renamed.command);
    }
  }
}

/// A renamed flag prints its name now, with `=` too; a value after `--` and another command print nothing
#[test]
fn a_renamed_flag_says_its_name_now() {
  let args = |args: &[&str]| args.iter().map(OsString::from).collect::<Vec<_>>();
  assert_eq!(renamed_in("land", &args(&["land", "auth", "--check"])), ["bd: --check of bd land is now --dry-run"]);
  assert_eq!(
    renamed_in("lease pick", &args(&["lease", "pick", "--unchecked=docs only"])),
    ["bd: --unchecked of bd lease pick is now --allow-unchecked"]
  );
  assert_eq!(renamed_in("land", &args(&["land", "auth", "--", "--check"])), Vec::<String>::new());
  assert_eq!(renamed_in("push", &args(&["push", "--check"])), Vec::<String>::new());
}

/// An old name is the new flag: `--check` of `bd safe-push` and `--dry-run` together are one flag given twice
#[test_log::test]
fn an_old_name_runs_as_the_new_one() {
  let directory = tempfile::tempdir().unwrap();
  let (exit_code, output) = run_json(&environment(directory.path()), &["safe-push", "auth", "--check", "--patronus-dry-run", "--dry-run"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  assert!(
    output["error"]["message"].as_str().unwrap().contains("'--dry-run' cannot be used multiple times"),
    "{output}"
  );
}

/// A flag the command lacks names the commands that have it, by its name now, and the flags of this command that
/// preview or accept a refusal; never a similar name of another sense (`--check` for `--unchecked`)
#[test]
fn a_flag_of_another_command_is_named_with_that_command() {
  let directory = tempfile::tempdir().unwrap();
  let (exit_code, output) = run_json(&environment(directory.path()), &["land", "auth", "--unchecked", "why"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(
    message.contains("--unchecked is no flag of `bd land`; `bd lease pick` and `bd pick` have --allow-unchecked"),
    "{message}"
  );
  assert!(
    message.contains("preview or accept a refusal: --dry-run, --allow-checks-not-passed, --bypass-rules"),
    "{message}"
  );
  assert!(!message.contains("--check'") && !message.contains("similar"), "{message}");
  assert!(message.contains("Usage: bd land") && message.contains("try 'bd land --help'"), "{message}");

  let (exit_code, output) = run_json(&environment(directory.path()), &["-C", "status", "pick", "--without-checks"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  let message = output["error"]["message"].as_str().unwrap();
  assert!(
    message.contains("`bd land` has --allow-checks-not-passed") && message.contains("`bd pick` that preview"),
    "{message}"
  );

  // a typo of a flag of this command keeps clap's similar name
  let (exit_code, output) = run_json(&environment(directory.path()), &["status", "--brif"]);
  assert_error(&output, exit_code, (2, "invalid_arguments"));
  assert!(output["error"]["message"].as_str().unwrap().contains("a similar flag exists: '--brief'"), "{output}");
}

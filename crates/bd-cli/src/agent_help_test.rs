use crate::Cli;
use crate::agent_help::{AGENT_HELP, READ_ONLY};
use crate::fixture_test::{commit_all, git, repo_with_baseline, run_in, write};
use clap::CommandFactory;
use std::collections::BTreeSet;
use test_utils::git_test_utils::TestRepo;

/// Long options `bd help agent` leaves to `--help` (paging, tuning, rare variants) or names by their short form (`-v`)
const NOT_IN_AGENT_HELP: &[&str] = &[
  "body-file",
  "branch",
  "clear",
  "depth",
  "files",
  "include-generated",
  "limit",
  "map",
  "max-commits",
  "offset",
  "remote",
  "rewrite-issues",
  "uninstall",
  "verbose",
];

/// The subcommands and the long options of every command, global ones included; without `--help`, `--version` and
/// hidden options
fn commands_and_options() -> (BTreeSet<String>, BTreeSet<String>) {
  let mut command = Cli::command();
  command.build();
  let options = std::iter::once(&command)
    .chain(command.get_subcommands())
    .flat_map(clap::Command::get_arguments)
    .filter(|argument| !argument.is_hide_set() && !matches!(argument.get_id().as_str(), "help" | "version"))
    .filter_map(|argument| argument.get_long().map(str::to_string))
    .collect();
  let commands = command.get_subcommands().map(|subcommand| subcommand.get_name().to_string()).collect();
  (commands, options)
}

/// The words after `prefix` in `text`: the letters, digits and `-` that follow each occurrence
fn words_after<'a>(text: &'a str, prefix: &str) -> BTreeSet<&'a str> {
  text
    .match_indices(prefix)
    .map(|(index, _)| {
      let rest = &text[index + prefix.len()..];
      &rest[..rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-')).unwrap_or(rest.len())]
    })
    .filter(|word| word.starts_with(|c: char| c.is_ascii_lowercase()))
    .collect()
}

#[test]
fn agent_help_names_every_command_and_option_that_exists() {
  let (commands, options) = commands_and_options();
  let named_commands = words_after(AGENT_HELP, "`bd ");
  let named_options = words_after(AGENT_HELP, "--");

  let missing_commands: Vec<&String> = commands.iter().filter(|command| !named_commands.contains(command.as_str())).collect();
  assert!(missing_commands.is_empty(), "commands `bd help agent` doesn't name: {missing_commands:?}");
  let missing_options: Vec<&String> = options
    .iter()
    .filter(|option| !named_options.contains(option.as_str()) && !NOT_IN_AGENT_HELP.contains(&option.as_str()))
    .collect();
  assert!(
    missing_options.is_empty(),
    "options neither in `bd help agent` nor in NOT_IN_AGENT_HELP: {missing_options:?}"
  );

  let stale_commands: Vec<&&str> = named_commands.iter().filter(|command| !commands.contains(**command)).collect();
  assert!(stale_commands.is_empty(), "`bd help agent` names commands that don't exist: {stale_commands:?}");
  let stale_options: Vec<&&str> = named_options.iter().filter(|option| !options.contains(**option)).collect();
  assert!(stale_options.is_empty(), "`bd help agent` names options that don't exist: {stale_options:?}");
  let listed_twice: Vec<&&str> = NOT_IN_AGENT_HELP
    .iter()
    .filter(|option| named_options.contains(**option) || !options.contains(**option))
    .collect();
  assert!(listed_twice.is_empty(), "NOT_IN_AGENT_HELP lists options that are named or don't exist: {listed_twice:?}");
}

/// The refs, the local config, the index and the working tree status: what a read-only command must not change
fn writable_state(repo: &TestRepo) -> (String, String, Vec<u8>, String) {
  (
    git(repo, &["for-each-ref", "--format=%(refname) %(objectname)"]),
    git(repo, &["config", "--local", "--list"]),
    std::fs::read(repo.path().join(".git/index")).unwrap(),
    git(repo, &["status", "--porcelain=v1", "-z", "--untracked-files=all"]),
  )
}

#[test_log::test]
fn read_only_commands_write_no_ref_config_index_or_working_tree() {
  assert!(AGENT_HELP.contains(READ_ONLY));
  assert!(Cli::command().render_long_help().to_string().contains(READ_ONLY));

  let repo = repo_with_baseline(&[("README.md", "readme\n")]);
  write(&repo, "shared.txt", "one\n");
  commit_all(&repo, "(a) Add shared", 1_700_000_100);
  write(&repo, "shared.txt", "two\n");
  let change = commit_all(&repo, "ABC-1 Change shared", 1_700_000_200);
  // staged and untracked work, which must stay as it is
  write(&repo, "staged.txt", "staged\n");
  git(&repo, &["add", "staged.txt"]);
  write(&repo, "untracked.txt", "untracked\n");
  let plan = tempfile::NamedTempFile::new().unwrap();
  std::fs::write(plan.path(), serde_json::json!({ "branches": { "b": [change] } }).to_string()).unwrap();

  let before = writable_state(&repo);
  for args in [
    &["status", "--stats"][..],
    &["status", "--brief"],
    &["commits", "--files", "--patch"],
    &["commits", "--brief"],
    &["suggest", "--fresh"],
    &["check", "--plan", plan.path().to_str().unwrap()],
    &["schema"],
    &["version"],
  ] {
    let (_, output) = run_in(&repo, args);
    assert_eq!(output.get("error"), None, "{args:?}: {output}");
    assert_eq!(writable_state(&repo), before, "{args:?}");
  }
}

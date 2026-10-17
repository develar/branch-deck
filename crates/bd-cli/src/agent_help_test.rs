use crate::Cli;
use crate::agent_help::{AGENT_HELP, READ_ONLY, UPDATE_HELP};
use crate::fixture_test::{commit_all, git, repo_with_baseline, run_in, write};
use clap::CommandFactory;
use std::collections::{BTreeMap, BTreeSet};
use test_utils::git_test_utils::TestRepo;

/// Options `bd help agent` leaves to `--help` (paging, tuning, rare variants), as `<command> --<option>`, or
/// `--<option>` for a global one; `--verbose` is named by its short form `-v`
const NOT_IN_AGENT_HELP: &[&str] = &[
  "--verbose",
  "commits --branch",
  "commits --depth",
  "commits --files",
  "commits --include-generated",
  "describe --body-file",
  "describe --clear",
  "group --map",
  "install --uninstall",
  "push --dry-run",
  "push --remote",
  "status --branch",
  "status --include-generated",
  "status --limit",
  "status --offset",
  "status --unassigned",
  "suggest --max-commits",
  "suggest --rewrite-issues",
];

/// Commands that the help names "with the same options" as another one
const SAME_OPTIONS_AS: &[(&str, &str)] = &[("rename", "group"), ("restack", "group"), ("unstack", "group")];

/// The long options of the commands: the global ones, and per subcommand its own; without `--help`, `--version` and
/// hidden options
fn options_of_commands() -> (BTreeSet<String>, BTreeMap<String, BTreeSet<String>>) {
  let mut command = Cli::command();
  command.build();
  let longs = |command: &clap::Command, global: bool| -> BTreeSet<String> {
    command
      .get_arguments()
      .filter(|argument| !argument.is_hide_set() && argument.is_global_set() == global && !matches!(argument.get_id().as_str(), "help" | "version"))
      .filter_map(|argument| argument.get_long().map(str::to_string))
      .collect()
  };
  let global = longs(&command, true);
  let commands = command
    .get_subcommands()
    .map(|subcommand| (subcommand.get_name().to_string(), &longs(subcommand, false) - &global))
    .collect();
  (global, commands)
}

/// The commands the help names (`` `bd <command> ``), and each `--option` with the command named last before it, `None`
/// before the first
fn named_in(text: &str) -> (BTreeSet<&str>, Vec<(Option<&str>, &str)>) {
  let word = |rest: &str| -> usize { rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-')).unwrap_or(rest.len()) };
  let mut command = None;
  let mut commands = BTreeSet::new();
  let mut named = Vec::new();
  let mut index = 0;
  while index < text.len() {
    let rest = &text[index..];
    if let Some(after) = rest.strip_prefix("`bd ")
      && after.starts_with(|c: char| c.is_ascii_lowercase())
    {
      command = Some(&after[..word(after)]);
      commands.extend(command);
      index += 4;
    } else if let Some(after) = rest.strip_prefix("--")
      && after.starts_with(|c: char| c.is_ascii_lowercase())
    {
      named.push((command, &after[..word(after)]));
      index += 2;
    } else {
      index += rest.chars().next().map_or(1, char::len_utf8);
    }
  }
  (commands, named)
}

#[test]
fn agent_help_names_every_command_and_option_that_exists() {
  let (global, commands) = options_of_commands();
  // `bd help update` holds the options of the session; together the two texts name everything
  let text = format!("{AGENT_HELP}\n{UPDATE_HELP}");
  let (named_commands, named) = named_in(&text);
  let is_named = |command: Option<&str>, option: &str| named.iter().any(|&(c, o)| o == option && (command.is_none() || c == command));

  let mut missing = Vec::new();
  for option in &global {
    if !is_named(None, option) && !NOT_IN_AGENT_HELP.contains(&format!("--{option}").as_str()) {
      missing.push(format!("--{option}"));
    }
  }
  for (command, options) in &commands {
    if !named_commands.contains(command.as_str()) {
      missing.push(format!("bd {command}"));
    }
    let named_as = SAME_OPTIONS_AS.iter().find(|(same, _)| same == command).map_or(command.as_str(), |(_, other)| other);
    for option in options {
      if !is_named(Some(named_as), option) && !NOT_IN_AGENT_HELP.contains(&format!("{named_as} --{option}").as_str()) {
        missing.push(format!("{command} --{option}"));
      }
    }
  }
  assert!(missing.is_empty(), "neither in `bd help agent`, `bd help update` nor in NOT_IN_AGENT_HELP: {missing:?}");

  // every command and option the help names exists, an option for the command it follows
  let stale: Vec<String> = named
    .iter()
    .filter(|(command, option)| !global.contains(*option) && command.is_none_or(|command| !commands.get(command).is_some_and(|options| options.contains(*option))))
    .map(|(command, option)| format!("{} --{option}", command.unwrap_or("bd")))
    .chain(
      named_commands
        .iter()
        .filter(|command| !commands.contains_key(**command))
        .map(|command| format!("bd {command}")),
    )
    .collect();
  assert!(stale.is_empty(), "`bd help agent` or `bd help update` names what doesn't exist: {stale:?}");

  let wrong: Vec<&&str> = NOT_IN_AGENT_HELP
    .iter()
    .filter(|entry| {
      if let Some((command, option)) = entry.split_once(" --") {
        !commands.get(command).is_some_and(|options| options.contains(option)) || is_named(Some(command), option)
      } else {
        let option = entry.trim_start_matches("--");
        !global.contains(option) || is_named(None, option)
      }
    })
    .collect();
  assert!(wrong.is_empty(), "NOT_IN_AGENT_HELP lists options that are named or don't exist: {wrong:?}");
}

/// HEAD, the refs, the local config, the index and the working tree status: what a read-only command must not change
pub(crate) fn writable_state(repo: &TestRepo) -> (String, String, String, Vec<u8>, String) {
  (
    format!("{}{}", git(repo, &["symbolic-ref", "HEAD"]), git(repo, &["rev-parse", "HEAD"])),
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
  let first = commit_all(&repo, "(a) Add shared", 1_700_000_100);
  write(&repo, "shared.txt", "two\n");
  write(&repo, "other.txt", "other\n");
  let change = commit_all(&repo, "ABC-1 Change shared", 1_700_000_200);
  // staged and untracked work, which must stay as it is
  write(&repo, "staged.txt", "staged\n");
  git(&repo, &["add", "staged.txt"]);
  write(&repo, "untracked.txt", "untracked\n");
  let plan = tempfile::NamedTempFile::new().unwrap();
  std::fs::write(plan.path(), serde_json::json!({ "branches": { "b": [change] } }).to_string()).unwrap();

  let before = writable_state(&repo);
  let cache = repo.path().join(".git/branch-deck/cache");
  let commands = [
    &["status"][..],
    &["status", "--stats"],
    &["status", "--brief"],
    &["commits", "--files", "--patch"],
    &["commits", "--brief"],
    &["suggest"],
    &["suggest", "--fresh"],
    &["check", "--plan", plan.path().to_str().unwrap()],
    &["check", "--build", "echo built > built.txt"],
    &["schema"],
    &["version"],
    &["update", "--dry-run"],
    &["update", "--status"],
    &["fold", "--dry-run"],
    &["move-hunk", &change, &first, "shared.txt", "--dry-run"],
  ];
  // with --no-cache first: no cache appears either
  for no_cache in [true, false] {
    for args in commands {
      let args: Vec<&str> = no_cache.then_some("--no-cache").into_iter().chain(args.iter().copied()).collect();
      let (_, output) = run_in(&repo, &args);
      assert_eq!(output.get("error"), None, "{args:?}: {output}");
      assert_eq!(writable_state(&repo), before, "{args:?}");
      assert!(!(no_cache && cache.exists()), "{args:?} --no-cache wrote the object cache");
    }
  }
  assert!(cache.exists(), "without --no-cache the cache is used");
}

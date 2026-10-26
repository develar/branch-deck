use crate::Cli;
use crate::agent_help::{AGENT_HELP, READ_ONLY, UPDATE_HELP, WORKTREES_HELP, skill_text};
use crate::fixture_test::writable_state;
use crate::fixture_test::{commit_all, git, repo_with_baseline, run_in, write};
use clap::CommandFactory;
use domain_types::error::ErrorCode;
use std::collections::{BTreeMap, BTreeSet};

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
  "push --remote",
  "schema --all",
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

/// The long options of the commands: the global ones, and per subcommand its own, a nested one as `lease get`; without
/// `--help`, `--version` and hidden options
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
  let mut commands = BTreeMap::new();
  for subcommand in command.get_subcommands() {
    commands.insert(subcommand.get_name().to_string(), &longs(subcommand, false) - &global);
    for nested in subcommand.get_subcommands().filter(|nested| nested.get_name() != "help") {
      commands.insert(format!("{} {}", subcommand.get_name(), nested.get_name()), &longs(nested, false) - &global);
    }
  }
  (global, commands)
}

/// The commands the help names (`` `bd <command> ``, `` `bd lease get ``, after -C <dir> too), and each `--option` with
/// the command named last before it, `start` before the first (the command whose `--help` the text is); `--help`
/// itself is no option of a command
fn named_in<'a>(text: &'a str, start: Option<&'a str>, commands_of: &BTreeMap<String, BTreeSet<String>>) -> (BTreeSet<&'a str>, Vec<(Option<&'a str>, &'a str)>) {
  let word = |rest: &str| -> usize { rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-')).unwrap_or(rest.len()) };
  let mut command = start;
  let mut commands = BTreeSet::new();
  let mut named = Vec::new();
  let mut index = 0;
  while index < text.len() {
    let rest = &text[index..];
    if let Some(after) = rest.strip_prefix("`bd ")
      && after.starts_with(|c: char| c.is_ascii_lowercase())
    {
      let first = word(after);
      // a nested command: its name and the word after it
      let nested = after[first..].strip_prefix(' ').map(|next| first + 1 + word(next));
      command = Some(match nested {
        Some(end) if commands_of.contains_key(&after[..end]) => &after[..end],
        _ => &after[..first],
      });
      // a nested command names its parent too
      commands.insert(&after[..first]);
      commands.extend(command);
      index += 4;
    } else if let Some(after) = rest.strip_prefix("--")
      && after.starts_with(|c: char| c.is_ascii_lowercase())
    {
      let option = &after[..word(after)];
      if option != "help" {
        named.push((command, option));
      }
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
  // `bd help update` holds the options of the session, `bd help worktrees` the work beside other sessions, the
  // `--help` of a command its outputs and refusals; together the texts name everything
  let command_helps = command_helps();
  let texts = [(None, AGENT_HELP), (None, UPDATE_HELP), (None, WORKTREES_HELP)]
    .into_iter()
    .chain(command_helps.iter().map(|(name, text)| (Some(name.as_str()), text.as_str())));
  let (mut named_commands, mut named) = (BTreeSet::new(), Vec::new());
  for (start, text) in texts {
    let (commands_of_text, named_of_text) = named_in(text, start, &commands);
    named_commands.extend(commands_of_text);
    named.extend(named_of_text);
  }
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
  assert!(
    missing.is_empty(),
    "neither in `bd help agent`, `bd help update`, `bd help worktrees`, the `--help` of a command nor in NOT_IN_AGENT_HELP: {missing:?}"
  );

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
  assert!(
    stale.is_empty(),
    "`bd help agent`, `bd help update`, `bd help worktrees` or the `--help` of a command names what doesn't exist: {stale:?}"
  );

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

/// The text after the options of each subcommand's `--help` (`after_long_help`): its outputs and refusals
fn command_helps() -> Vec<(String, String)> {
  let mut command = Cli::command();
  command.build();
  command
    .get_subcommands()
    .filter_map(|subcommand| subcommand.get_after_long_help().map(|text| (subcommand.get_name().to_string(), text.to_string())))
    .collect()
}

/// Where an agent reads what a code asks of it: `help <topic>` or the `--help` of a command. Exhaustive, so that a new
/// code is placed when it is added; `&[]` for a code whose message says it all (an invalid argument, a missing repository)
const fn documented_in(code: ErrorCode) -> &'static [&'static str] {
  match code {
    ErrorCode::InvalidArguments
    | ErrorCode::NotARepository
    | ErrorCode::NoBranchPrefix
    | ErrorCode::UnknownBranch
    | ErrorCode::NoBaseline
    | ErrorCode::NotAheadOfBaseline
    | ErrorCode::ParkedMoved
    | ErrorCode::PrefixChanged
    | ErrorCode::NotParked
    | ErrorCode::DetachedHead
    | ErrorCode::NoSession
    | ErrorCode::NoRemote
    | ErrorCode::InstallRefused
    | ErrorCode::NotOnSpace
    | ErrorCode::PatronusRefused
    | ErrorCode::NoLease
    | ErrorCode::LeaseMismatch
    | ErrorCode::NotASlot
    | ErrorCode::SlotInPool
    | ErrorCode::BranchExists
    | ErrorCode::GitNotFound
    | ErrorCode::GitError
    | ErrorCode::NetworkError
    | ErrorCode::PatronusError
    | ErrorCode::Internal
    | ErrorCode::Canceled => &[],
    ErrorCode::InvalidPlan => &["reword"],
    ErrorCode::StackConflict
    | ErrorCode::StackNotBuilt
    | ErrorCode::TargetConflict
    | ErrorCode::PushDismissesApprovals
    | ErrorCode::PullRequestOtherBase
    | ErrorCode::ReviewUnknown
    | ErrorCode::NotLoggedIn => &["safe-push"],
    ErrorCode::ApprovalsMissing | ErrorCode::RulesUnknown => &["help agent", "safe-push"],
    ErrorCode::HunkConflict | ErrorCode::BlockNotFound => &["move-hunk"],
    ErrorCode::HeadMoved => &["help agent", "group"],
    ErrorCode::GroupsMoved | ErrorCode::StacksMoved => &["group"],
    ErrorCode::Locked
    | ErrorCode::BranchNotBuilt
    | ErrorCode::BranchCheckedOut
    | ErrorCode::DropConflict
    | ErrorCode::NothingToReapply
    | ErrorCode::ReapplyConflict
    | ErrorCode::Unapplied
    | ErrorCode::UndoStale => &["help agent"],
    ErrorCode::Parked | ErrorCode::LeaseRejected | ErrorCode::PushTooLarge => &["help agent", "push"],
    ErrorCode::PushRefused | ErrorCode::Landed => &["push", "safe-push"],
    ErrorCode::RobotRunning => &["help agent", "push", "safe-push"],
    ErrorCode::DanglingReference | ErrorCode::MissingDeclaration => &["help agent", "check", "safe-push"],
    ErrorCode::UncommittedOverlap => &["help agent", "help update", "help worktrees"],
    ErrorCode::OperationInProgress
    | ErrorCode::RangeUnknown
    | ErrorCode::SourceOnNewerBaseline
    | ErrorCode::HomeBranchSwitched
    | ErrorCode::NoHomeBranch
    | ErrorCode::SlotBusy
    | ErrorCode::HostSlot
    | ErrorCode::UnlandedCommits
    | ErrorCode::DirtyWorktree
    | ErrorCode::OtherVolume
    | ErrorCode::LowDiskSpace => &["help worktrees"],
    ErrorCode::RoundMoved
    | ErrorCode::SessionStale
    | ErrorCode::UnsupportedConflict
    | ErrorCode::UnresolvedMarkers
    | ErrorCode::ResolutionOutsideConflict
    | ErrorCode::MissingResolution
    | ErrorCode::EmptyResolution
    | ErrorCode::NotLandedRound
    | ErrorCode::LandedDiffers
    | ErrorCode::LandedNotEmpty
    | ErrorCode::FinishPending => &["help update"],
    ErrorCode::NoPullRequest | ErrorCode::NoReviewer | ErrorCode::ReviewRefused => &["request-review"],
  }
}

/// Every refusal an agent must act on is named where `documented_in` says: in the core, a topic or the `--help` of its
/// command, which the error points the agent to
#[test]
fn every_refusal_is_in_the_help_of_its_command() {
  let command_helps: BTreeMap<String, String> = command_helps().into_iter().collect();
  let codes = schemars::schema_for!(ErrorCode).to_value();
  let codes: Vec<&str> = codes["oneOf"]
    .as_array()
    .map(|variants| variants.iter().filter_map(|variant| variant["const"].as_str()).collect())
    .or_else(|| codes["enum"].as_array().map(|variants| variants.iter().filter_map(serde_json::Value::as_str).collect()))
    .unwrap();
  assert!(codes.len() > 50, "{codes:?}");
  let mut missing = Vec::new();
  for name in codes {
    let code: ErrorCode = serde_json::from_value(serde_json::Value::from(name)).unwrap();
    for place in documented_in(code) {
      let text = match *place {
        "help agent" => AGENT_HELP,
        "help update" => UPDATE_HELP,
        "help worktrees" => WORKTREES_HELP,
        command => command_helps.get(command).unwrap_or_else(|| panic!("`bd {command} --help` has no outputs and refusals")),
      };
      if !text.contains(&format!("`{name}`")) {
        missing.push(format!("{name} in {place}"));
      }
    }
  }
  assert!(missing.is_empty(), "refusals that their help doesn't name: {missing:?}");
}

/// `bd help <topic>` prints its text; an unknown topic names every topic
#[test]
fn every_help_topic_prints_its_text() {
  let directory = tempfile::tempdir().unwrap();
  let environment = crate::fixture_test::environment(directory.path());
  for (topic, text) in [("agent", AGENT_HELP), ("update", UPDATE_HELP), ("worktrees", WORKTREES_HELP)] {
    let mut out = Vec::new();
    assert_eq!(crate::run_with(["help", topic].map(std::ffi::OsString::from), &environment, &mut out), 0, "{topic}");
    assert_eq!(String::from_utf8(out).unwrap(), text, "{topic}");
  }
  // a command: its `--help`, the outputs and refusals after the options
  let mut out = Vec::new();
  assert_eq!(crate::run_with(["help", "push"].map(std::ffi::OsString::from), &environment, &mut out), 0);
  let push_help = String::from_utf8(out).unwrap();
  assert!(push_help.contains("Usage: bd push") && push_help.contains("`push_too_large`"), "{push_help}");
  let (exit_code, output) = crate::fixture_test::run_json(&environment, &["help", "lanes"]);
  assert_eq!(exit_code, 2, "{output}");
  assert!(output["error"]["message"].as_str().unwrap().contains("`bd help worktrees`"), "{output}");
}

/// `bd help worktrees` puts the prefix at commit time first, lands commits only with `bd pick`, and checks by content
#[test]
fn the_worktrees_help_writes_the_prefix_first_and_checks_by_content() {
  let prefix = WORKTREES_HELP.find("write the prefix\n   when you commit, never later").unwrap();
  let pick = WORKTREES_HELP.find("`bd pick --from <your worktree> --follow`").unwrap();
  assert!(prefix < pick, "{WORKTREES_HELP}");
  for needed in [
    "never with `git cherry-pick`",
    "--dry-run",
    "Never compare by date",
    "branch-deck: pick",
    "`bd workspace` says `lease.on`",
  ] {
    assert!(WORKTREES_HELP.contains(needed), "{needed}");
  }
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
    &["schema", "status"],
    &["version"],
    &["update", "--dry-run"],
    &["update", "--status"],
    &["pick", &first, "--dry-run"],
    &["group", "--dry-run", "--plan", plan.path().to_str().unwrap()],
    &["rename", "a", "renamed", "--dry-run"],
    &["describe", "a"],
    &["fold", "--dry-run"],
    &["move-hunk", &change, &first, "shared.txt", "--dry-run"],
    &["reword", &change, "--subject", "ABC-1 Change the shared file", "--dry-run"],
    &["lease", "status"],
    &["workspace"],
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

/// The command of `allowed-tools` is a YAML string, also when its shell quoting holds ` #`, a quote or a backslash
#[test]
fn the_skill_allows_every_bd_command_as_named() {
  assert!(skill_text("bd").contains("\nallowed-tools: \"Bash(bd:*)\"\n---\n"));
  assert!(skill_text("'/Apps/Branch #Deck.app/bd' cli").contains("\nallowed-tools: \"Bash('/Apps/Branch #Deck.app/bd' cli:*)\"\n"));
  assert!(skill_text(r#"& 'C:\Apps\a\"b\bd.exe'"#).contains(r#"allowed-tools: "Bash(& 'C:\\Apps\\a\\\"b\\bd.exe':*)""#));
}

/// The description says when to load the skill, first of all before the first edit, within the 1024 characters of the
/// Agent Skills format, where an agent truncates or refuses a longer one
#[test]
fn the_skill_description_starts_with_the_first_edit_and_fits_the_limit() {
  let skill = skill_text("bd");
  let description = skill.lines().find_map(|line| line.strip_prefix("description: ")).unwrap();
  assert!(
    description.starts_with("Use before the first edit or commit in a Git repository: `bd workspace`"),
    "{description}"
  );
  assert!(description.chars().count() <= 1024, "{} characters", description.chars().count());
}

/// A repository needs no instructions of its own for the isolation: the skill says that a lease is the only worktree
/// of the agent's own, also when bd fails, and how a sandbox writes the lease
#[test]
fn the_skill_keeps_the_agent_to_its_lease_also_when_bd_fails_and_in_a_sandbox() {
  let skill = skill_text("bd");
  assert!(skill.contains("never `git worktree add` or a clone for it on your own initiative, also not\n  when a bd command fails"));
  assert!(skill.contains("ask for write\n  access to exactly `path` (`request_permissions`), never the pool or full access. Refused, return the untouched lease."));
}

/// `bd help agent` is what every agent reads before grouping or pushing, so it stays a core of at most 10 KB; the
/// fields of an output go into the `--help` of its command. A rule that waits for the user never leaves it: an agent
/// acts on the error it gets and seldom reads a `--help` first
#[test]
fn the_agent_help_is_a_short_core_with_every_guard() {
  assert!(
    AGENT_HELP.len() <= 10_240,
    "`bd help agent` has {} bytes: move the fields of an output into the `after_long_help` of its command",
    AGENT_HELP.len()
  );
  for guard in [
    "never raise `branchdeck.pushSizeLimit` yourself",
    "never push, start or unpark a parked branch on your own",
    "dropped_commits`; `bd push --force --expect-remote <remote.head>` for\n  that one branch only once the user approved",
    "`--without-approvals` only\n  when the user says so",
    "only when the user says it is a false alarm",
    "never pass `bd lease get --min-free 0` on your own",
    "each `error.next_steps` entry with\n  `needs_approval`",
    "Never stash, reset, check out or commit the\n  user's files",
  ] {
    assert!(AGENT_HELP.contains(guard), "{guard}");
  }
  // the core sends the agent to the `--help` of each command that has its outputs and refusals there
  for (command, _) in command_helps() {
    assert!(
      AGENT_HELP.contains(&format!("`bd {command} --help`")),
      "`bd {command} --help` is not named by `bd help agent`"
    );
  }
}

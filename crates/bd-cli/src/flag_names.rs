//! The names of the flags of `bd` as an agent types them (spec/cli/agent-cli.spec.md "Flag names"): one word for one
//! sense in every command. A preview is `--dry-run`; a flag that accepts a refusal is `--allow-<what refused>`, given
//! only once the user agreed; an act is its own verb (`--force`, `--cancel-running`). A flag renamed to this vocabulary
//! keeps its old name as an alias, and its use prints one line on stderr (`renamed_in`). A flag that a command lacks is
//! `invalid_arguments` whose message names the commands that have it and the flags of this one that preview or accept a
//! refusal (`unknown_argument`), never a similar name alone: clap's edit distance offered `--check`, a preview, for
//! `brd land --unchecked`.

use clap::Command;
use clap::error::{ContextKind, ContextValue, Error};
use std::ffi::OsString;
use std::fmt::Write as _;

/// A flag renamed to the vocabulary: the command, its old name and what it is now
pub struct Renamed {
  /// `land`, `lease pick`
  pub command: &'static str,
  /// The old name, an alias of the new flag or a hidden flag of its own
  pub old: &'static str,
  /// The new flags in words: `--dry-run`, or `--overwrite-edited (with --materialize)` for a flag of two senses
  pub now: &'static str,
}

/// Every renamed flag: an alias that stays, so that a skill written before keeps working
pub const RENAMED: &[Renamed] = &[
  Renamed {
    command: "safe-push",
    old: "--check",
    now: "--dry-run",
  },
  Renamed {
    command: "land",
    old: "--check",
    now: "--dry-run",
  },
  Renamed {
    command: "request-review",
    old: "--check",
    now: "--dry-run",
  },
  Renamed {
    command: "land",
    old: "--without-checks",
    now: "--allow-checks-not-passed",
  },
  Renamed {
    command: "lease pick",
    old: "--unchecked",
    now: "--allow-unchecked",
  },
  Renamed {
    command: "pick",
    old: "--unchecked",
    now: "--allow-unchecked",
  },
  Renamed {
    command: "update",
    old: "--force",
    now: "--overwrite-edited (with --materialize) or --allow-live-session (with --abort)",
  },
];

/// The lines for stderr of the renamed flags that `args` of the command `op` used: `brd: --check of brd land is now
/// --dry-run`. The arguments after `--` are values
#[must_use]
pub fn renamed_in(op: &str, args: &[OsString]) -> Vec<String> {
  let flags: Vec<&str> = args
    .iter()
    .map_while(|arg| arg.to_str().filter(|arg| *arg != "--"))
    .filter(|arg| arg.starts_with("--"))
    .map(|arg| arg.split('=').next().unwrap_or(arg))
    .collect();
  RENAMED
    .iter()
    .filter(|renamed| renamed.command == op && flags.contains(&renamed.old))
    .map(|renamed| format!("brd: {} of brd {op} is now {}", renamed.old, renamed.now))
    .collect()
}

/// The message of an argument the command lacks (`ErrorKind::UnknownArgument`) in `root`, the built command of `brd`:
/// the commands that have the flag, by its name now, and the flags of this command that preview or accept a refusal;
/// clap's similar name only when neither says more
#[must_use]
pub fn unknown_argument(root: &Command, error: &Error, args: &[OsString]) -> String {
  let Some(ContextValue::String(asked)) = error.get(ContextKind::InvalidArg) else {
    return error.render().to_string().trim().to_string();
  };
  let path = command_path(root, args);
  let command = path.iter().fold(root, |command, name| command.find_subcommand(name).unwrap_or(command));
  let op = path.join(" ");
  let name = asked.trim_start_matches('-').split('=').next().unwrap_or_default();
  let mut message = format!("error: unexpected argument '{asked}' found\n");
  let elsewhere = flags_elsewhere(root, &op, name);
  if !elsewhere.is_empty() {
    let mut by_flag: Vec<(&str, Vec<String>)> = Vec::new();
    for (command, flag) in &elsewhere {
      match by_flag.iter_mut().find(|(named, _)| named == flag) {
        Some((_, commands)) => commands.push(format!("`brd {command}`")),
        None => by_flag.push((flag, vec![format!("`brd {command}`")])),
      }
    }
    let named = by_flag
      .iter()
      .map(|(flag, commands)| match commands.as_slice() {
        [one] => format!("{one} has --{flag}"),
        [rest @ .., last] => format!("{} and {last} have --{flag}", rest.join(", ")),
        [] => String::new(),
      })
      .collect::<Vec<_>>()
      .join("; ");
    let _ = write!(message, "\n  tip: --{name} is no flag of `brd {op}`; {named}\n");
  }
  let previews = preview_and_refusal_flags(command);
  if !previews.is_empty() {
    let _ = write!(message, "\n  tip: the flags of `brd {op}` that preview or accept a refusal: {}\n", previews.join(", "));
  }
  if elsewhere.is_empty()
    && let Some(ContextValue::String(suggested)) = error.get(ContextKind::SuggestedArg)
    && let Some(flag) = visible_name(command, suggested.trim_start_matches('-'))
  {
    let _ = write!(message, "\n  tip: a similar flag exists: '--{flag}'\n");
  }
  let mut command = command.clone();
  let _ = write!(message, "\n{}\n\nFor more information, try 'brd {op} --help'.", command.render_usage());
  message
}

/// The subcommands that `args` name, from `root` down: `["lease", "pick"]`. The value of an option that takes one is
/// skipped, so `-C status` names no command
fn command_path(root: &Command, args: &[OsString]) -> Vec<String> {
  let mut path = Vec::new();
  let mut command = root;
  let mut skip_value = false;
  for arg in args.iter().filter_map(|arg| arg.to_str()) {
    if std::mem::take(&mut skip_value) {
      continue;
    }
    if arg == "--" {
      break;
    }
    if let Some(flag) = arg.strip_prefix('-') {
      skip_value = !arg.contains('=') && takes_value(root, command, flag);
      continue;
    }
    match command.find_subcommand(arg) {
      Some(subcommand) => {
        path.push(subcommand.get_name().to_string());
        command = subcommand;
      }
      None => break,
    }
  }
  path
}

/// Whether the option `flag` (without its first `-`) of `command`, or a global one of `root`, takes a value
fn takes_value(root: &Command, command: &Command, flag: &str) -> bool {
  let matches = |arg: &&clap::Arg| match flag.strip_prefix('-') {
    Some(long) => arg.get_long() == Some(long) || arg.get_all_aliases().is_some_and(|aliases| aliases.contains(&long)),
    None => flag.chars().next().is_some_and(|short| arg.get_short() == Some(short)),
  };
  command
    .get_arguments()
    .chain(root.get_arguments())
    .find(matches)
    .is_some_and(|arg| arg.get_action().takes_values())
}

/// The commands other than `op` with a visible flag of the name or the alias `name`, each with the flag's name now
fn flags_elsewhere(root: &Command, op: &str, name: &str) -> Vec<(String, String)> {
  let mut found = Vec::new();
  let mut stack: Vec<(String, &Command)> = root.get_subcommands().map(|command| (command.get_name().to_string(), command)).collect();
  while let Some((path, command)) = stack.pop() {
    if command.is_hide_set() {
      continue;
    }
    if path != op
      && let Some(flag) = visible_name(command, name)
    {
      found.push((path.clone(), flag));
    }
    stack.extend(command.get_subcommands().map(|subcommand| (format!("{path} {}", subcommand.get_name()), subcommand)));
  }
  found.sort();
  found
}

/// The name of the visible flag of `command` whose name or alias is `name`
fn visible_name(command: &Command, name: &str) -> Option<String> {
  command
    .get_arguments()
    .filter(|arg| !arg.is_hide_set())
    .find(|arg| arg.get_long() == Some(name) || arg.get_all_aliases().is_some_and(|aliases| aliases.contains(&name)))
    .and_then(|arg| arg.get_long().map(str::to_string))
}

/// The visible flags of `command` that preview (`--dry-run`) or accept a refusal (`--allow-*`, `--without-*`,
/// `--bypass-*`, `--force`)
fn preview_and_refusal_flags(command: &Command) -> Vec<String> {
  command
    .get_arguments()
    .filter(|arg| !arg.is_hide_set())
    .filter_map(clap::Arg::get_long)
    .filter(|long| *long == "dry-run" || *long == "force" || ["allow-", "without-", "bypass-"].iter().any(|prefix| long.starts_with(prefix)))
    .map(|long| format!("--{long}"))
    .collect()
}

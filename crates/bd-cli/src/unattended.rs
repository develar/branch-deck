//! Unattended Mode in brd (openspec/specs/repository/unattended-mode/spec.md): `brd unattended on|off|status`, the
//! switch of the app's setting through its writer (`app_settings::unattended::set_unattended`) with the requests denied
//! since the mode was last turned on, and `brd hook permission-request`, the `PermissionRequest` hook of Claude Code that
//! `brd install` adds (`claude_hook`).
//!
//! The hook runs before every permission prompt of every session and subagent, so it reads only the settings file of
//! the app and its stdin, starts no git and no network, logs nothing, and always exits 0: with the mode on it prints the
//! denial whose message tells the agent to go on without the access and appends the request to the journal of denials;
//! with the mode off, a settings file it cannot read, or an input that is not a hook's, it prints nothing, and Claude
//! Code asks the user as it would without the hook. It never allows anything.

use crate::claude_hook::{HookExec, claude_settings_file, hook_state};
use app_settings::file::SETTINGS_FILE_NAME;
use app_settings::service::SettingsService;
use app_settings::unattended::{append_denial, denials_file, read_denials, set_unattended};
use bd_core::bd_command::{BdCommand, CLI_NAME, bd_command, display};
use bd_core::context::{Environment, app_settings_of};
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use domain_types::error::ErrorCode;
use domain_types::unattended::{SUMMARY_LIMIT, UnattendedDenial, UnattendedHook};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::Path;

/// What the hook tells an agent whose request it denied
pub const DENY_MESSAGE: &str = "Branch Deck is in Unattended Mode: nobody can approve this. Do it without new access (a path inside the working \
                                directories, the pool path of a lease's notes), or skip this step, name it in your report and go on.";

/// The origin of a change of the settings that brd asks for, as a window label names the change of a window
pub const ORIGIN: &str = "brd";

#[derive(Debug, clap::Args)]
pub struct UnattendedArgs {
  #[command(subcommand)]
  pub command: UnattendedCommand,
}

#[derive(Debug, Clone, Copy, clap::Subcommand)]
pub enum UnattendedCommand {
  /// Turn the mode on: every permission request of an agent is denied with a message to go on without it; the list of
  /// the last run starts anew
  On,
  /// Turn the mode off: Claude Code asks as before; the list of this run stays until the mode is turned on again
  Off,
  /// Whether the mode is on, whether its hook is installed, and the requests denied since it was last turned on
  Status,
}

#[derive(Debug, clap::Args)]
pub struct HookArgs {
  #[command(subcommand)]
  pub command: HookCommand,
}

#[derive(Debug, Clone, Copy, clap::Subcommand)]
pub enum HookCommand {
  /// The `PermissionRequest` hook of Claude Code (`brd install` adds it): reads the hook's JSON on stdin; in Unattended
  /// Mode prints the denial and records it, else prints nothing and Claude Code asks as usual. Always exits 0
  PermissionRequest,
}

/// `brd unattended on|off|status`
#[derive(Debug, Serialize, JsonSchema)]
pub struct UnattendedOutput {
  pub schema: SchemaVersion,
  /// Whether the mode is on now
  pub on: bool,
  /// Whether this command turned it on or off; false for `status`, and for a switch to the state it was in
  pub changed: bool,
  /// Whether `~/.claude/settings.json` has the hook of this brd: `missing` or `stale` deny nothing until `brd install`,
  /// `invalid` until the file is fixed and `brd install` runs
  pub hook: UnattendedHook,
  /// How many requests were denied since the mode was last turned on
  pub denied: usize,
  /// Those requests, oldest first
  pub denials: Vec<UnattendedDenial>,
  /// The journal they are read from
  pub journal: String,
  /// What to run next: `brd install` while the hook is not this brd's
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub next: Vec<String>,
}

/// `brd unattended`
///
/// # Errors
/// No app data directory, a settings writer that cannot start or save, a journal of the last run that cannot be removed
pub fn unattended(environment: &Environment, args: &UnattendedArgs) -> Result<UnattendedOutput, CliError> {
  let app_data = environment
    .app_data
    .as_deref()
    .ok_or_else(|| CliError::new(ErrorCode::InvalidArguments, "No data directory of Branch Deck on this machine"))?;
  let journal = denials_file(app_data);
  let (on, changed) = match args.command {
    UnattendedCommand::Status => (app_settings_of(Some(&app_data.join(SETTINGS_FILE_NAME))).unattended, false),
    UnattendedCommand::On | UnattendedCommand::Off => {
      let on = matches!(args.command, UnattendedCommand::On);
      std::fs::create_dir_all(app_data).map_err(|e| CliError::new(ErrorCode::Internal, format!("{}: {e}", app_data.display())))?;
      let settings = SettingsService::open(app_data.to_path_buf()).map_err(|e| CliError::new(ErrorCode::Internal, format!("Cannot open the settings of Branch Deck: {e}")))?;
      // saved once it answers: the hook reads the file
      let switch = set_unattended(&settings, &journal, on, ORIGIN)
        .map_err(|e| CliError::new(ErrorCode::Internal, format!("Cannot turn {} Unattended Mode: {e}", if on { "on" } else { "off" })))?;
      (on, switch.changed)
    }
  };
  let hook = installed_hook(environment);
  let denials = read_denials(&journal);
  let next = if hook == UnattendedHook::Installed {
    Vec::new()
  } else {
    vec![format!(
      "{} install",
      bd_command(environment).map_or_else(|_| CLI_NAME.to_owned(), |command| command.for_shell())
    )]
  };
  Ok(UnattendedOutput {
    schema: SchemaVersion,
    on,
    changed,
    hook,
    denied: denials.len(),
    denials,
    journal: display(&journal),
    next,
  })
}

/// Whether the settings of Claude Code under the home directory have the hook of this brd (`brd install` writes the
/// command of the `brd` symlink, `bd_command` names it once the symlink points here)
pub fn installed_hook(environment: &Environment) -> UnattendedHook {
  let (Some(home), Ok(brd)) = (environment.home.as_deref(), bd_command(environment)) else {
    return UnattendedHook::Missing;
  };
  hook_state(&claude_settings_file(home), &HookExec::of(&brd))
}

/// The hook of `brd install` for `brd`, the command it writes
pub fn hook_command_of(brd: &Path) -> HookExec {
  HookExec::of(&BdCommand {
    program: brd.to_path_buf(),
    subcommand: None,
  })
}

/// The input of the hook that the denial reads; every other field of it is ignored
#[derive(Deserialize)]
struct HookInput {
  #[serde(default)]
  hook_event_name: Option<String>,
  #[serde(default)]
  session_id: String,
  #[serde(default)]
  agent_id: Option<String>,
  #[serde(default)]
  agent_type: Option<String>,
  #[serde(default)]
  cwd: String,
  tool_name: String,
  #[serde(default)]
  tool_input: Value,
}

/// The answer of the hook to `input` with the app data directory `app_data` at `now` (Unix seconds): the JSON that
/// denies the request while the mode is on, after the request went into the journal of denials; none otherwise
pub fn permission_request(input: &str, app_data: &Path, now: u32) -> Option<String> {
  if !app_settings_of(Some(&app_data.join(SETTINGS_FILE_NAME))).unattended {
    return None;
  }
  let input: HookInput = serde_json::from_str(input).ok()?;
  if input.hook_event_name.as_deref().is_some_and(|event| event != "PermissionRequest") {
    return None;
  }
  let denial = UnattendedDenial {
    time: now,
    session_id: input.session_id,
    agent_id: input.agent_id.filter(|id| !id.is_empty()),
    agent_type: input.agent_type.filter(|kind| !kind.is_empty()),
    cwd: input.cwd,
    summary: summary_of(&input.tool_input),
    tool_name: input.tool_name,
  };
  // the denial matters more than its record: a journal that cannot be written denies all the same
  let _ = append_denial(&denials_file(app_data), &denial);
  Some(
    json!({
      "hookSpecificOutput": {
        "hookEventName": "PermissionRequest",
        "decision": { "behavior": "deny", "message": DENY_MESSAGE },
      }
    })
    .to_string(),
  )
}

/// `brd hook permission-request` of the process: the input from `stdin`, the answer to `stdout`; exit code 0 whatever
/// happens, so Claude Code never reads a failure of the hook
pub fn run_permission_request(app_data: Option<&Path>, stdin: &mut dyn Read, stdout: &mut dyn Write) -> i32 {
  let mut input = String::new();
  if stdin.read_to_string(&mut input).is_err() {
    return 0;
  }
  if let Some(answer) = app_data.and_then(|app_data| permission_request(&input, app_data, sync_utils::timestamp::unix_now())) {
    let _ = writeln!(stdout, "{answer}");
    let _ = stdout.flush();
  }
  0
}

/// The fields of a tool's input that say what it was asked for, the first one the input has: the command of `Bash`, the
/// path of a file tool, the URL of a fetch, the pattern or query of a search. Never `content`, `new_string` and the like
const SUMMARY_FIELDS: [&str; 7] = ["command", "file_path", "notebook_path", "url", "pattern", "query", "path"];

/// What the request asked for, at most `SUMMARY_LIMIT` characters: the first of `SUMMARY_FIELDS` the input has, else
/// the names of its arguments
pub fn summary_of(tool_input: &Value) -> String {
  let Some(object) = tool_input.as_object() else {
    return String::new();
  };
  let text = SUMMARY_FIELDS
    .iter()
    .find_map(|field| object.get(*field).and_then(Value::as_str))
    .map_or_else(|| object.keys().map(String::as_str).collect::<Vec<_>>().join(", "), str::to_owned);
  if text.chars().count() <= SUMMARY_LIMIT {
    return text;
  }
  let mut cut: String = text.chars().take(SUMMARY_LIMIT - 1).collect();
  cut.push('…');
  cut
}

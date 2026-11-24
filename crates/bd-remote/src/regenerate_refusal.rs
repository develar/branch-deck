//! The refusal of a push or a start whose copy holds a generated file the words merged in a conflict of its sync
//! (`Confirmation::Regenerate`, `domain_policy::regenerate`), one wording for `brd push`, `brd safe-push` and `brd land`:
//! the file, why it is generated, the copy that merged it, the commit the copy lacks and how the copy gets it. The flag
//! that lands anyway is the user's word, named in the message, never a step.

use crate::safe_push_command::Refusal;
use bd_core::bd_command::shell_word;
use bd_core::context::RepoContext;
use domain_types::conflict::RegenerateFile;
use domain_types::error::ErrorCode;
use domain_types::landing::{LandingFix, StackCommit};

/// The flag of `brd push`, `brd safe-push` and `brd land` that lands such a copy anyway
pub const FLAG: &str = "--allow-generated-merged-by-words";

/// The refusal of the copy of `what` that holds `files`, led by `lead` ("Nothing was pushed"), with `fix`, how the copy
/// gets the commit it lacks (`BranchTable::regenerate_fix`)
pub(crate) fn regenerate_refusal(context: &RepoContext, lead: &str, what: &str, files: &[RegenerateFile], fix: Option<&LandingFix>) -> Refusal {
  let short = context.short_ids(
    files
      .iter()
      .flat_map(|file| [Some(file.commit.id.as_str()), file.lacks.as_ref().map(|lacks| lacks.id.as_str())])
      .flatten(),
  );
  let named = |commit: &StackCommit| format!("{} ({})", short.get(&commit.id), commit.subject);
  let Some(first) = files.first() else {
    return Refusal::new(
      ErrorCode::GeneratedMergedByWords,
      format!("{lead}: the copy of {what} holds a generated file the words merged"),
    );
  };
  let lacks = first.lacks.as_ref().map_or_default(|lacks| {
    let of = if lacks.branch.is_empty() {
      "unassigned".to_string()
    } else {
      format!("of {}", lacks.branch)
    };
    format!("; the copy lacks {} {of}, which changes it too", named(lacks))
  });
  let (fix_words, step) = match fix {
    Some(LandingFix::Restack { branch, onto }) => {
      let command = format!("{} restack {} {}", context.bd, shell_word(branch), shell_word(onto));
      (
        format!(". `{command}` gives the copy that commit after the user approves, or it clears once {onto} lands"),
        Some(command),
      )
    }
    Some(LandingFix::MoveCommit { commit, into }) => (format!(". Move {} into {into}, after the user approves", short.get(&commit.id)), None),
    Some(LandingFix::Unstack { branch, from }) => (format!(". `{} unstack {branch}` lands it without {from}", context.bd), None),
    Some(LandingFix::Start { subject }) => {
      let whole_stack = if subject.is_whole_stack() { " --whole-stack" } else { "" };
      (
        format!(". `{} safe-push {}{whole_stack}` lands the copy with that commit", context.bd, shell_word(subject.name())),
        None,
      )
    }
    None => (String::new(), None),
  };
  let more = match files.len() - 1 {
    0 => String::new(),
    rest => format!(" ({rest} more: `regenerate` of `brd sync`)"),
  };
  let message = format!(
    "{lead}: the copy of {what} holds {}, which the words merged in the copy of {} in a conflict of the sync, not its generator ({}){lacks}{fix_words}{more}; ask the user, and only once they agree pass {FLAG}",
    first.path,
    named(&first.commit),
    first.generated_by.describe(),
  );
  let refusal = Refusal::new(ErrorCode::GeneratedMergedByWords, message);
  match step {
    Some(command) => refusal.with_next_step(command, true),
    None => refusal,
  }
}

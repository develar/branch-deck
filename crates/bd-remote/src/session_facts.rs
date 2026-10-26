//! The facts of the remote a `bd` command decides on, read as the app's session reads them: a one-shot session of the
//! command's table (`repository_model::one_shot::OneShot`: the table of its sync, or of a grouping it read without one),
//! filled through the one reader of the facts (`ServiceFactSources` over `BdServices`) with what its intent needs
//! (`operations::needs`, `operations::facts::fill`). `bd push`, `bd safe-push`, `bd robot` and `bd status` read the
//! robots, what they land, landed and hold, the pull requests, the profiles, the rules and the review from it; the words
//! stay theirs.

use bd_core::context::RepoContext;
use bd_core::error::CliError;
use bd_core::services::session_key;
use domain_types::error::ErrorCode;
use domain_types::patronus::{PatronusAvailability, PatronusRobot, PatronusRobotsResult};
use git_executor::git_command_executor::GitCommandExecutor;
use operations::facts::fill;
use operations::needs::FactNeed;
use remote_facts::fact_sources::FactSources;
use repository_model::branch_table::BranchTable;
use repository_model::facts::{FactResult, RobotsRead};
use repository_model::one_shot::OneShot;
use repository_model::table::TableState;
use std::sync::Arc;

/// The one-shot session of a command and the reads of its facts
pub struct SessionFacts {
  pub session: OneShot,
  sources: Arc<dyn FactSources>,
  git: GitCommandExecutor,
}

impl SessionFacts {
  /// The session of `table` with `state`, the table state of the same sync (an empty one beside the table of a
  /// grouping), read through `sources`
  pub fn new(context: &RepoContext, state: TableState, table: BranchTable, sources: Arc<dyn FactSources>) -> Self {
    Self {
      session: OneShot::new(session_key(context), state, table),
      sources,
      git: context.git.clone(),
    }
  }

  /// Reads `needs` into the facts of the session, side by side, one request per fact
  pub fn read(&mut self, context: &RepoContext, needs: &[FactNeed]) -> Result<(), CliError> {
    context.runtime()?.block_on(fill(&mut self.session, &self.sources, &self.git, needs));
    Ok(())
  }

  /// The reads of the facts of the command, for a read that is no fact of the session (the robot a comment started)
  pub fn sources(&self) -> Arc<dyn FactSources> {
    Arc::clone(&self.sources)
  }

  /// The Patronus view of the table over the robots the session read (`OneShot::derive_patronus`): what they land,
  /// landed and hold
  pub fn derive(&mut self) {
    self.session.derive_patronus(&self.git);
  }

  /// The robots `robots` as the session knows them now, then the Patronus view over them (`know_robots`)
  pub fn know_robots(&mut self, robots: Vec<PatronusRobot>) {
    know_robots(&mut self.session, &self.git, robots);
  }
}

/// The robots `robots` as `session` knows them now, then the Patronus view over them: the robots `bd status` and the
/// merge of `bd robot` read of the object cache (`robot_list::read_robots`), not of a list of this command
pub fn know_robots(session: &mut OneShot, git: &GitCommandExecutor, robots: Vec<PatronusRobot>) {
  let result = PatronusRobotsResult {
    availability: PatronusAvailability::Available,
    robots,
    github_repository: None,
  };
  session.apply(FactResult::Robots {
    read: RobotsRead::List { full: true },
    result,
  });
  session.derive_patronus(git);
}

/// The robots the session listed, or why Patronus listed none, as `bd` fails with it
pub fn listed_robots(session: &OneShot) -> Result<&[PatronusRobot], CliError> {
  available(session.facts().robots.view().head.value.as_ref().map(|head| &head.availability))?;
  Ok(session.robots())
}

/// The robots started since `since` (Unix seconds), asked now through `sources` (the robot a `/patronus` comment
/// started), or why Patronus listed none
pub fn robots_since(context: &RepoContext, sources: &dyn FactSources, since: u32) -> Result<Vec<PatronusRobot>, CliError> {
  let result = context.runtime()?.block_on(sources.robots(&session_key(context), Some(since), false));
  available(Some(&result.availability))?;
  Ok(result.robots)
}

fn available(availability: Option<&PatronusAvailability>) -> Result<(), CliError> {
  match availability {
    Some(PatronusAvailability::Available) => Ok(()),
    Some(PatronusAvailability::NotLoggedIn) => Err(CliError::new(
      ErrorCode::NotLoggedIn,
      "Patronus rejected the Space token: run ./safePush.cmd once to log in again",
    )),
    Some(PatronusAvailability::Error { message }) => Err(CliError::new(ErrorCode::PatronusError, message.clone())),
    Some(PatronusAvailability::NotPatronus) | None => Err(CliError::new(ErrorCode::NotOnSpace, "The repository is not on Space: Patronus can't push it")),
  }
}

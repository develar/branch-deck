//! What the app's actions on the robots run on, the same for the app's commands and the E2E server's routes: the
//! restart and the cancel of a robot (`start_guard`; a start is `operations::land_execute`, the push `operations::push`).
//! Each host builds one `RemoteActions` per command (`RemoteActions::new`); every flow is a method of it, in the module of
//! its checks, and reads the user's robots itself: never the page's, never a list kept before. The table the checks read
//! is the one of the last sync of the repository's session (`Sessions::table`). The branch prefix is the one the host
//! resolved for the command (`BranchPrefixes`), never one the page sent.

use crate::facts::TableFacts;
use crate::session::{SessionKey, Sessions};
use anyhow::{Result, anyhow};
use domain_types::patronus::PatronusRobot;
use git_executor::git_command_executor::GitCommandExecutor;
use patronus::service::PatronusService;
use sync_core::patronus::SpaceRepository;

/// Why an action that needs Patronus is refused off Space, in the words of both hosts
pub const NOT_ON_SPACE: &str = "The repository is not on Space: Patronus can't push it";

/// What the app's actions on the robots run on: the Patronus service with the Space repository (none off Space), git,
/// the sessions with the tables of their last syncs and the prefix the branches of the repository are named under
pub struct RemoteActions<'a> {
  /// The service and the Space repository Patronus names the repository by (`ultimate`); `None` off Space: no robot
  /// holds a push, and a robot action is refused
  patronus: Option<(&'a PatronusService, &'a str)>,
  pub(crate) git_executor: &'a GitCommandExecutor,
  /// The session of the repository under `branch_prefix` holds the table of its last sync
  pub(crate) sessions: &'a Sessions,
  /// The prefix the host resolved for the command: the branches of a push and of a robot, the key of the session
  pub(crate) branch_prefix: &'a str,
}

impl<'a> RemoteActions<'a> {
  /// `patronus`: the service and the Space repository of the repository; `None` off Space, or when the host can't read it.
  /// `branch_prefix`: the prefix the host resolved for the repository of the command
  pub fn new(patronus: Option<(&'a PatronusService, &'a SpaceRepository)>, git_executor: &'a GitCommandExecutor, sessions: &'a Sessions, branch_prefix: &'a str) -> Self {
    Self {
      patronus: patronus.map(|(service, space)| (service, space.name.as_str())),
      git_executor,
      sessions,
      branch_prefix,
    }
  }

  /// The session of `repository_path` under the prefix of the command
  pub(crate) fn key(&self, repository_path: &str) -> SessionKey {
    SessionKey {
      worktree: repository_path.to_owned(),
      prefix: self.branch_prefix.to_owned(),
    }
  }

  /// The table of the last sync of the session of `repository_path`, with the memo of its views; `None` while the session
  /// is not open or never synced
  pub(crate) async fn table(&self, repository_path: &str) -> Option<TableFacts> {
    self.sessions.table(&self.key(repository_path)).await
  }

  /// The service and the Space repository of a robot action; off Space the action is refused
  pub(crate) fn patronus(&self) -> Result<(&'a PatronusService, &'a str)> {
    self.patronus.ok_or_else(|| anyhow!(NOT_ON_SPACE))
  }

  /// The user's robots for a robot action: one full list asked now (`PatronusService::robots_for_action`); a list
  /// Patronus doesn't give fails the action, in the words of a failed action
  pub(crate) async fn robots_for_action(&self) -> Result<Vec<PatronusRobot>> {
    let (service, repository) = self.patronus()?;
    service.robots_for_action(repository, self.branch_prefix).await.map_err(|e| anyhow!(e))
  }
}

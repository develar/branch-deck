//! What the app's actions on the remote run on, the same for the app's commands and the E2E server's routes: the push
//! (`push_guard`), and the landing check of the popover, the start, the restart and the cancel of a robot
//! (`start_guard`). Each host builds one `RemoteActions` per command (`RemoteActions::new`); every flow is a method of
//! it, in the module of its checks, and reads the user's robots itself: never the page's, never a list kept before.

use crate::views::RepositoryViews;
use anyhow::{Result, anyhow};
use domain_types::patronus::PatronusRobot;
use git_executor::git_command_executor::GitCommandExecutor;
use patronus::service::PatronusService;
use sync_core::patronus::SpaceRepository;

/// Why an action that needs Patronus is refused off Space, in the words of both hosts
pub const NOT_ON_SPACE: &str = "The repository is not on Space: Patronus can't push it";

/// What the app's actions on the remote run on: the Patronus service with the Space repository (none off Space), git
/// and the tables of the last syncs
pub struct RemoteActions<'a> {
  /// The service and the Space repository Patronus names the repository by (`ultimate`); `None` off Space: no robot
  /// holds a push, and a robot action is refused
  patronus: Option<(&'a PatronusService, &'a str)>,
  pub(crate) git_executor: &'a GitCommandExecutor,
  pub(crate) views: &'a RepositoryViews,
}

impl<'a> RemoteActions<'a> {
  /// `patronus`: the service and the Space repository of the repository; `None` off Space, or when the host can't read it
  pub fn new(patronus: Option<(&'a PatronusService, &'a SpaceRepository)>, git_executor: &'a GitCommandExecutor, views: &'a RepositoryViews) -> Self {
    Self {
      patronus: patronus.map(|(service, space)| (service, space.name.as_str())),
      git_executor,
      views,
    }
  }

  /// The user's robots for a check that holds nothing up (the push, the landing check): one full list asked now
  /// (`PatronusService::robots_for_checks`), read before the lock, as no network request runs under it; none off Space
  pub(crate) async fn robots_for_checks(&self, branch_prefix: &str) -> Vec<PatronusRobot> {
    match self.patronus {
      Some((service, repository)) => service.robots_for_checks(repository, branch_prefix).await,
      None => Vec::new(),
    }
  }

  /// The service and the Space repository of a robot action; off Space the action is refused
  pub(crate) fn patronus(&self) -> Result<(&'a PatronusService, &'a str)> {
    self.patronus.ok_or_else(|| anyhow!(NOT_ON_SPACE))
  }

  /// The user's robots for a robot action: one full list asked now (`PatronusService::robots_for_action`); a list
  /// Patronus doesn't give fails the action, in the words of a failed action
  pub(crate) async fn robots_for_action(&self, branch_prefix: &str) -> Result<Vec<PatronusRobot>> {
    let (service, repository) = self.patronus()?;
    service.robots_for_action(repository, branch_prefix).await.map_err(|e| anyhow!(e))
  }
}

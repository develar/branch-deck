use domain_types::cli_tools::CliToolStatus;
use std::sync::Mutex;

/// The statuses the Setup dot takes without running the CLIs again: those of the read that started last. Reads run at
/// once (the Setup dot of each window, Settings → Tools) and end in any order; a read that started before an action
/// ended and ends after the read that followed it would bring back what the action changed
#[derive(Default)]
pub struct KeptStatuses {
  state: Mutex<Reads>,
}

#[derive(Default)]
struct Reads {
  /// The reads started so far
  started: u64,
  /// The ticket of the read the statuses are of; a read with no greater ticket keeps nothing
  kept_from: u64,
  statuses: Option<Vec<CliToolStatus>>,
}

/// The place of a read among the reads started
#[derive(Clone, Copy, Debug)]
pub struct ReadTicket(u64);

impl KeptStatuses {
  /// Called as a read starts, before it asks anything
  pub fn start_read(&self) -> ReadTicket {
    let mut reads = self.lock();
    reads.started += 1;
    ReadTicket(reads.started)
  }

  /// Keeps what the read of `ticket` found, unless a read that started after it was kept already
  pub fn keep(&self, ticket: ReadTicket, statuses: &[CliToolStatus]) {
    let mut reads = self.lock();
    if ticket.0 > reads.kept_from {
      reads.kept_from = ticket.0;
      reads.statuses = Some(statuses.to_vec());
    }
  }

  /// The statuses kept; None before the first read ended
  pub fn get(&self) -> Option<Vec<CliToolStatus>> {
    self.lock().statuses.clone()
  }

  /// Forgets the statuses and every read running now: the tools changed under them (a fixture of the E2E server)
  pub fn forget(&self) {
    let mut reads = self.lock();
    reads.kept_from = reads.started;
    reads.statuses = None;
  }

  fn lock(&self) -> std::sync::MutexGuard<'_, Reads> {
    self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
  }
}

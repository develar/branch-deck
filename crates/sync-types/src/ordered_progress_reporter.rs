use crate::{ProgressReporter, SyncEvent};
use crossbeam::queue::SegQueue;
use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

/// Progress reporter wrapper that ensures correct event ordering
/// Queues branch-specific events until BranchesGrouped is sent
pub struct OrderedProgressReporter<P: ProgressReporter> {
  inner: P,
  queued_events: Arc<SegQueue<SyncEvent>>,
  branches_grouped_sent: Arc<AtomicBool>,
}

impl<P: ProgressReporter> OrderedProgressReporter<P> {
  pub fn new(inner: P) -> Self {
    Self {
      inner,
      queued_events: Arc::new(SegQueue::new()),
      branches_grouped_sent: Arc::new(AtomicBool::new(false)),
    }
  }

  fn is_branch_specific_event(event: &SyncEvent) -> bool {
    matches!(
      event,
      SyncEvent::BranchStatusUpdate { .. } | SyncEvent::CommitSynced { .. } | SyncEvent::CommitError { .. } | SyncEvent::CommitsBlocked { .. } | SyncEvent::RemoteStatusUpdate(..)
    )
  }

  fn flush_queued_events(&self) -> anyhow::Result<()> {
    while let Some(event) = self.queued_events.pop() {
      self.inner.send(event)?;
    }
    Ok(())
  }
}

impl<P: ProgressReporter> ProgressReporter for OrderedProgressReporter<P> {
  fn send(&self, event: SyncEvent) -> anyhow::Result<()> {
    match &event {
      SyncEvent::BranchesGrouped { .. } => {
        // Send BranchesGrouped immediately
        self.inner.send(event)?;
        // Mark as sent and flush all queued events
        self.branches_grouped_sent.store(true, Ordering::Release);
        self.flush_queued_events()?;
        Ok(())
      }
      _ if Self::is_branch_specific_event(&event) => {
        if self.branches_grouped_sent.load(Ordering::Acquire) {
          // BranchesGrouped already sent, pass through immediately
          self.inner.send(event)
        } else {
          // Queue the event until BranchesGrouped is sent
          self.queued_events.push(event);
          Ok(())
        }
      }
      _ => {
        // Non-branch events (IssueNavigationConfig, UnassignedCommits, Completed) pass through
        self.inner.send(event)
      }
    }
  }
}

impl<P: ProgressReporter> Clone for OrderedProgressReporter<P>
where
  P: Clone,
{
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
      queued_events: Arc::clone(&self.queued_events),
      branches_grouped_sent: Arc::clone(&self.branches_grouped_sent),
    }
  }
}

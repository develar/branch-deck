//! The ordered queue of a connection (docs/design/backend-protocol.md, "Key decision 6"): every frame the host writes to
//! a client goes through it, in the order of its pushes, so an action pushed before an answer is read before it. The
//! actions of a window of a channel are counted: a window past [`STALE_CAP`] of waiting actions has them dropped and is
//! stale, its next actions dropped too, until the host resyncs it once the queue drained ("A slow subscriber is
//! resynced").

use crate::wire::Frame;
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::sync::{Mutex, MutexGuard, PoisonError};
use tokio::sync::Notify;

/// The bytes of the waiting actions of one window of a channel past which they are dropped for one resync
pub const STALE_CAP: usize = 8 * 1024 * 1024;

/// A window of a channel: the id of the channel and the label of the window
pub type WindowOf = (u64, String);

/// What became of an action pushed for a window
#[derive(Debug, PartialEq, Eq)]
pub enum Pushed {
  /// It waits in the queue
  Queued,
  /// The window is stale: the action and those that waited are dropped until its resync
  Stale,
  /// The connection ended
  Closed,
}

#[derive(Default)]
pub struct Outbox {
  state: Mutex<State>,
  ready: Notify,
}

#[derive(Default)]
struct State {
  frames: VecDeque<(Frame, Option<WindowOf>)>,
  closed: bool,
  /// The bytes of the waiting actions of each window
  waiting: HashMap<WindowOf, usize>,
  /// The windows whose actions are dropped until their resync
  stale: BTreeSet<WindowOf>,
  /// The stale windows due to resync once the queue drained
  due: BTreeSet<WindowOf>,
}

impl Outbox {
  fn state(&self) -> MutexGuard<'_, State> {
    self.state.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// Queues `frame`: false once the connection ended
  pub fn push(&self, frame: Frame) -> bool {
    {
      let mut state = self.state();
      if state.closed {
        return false;
      }
      state.frames.push_back((frame, None));
    }
    self.ready.notify_one();
    true
  }

  /// Queues the action `frame` of the window `of`, within its cap (`STALE_CAP`)
  pub fn push_action(&self, of: &WindowOf, frame: Frame) -> Pushed {
    self.push_action_within(of, frame, STALE_CAP)
  }

  pub(crate) fn push_action_within(&self, of: &WindowOf, frame: Frame, cap: usize) -> Pushed {
    {
      let mut state = self.state();
      if state.closed {
        return Pushed::Closed;
      }
      if state.stale.contains(of) {
        return Pushed::Stale;
      }
      let waiting = state.waiting.get(of).copied().unwrap_or(0) + frame.size();
      if waiting > cap {
        state.frames.retain(|(_, window)| window.as_ref() != Some(of));
        state.waiting.remove(of);
        state.stale.insert(of.clone());
        state.due.insert(of.clone());
        drop(state);
        self.ready.notify_one();
        return Pushed::Stale;
      }
      state.waiting.insert(of.clone(), waiting);
      state.frames.push_back((frame, Some(of.clone())));
    }
    self.ready.notify_one();
    Pushed::Queued
  }

  /// Whether the actions of `of` are dropped until its resync
  pub fn is_stale(&self, of: &WindowOf) -> bool {
    self.state().stale.contains(of)
  }

  /// The resync of `of` is queued: its actions are queued again
  pub fn resynced(&self, of: &WindowOf) {
    let mut state = self.state();
    state.stale.remove(of);
    state.due.remove(of);
  }

  /// The window `of` left its channel: nothing of it waits to resync
  pub fn forget(&self, of: &WindowOf) {
    let mut state = self.state();
    state.stale.remove(of);
    state.due.remove(of);
    state.waiting.remove(of);
  }

  /// The next frame, once one is queued; or the stale windows due to resync now that the queue drained; none once the
  /// connection ended and its queue is empty
  pub async fn next(&self) -> Option<Next> {
    loop {
      let notified = self.ready.notified();
      {
        let mut state = self.state();
        if let Some((frame, of)) = state.frames.pop_front() {
          if let Some(of) = of
            && let Some(waiting) = state.waiting.get_mut(&of)
          {
            *waiting = waiting.saturating_sub(frame.size());
          }
          return Some(Next::Frame(frame));
        }
        if !state.due.is_empty() {
          return Some(Next::Resync(std::mem::take(&mut state.due).into_iter().collect()));
        }
        if state.closed {
          return None;
        }
      }
      notified.await;
    }
  }

  /// The connection ended: nothing more is queued, what is queued is still read
  pub fn close(&self) {
    self.state().closed = true;
    self.ready.notify_one();
  }

  pub fn is_closed(&self) -> bool {
    self.state().closed
  }
}

/// What a writer of a connection takes from its queue
#[derive(Debug, PartialEq, Eq)]
pub enum Next {
  Frame(Frame),
  /// The stale windows to resync now
  Resync(Vec<WindowOf>),
}

#[cfg(test)]
#[path = "outbox_test.rs"]
mod outbox_test;

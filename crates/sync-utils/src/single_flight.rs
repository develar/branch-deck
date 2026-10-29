//! One read for every caller that asks for the same key while it runs (a single flight), with no lock held across it
//! (docs/design/rust-domain-authority.md, «Инварианты», 2). The flights are a `std` mutex over a map, held only to join
//! a flight or to register one; a caller that joins waits for the answer of the flight, never for a guard, so nothing
//! the flight does (a process, a request, a file) holds anyone who asks for something else. No runtime: the answer
//! wakes the joiners itself, so the crates that only read files (`app-settings`) stay without one.
//!
//! - The first caller of a key leads: it runs the read, every caller that asks meanwhile gets its answer.
//! - A leader that goes away before it answered (its caller was dropped) ends the flight: its joiners ask again, and
//!   the first of them leads.
//! - The answer leaves the map once every joiner can see it: a caller that comes later starts a new flight, so what
//!   should outlive the flight (a kept token, a list in the object cache) is kept by the read before it answers.

use std::collections::HashMap;
use std::hash::Hash;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::task::{Context, Poll, Waker};

/// The flights in the air, by key
pub struct SingleFlight<K, V> {
  flights: Mutex<HashMap<K, Arc<Flight<V>>>>,
}

/// A flight: its answer, and the joiners to wake when it comes
struct Flight<V> {
  state: Mutex<FlightState<V>>,
}

enum FlightState<V> {
  Running(Vec<Waker>),
  Answered(V),
  /// The leader went away without an answer
  Abandoned,
}

/// What a caller does for its key: lead a new flight or join the one in the air
pub enum Begin<'a, K: Eq + Hash, V> {
  Lead(Lead<'a, K, V>),
  Join(Join<V>),
}

/// The flight of a key that this caller runs; dropped without `finish`, it ends without an answer
pub struct Lead<'a, K: Eq + Hash, V> {
  flights: &'a SingleFlight<K, V>,
  key: Option<K>,
  flight: Arc<Flight<V>>,
}

/// The flight of another caller that this one waits for: a future of its answer, None when its leader went away
pub struct Join<V> {
  flight: Arc<Flight<V>>,
}

impl<K, V> Default for SingleFlight<K, V> {
  fn default() -> Self {
    Self {
      flights: Mutex::new(HashMap::new()),
    }
  }
}

impl<V> Flight<V> {
  fn state(&self) -> MutexGuard<'_, FlightState<V>> {
    self.state.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// The flight ends with `end` unless it ended, and its joiners wake
  fn end(&self, end: FlightState<V>) {
    let mut state = self.state();
    if let FlightState::Running(waiting) = &mut *state {
      let waiting = std::mem::take(waiting);
      *state = end;
      drop(state);
      waiting.into_iter().for_each(Waker::wake);
    }
  }
}

impl<K: Eq + Hash + Clone, V: Clone> SingleFlight<K, V> {
  fn flights(&self) -> MutexGuard<'_, HashMap<K, Arc<Flight<V>>>> {
    self.flights.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// Leads a new flight of `key`, or joins the one in the air
  pub fn begin(&self, key: K) -> Begin<'_, K, V> {
    let mut flights = self.flights();
    if let Some(flight) = flights.get(&key) {
      return Begin::Join(Join { flight: Arc::clone(flight) });
    }
    let flight = Arc::new(Flight {
      state: Mutex::new(FlightState::Running(Vec::new())),
    });
    flights.insert(key.clone(), Arc::clone(&flight));
    Begin::Lead(Lead {
      flights: self,
      key: Some(key),
      flight,
    })
  }

  /// The answer of `read` for `key`: of the flight in the air when one runs, else of `read` run now, which every caller
  /// that asks meanwhile gets. A leader that went away is replaced by this caller.
  pub async fn run<F: Future<Output = V>>(&self, key: K, read: impl FnOnce() -> F) -> V {
    loop {
      match self.begin(key.clone()) {
        Begin::Lead(lead) => {
          let answer = read().await;
          lead.finish(answer.clone());
          return answer;
        }
        Begin::Join(join) => {
          if let Some(answer) = join.await {
            return answer;
          }
        }
      }
    }
  }

  /// The flights in the air answer their callers, and a caller that asks from now on starts a new one: what they read
  /// is of before (a sign-in, a new tool)
  pub fn forget(&self) {
    self.flights().clear();
  }
}

impl<K: Eq + Hash, V> Lead<'_, K, V> {
  /// The answer of the flight, to every caller that joined it
  pub fn finish(self, answer: V) {
    self.flight.end(FlightState::Answered(answer));
  }
}

impl<K: Eq + Hash, V> Drop for Lead<'_, K, V> {
  fn drop(&mut self) {
    // the answer first: a caller that comes once the flight left the map starts its own
    self.flight.end(FlightState::Abandoned);
    let Some(key) = self.key.take() else { return };
    let mut flights = self.flights.flights.lock().unwrap_or_else(PoisonError::into_inner);
    if flights.get(&key).is_some_and(|flight| Arc::ptr_eq(flight, &self.flight)) {
      flights.remove(&key);
    }
  }
}

impl<V: Clone> Future for Join<V> {
  type Output = Option<V>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let mut state = self.flight.state();
    match &mut *state {
      FlightState::Answered(answer) => Poll::Ready(Some(answer.clone())),
      FlightState::Abandoned => Poll::Ready(None),
      FlightState::Running(waiting) => {
        if !waiting.iter().any(|waker| waker.will_wake(cx.waker())) {
          waiting.push(cx.waker().clone());
        }
        Poll::Pending
      }
    }
  }
}

use crate::get_queue::{Behind, Gate, QueueTicket, TicketPhase, settle_queue};
use pretty_assertions::assert_eq;
use std::time::Duration;
use sync_utils::process_scan::process_started;

/// The time of the tests, Unix milliseconds: a heartbeat of it is fresh, one a minute before it stale
const NOW: u64 = 1_000_000_000;
const STALE: Duration = Duration::from_mins(1);

/// A waiting ticket of this process that entered at `entered_ms`, its heartbeat at `NOW`
fn ticket(id: &str, entered_ms: u64) -> QueueTicket {
  let pid = std::process::id();
  QueueTicket {
    id: id.to_string(),
    holder: format!("holder-{id}"),
    pid,
    process_started: process_started(pid),
    entered_ms,
    heartbeat_ms: NOW,
    phase: TicketPhase::Waiting,
    tried: Vec::new(),
    takes_over: false,
  }
}

fn ahead(gate: &Gate) -> Option<(String, usize)> {
  gate.behind().map(|Behind { holder, ahead, .. }| (holder, ahead))
}

/// The tickets wait in the order they entered, whatever their order in the state; a get without a ticket comes after
/// every one, and only the first ticket is first
#[test]
fn the_tickets_wait_in_the_order_they_entered_and_a_get_without_one_comes_last() {
  let queue = vec![ticket("third", 30), ticket("first", 10), ticket("second", 20)];
  let first = Gate::of(&queue, Some(&queue[1]), NOW, STALE);
  assert!(first.is_first());
  let third = Gate::of(&queue, Some(&queue[0]), NOW, STALE);
  assert_eq!(ahead(&third), Some(("holder-second".to_string(), 2)));
  let without = Gate::of(&queue, None, NOW, STALE);
  assert_eq!(ahead(&without), Some(("holder-third".to_string(), 3)));
  // two tickets of one millisecond: the id orders them
  let same = vec![ticket("b", 10), ticket("a", 10)];
  assert!(Gate::of(&same, Some(&same[1]), NOW, STALE).is_first());
  assert!(!Gate::of(&same, Some(&same[0]), NOW, STALE).is_first());
}

/// A free slot goes to the first waiting ticket that has not tried it: a ticket behind it, and a get without a ticket,
/// may take it only once every ticket ahead tried it. An idle lease waits only for the tickets that take idle leases
/// over
#[test]
fn a_free_slot_goes_to_the_first_ticket_that_has_not_tried_it_and_an_idle_lease_to_one_that_takes_it_over() {
  let mut head = ticket("head", 10);
  head.tried = vec!["/pool/1".to_string()];
  let mut taker = ticket("taker", 20);
  taker.takes_over = true;
  taker.tried = vec!["/pool/1".to_string(), "/pool/2".to_string()];
  let queue = vec![head, taker, ticket("last", 30)];
  let last = Gate::of(&queue, Some(&queue[2]), NOW, STALE);
  assert!(last.lets_free("/pool/1"), "every ticket ahead tried it");
  assert!(!last.lets_free("/pool/2"), "the head has not tried it");
  assert!(last.lets_idle("/pool/2"), "the head takes no idle lease over, and the taker tried it");
  assert!(!last.lets_idle("/pool/3"), "the taker has not tried it");
  let taker = Gate::of(&queue, Some(&queue[1]), NOW, STALE);
  assert!(taker.lets_idle("/pool/3"));
  assert!(!taker.lets_free("/pool/3"));
}

/// The queue passes over a busy ticket, a ticket whose heartbeat is older than the stale limit and a ticket whose
/// process is gone. The first two keep their place: a stale ticket with a fresh heartbeat is ahead again. The ticket
/// of a gone process leaves the queue at the next write (`settle_queue`)
#[test]
fn a_busy_a_stale_and_a_gone_ticket_are_passed_over_and_only_a_gone_one_leaves() {
  let mut busy = ticket("busy", 10);
  busy.phase = TicketPhase::Busy;
  let mut stale = ticket("stale", 20);
  stale.heartbeat_ms = NOW - 60_001;
  let mut gone = ticket("gone", 30);
  gone.process_started = Some(1);
  let mut queue = vec![busy, stale, gone, ticket("waiting", 40)];
  let gate = Gate::of(&queue, Some(&queue[3]), NOW, STALE);
  assert!(gate.is_first(), "{gate:?}");
  assert!(gate.lets_free("/pool/1"));

  queue[1].heartbeat_ms = NOW - 60_000;
  assert_eq!(ahead(&Gate::of(&queue, Some(&queue[3]), NOW, STALE)), Some(("holder-stale".to_string(), 1)));
  assert!(Gate::of(&queue, Some(&queue[3]), NOW, Duration::from_secs(59)).is_first(), "the stale limit is a parameter");

  settle_queue(&mut queue);
  let ids: Vec<&str> = queue.iter().map(|ticket| ticket.id.as_str()).collect();
  assert_eq!(ids, ["busy", "stale", "waiting"]);
}

/// A phase that a later brd wrote parses as `Unknown`, and the queue passes the ticket over as a busy one, so this brd
/// still reads the state
#[test]
fn a_ticket_of_a_phase_that_a_later_brd_wrote_is_passed_over_as_a_busy_one() {
  let mut later = serde_json::to_value(ticket("later", 10)).unwrap();
  later["phase"] = serde_json::json!("phase_of_a_later_brd");
  let later: QueueTicket = serde_json::from_value(later).unwrap();
  assert_eq!(later.phase, TicketPhase::Unknown);
  let queue = vec![later, ticket("waiting", 20)];
  assert!(Gate::of(&queue, Some(&queue[1]), NOW, STALE).is_first());
}

//! The engine of the operations of the remote (docs/design/rust-domain-authority.md, "Планирование и исполнение
//! операций", "Конкурентность"): an intent of the user or an agent (`domain_types::operation::Intent`) is admitted over
//! the operations that run or wait in its session, runs the effects its plan orders, and ends with an outcome its caller
//! and every window see. The app and `bd` are two adapters of it: the same intents, plans and outcomes, the same
//! admission, the same witness. It sits above the model and the facts (`repository-model`, the rules of
//! `domain-policy`) and adds no git of its own beyond the reads of its witness.
//!
//! - `registry`: the in-process registry, the one writer of the `operations` part of each session: the admission of an
//!   operation and its entry into the session as one change on the actor of the session, its step and moves, its end
//!   with the admission again of the ones that waited, in the order they came, and the requests that join it
//! - `engine`: the frame of every family: admitted, the body of the family runs and ends the operation with its answer;
//!   joined, the answer of the operation it joined
//! - `needs`: what the plan of an intent reads of the remote before it decides (`needs`), and what the first decision
//!   of a start waits for (`waits`)
//! - `facts`: the reads of the needs, side by side, one per fact, into the facts of the session of the operation
//!   (`fill`, a `OneShot` of `bd` today)
//! - `events`: where the caller of an operation hears its steps, the progress of its push and its outcome
//!   (`EventSink`)
//! - `witness`: what a plan was prepared on (`Witness`, its `Fingerprint`): the tips of the refs it moves and leases
//!   from one `for-each-ref`, the pull requests and the robots it reads, and the check before the first effect
//!   (`plan_changed`)

pub mod engine;
pub mod events;
pub mod facts;
pub mod needs;
pub mod registry;
pub mod witness;

#[cfg(test)]
mod http_budget_test;
#[cfg(test)]
mod process_budget_test;

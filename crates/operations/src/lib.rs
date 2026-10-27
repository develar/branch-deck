//! The engine of the operations of the remote (docs/design/rust-domain-authority.md, "Планирование и исполнение
//! операций", "Конкурентность"): an intent of the user or an agent (`domain_types::operation::Intent`) is admitted over
//! the operations that run or wait in its session, runs the effects its plan orders, and ends with an outcome its caller
//! and every window see. The app and `bd` are two adapters of it: the same intents, plans and outcomes, the same
//! admission, the same witness. It sits above the model and the facts (`repository-model`, the rules of
//! `domain-policy`) and adds no git of its own beyond the reads of its witness.
//!
//! - `registry`: the in-process registry, the one writer of the `operations` part of each session: the admission of an
//!   operation and its entry into the session as one change on the actor of the session, its step and moves, its end
//!   with the admission again of the ones that waited, in the order they came, and the requests that join it; between
//!   processes the registry file admits first (`RegistryFiles`)
//! - `registry_file`: the operations registry of a repository (`operations.json` in `$GIT_COMMON_DIR/branch-deck/`), its
//!   one writer: a record per operation of each process of the repository, `busy` while another process runs what an
//!   operation needs, the records of processes that are gone dropped, the heartbeat of the records of this process
//! - `engine`: the frame of every family: admitted, the body of the family runs and ends the operation with its answer;
//!   joined, the answer of the operation it joined; `prepare` and `execute` by the family of the intent; the start of
//!   the app admitted with what its plan reserves (`execute_land`); what the run of a plan asks the registry for, the
//!   app's and `bd`'s alike (`request_of`)
//! - `needs`: what the plan of an intent reads of the remote before it decides (`needs`), and what the first decision
//!   of a start waits for (`waits`)
//! - `facts`: the reads of the needs, side by side, one per fact, into the facts of the session of the operation
//!   (`fill`, a `OneShot` of `bd` today)
//! - `events`: where the caller of an operation hears its steps, the progress of its push and its outcome
//!   (`EventSink`)
//! - `issue`: the state of a YouTrack issue (Mark Fixed and its undo, the same intent)
//! - `land`: the plan of a Safe Push or a Dry Run (`prepare`): the decision of `domain_policy::land` over the facts of
//!   the session of the app, or of the one-shot session of `bd` with the reads its first decision waits for, with the
//!   profiles it offers and its fingerprint, and with what its run needs (`planned_of`)
//! - `land_execute`: the run of a start, one for the app and `bd` (`execute`): the plan prepared again, the cheap
//!   witness, the lock only for the full witness and the freeze of the push, then the retargets, the cancel, the push,
//!   the late witness and the start, each effect with its status in the outcome; `StartHost` is where its facts live;
//!   `dry_run`, what the remote would answer the push of a plan (`bd push --dry-run`)
//! - `land_session`: the `StartHost` of the app and the E2E server, the session of the repository (`SessionStart`), and
//!   what a start of theirs runs on (`session_start`)
//! - `robot_start`: the start of the robot through the API, or as a `/patronus` comment whose reply is read on the
//!   clock of the session, and the robot it started (`comment_robot`)
//! - `push`: the push of the virtual branches, one core for the app and `bd`: the lock only while the witness is read in
//!   full and its commits and leases are fixed, the network push after it (`send`, once more over a remote that moved
//!   without commits a branch lacks); the intent `Push` of a branch alone, of a stack (`domain_policy::push::push_plan`),
//!   up to a commit, of an unapplied branch and of the stack branch of a whole stack; the plans of several as one push
//!   (`push_together`, `bd push`)
//! - `robot`: the restart of a robot of the user, with its push first while the branch has work the remote lacks, run as
//!   a start runs, and its cancel (`cancel`: one that finished is as it ended, the cancel of a start too)
//! - `pull_request`: the writes of pull requests an operation makes: the retarget into the target, a step of a start and
//!   a push, the retarget the user asks of a pull request (`Intent::Retarget`), the creation of the pull request of a
//!   subject and a review request of its open one, each with the push it needs first
//! - `close`: the closes of pull requests: the one the user asks with its reason, and those of the pull requests a merged
//!   robot landed with it (D7), each with its comment, one GitHub refuses stopping none of the others
//! - `review`: the review of a pull request of someone else and its Safe Push: the `/patronus merge` comment while
//!   GitHub shows the approved commit, its robot looked for a while after the reply
//! - `witness`: what a plan was prepared on (`Witness`, its `Fingerprint`): the tips of the refs it moves and leases
//!   from one `for-each-ref`, the pull requests and the robots it reads, and the check before the first effect
//!   (`plan_changed`)

pub mod close;
pub mod engine;
pub mod events;
pub mod facts;
pub mod issue;
pub mod land;
pub mod land_execute;
pub mod land_session;
pub mod needs;
pub mod pull_request;
pub mod push;
pub mod registry;
pub mod registry_file;
pub mod review;
pub mod robot;
pub mod robot_start;
pub mod witness;

#[cfg(test)]
mod http_budget_test;
#[cfg(test)]
mod process_budget_test;
#[cfg(test)]
mod registry_scan_test;
#[cfg(test)]
mod remote_write_scan_test;

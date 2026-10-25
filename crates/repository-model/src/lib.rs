//! What Rust derives of the branch table, so the page and `bd` read one rule (docs/design/git-operations.md,
//! "Repository view"). The app keeps the table of its last sync and answers `get_repository_view` from it; `bd` builds
//! the table from the report of its own sync or from a grouping, and reads the same view.
//!
//! The crate grows into the model of a repository (docs/design/rust-domain-authority.md): the sessions of the process
//! own the table of each worktree and run its syncs (`session`, `runtime`, `sync_scheduler`), and hold its facts
//! (`facts`): the table of the last sync and the robots of Patronus now, the other facts of the remote services in the
//! steps of s7-session-facts (the scheduler of `remote-facts` reads them). Besides them: the table of the last sync for `get_repository_view` (`branch_table`, `views`), the
//! rule of what the robots land (`patronus_view`) and the guards of the app's actions on the remote (`push_guard`,
//! `start_guard`, run through `actions`). It holds what the checks and the remote services found and asks no remote
//! service itself, except the Patronus client `RemoteActions` carries until the engine of the remote takes it.
//!
//! - `actions`: what the app's actions on the remote run on, the same for the app's commands and the E2E server's routes
//!   (`RemoteActions`: the Patronus service with the Space repository, git, the sessions with the tables of their last
//!   syncs, the landing gate of the host); the flows are its methods
//! - `facts`: the facts of a session, each with the number of the observation that answered it (`Seq`, one counter per
//!   session) and the state of a fact without an answer (`FactRecord`); `FactStore`, the table of the last sync and the
//!   robots of Patronus (`RobotsFact`, by id), and what the poll of the robots asks about (`robots_digest`)
//! - `branch_table`: the branch table a sync left: the branches in the order of the table, their tips and copies, the
//!   stack branches, the parked branches, the remote facts of the sync (`remote_facts`), `stack_ancestors`
//! - `landing`: the landing gate a start and its popover check through (`LandingGate`), which the hosts answer with the
//!   trial check
//! - `patch`: the patches of a view by part, and the contract of applying one (`apply_patch`): only to the view at its
//!   base, in the same epoch
//! - `patronus_view`: what the robots land, landed and hold of the table, by the hashes and else by content
//!   (`sync_core::robot_landing`), once per state (`ContentMemo`)
//! - `runtime`: the actor of a session: its mailbox, the runs of its syncs with the mailbox as their reporter, the
//!   patches at most every 16 ms; `GitSessionHost`, the syncs of the app's sessions with git
//! - `push_guard`: the checks of the app's push under the repository lock, by the rules of `bd push`: parked, landed
//!   whole, held by a running Safe Push; the words of the holds for both sides; the sequence of the push (`push_branches`,
//!   in the session of its key)
//! - `session`: the sessions of the process by worktree and branch prefix (`Sessions`, `SessionKey`): open, subscribe,
//!   snapshot, focus, the demand of each window (`set_demand`), sync tickets, `apply` and `push` (the result of an
//!   operation, with the version of its patch), and their removal once idle (`sweep`, on the session clock); the
//!   timers of a session wait on its clock (`runtime::sleep_until`); the events of the sessions for the scheduler of the
//!   facts (`events`, `SessionEvent`), which reads with `begin` and applies with `apply_facts`; `write_robot`
//! - `start_guard`: the checks of the app's start, restart and cancel of a robot, by the rules of `bd safe-push`: parked,
//!   landed whole, not built, a robot on its last commit or one that lands a part of it; the words of the robot refusals
//!   for both sides, and the whole sequence of the action (`start_robot`, `restart_robot`, `cancel_robot`) and of the
//!   landing check of the popover (`check_landing`)
//! - `state`: what a session sent its windows of the table (`View`): the patch of the parts that changed since
//!   (`diff`), the snapshot at the version of the last patch
//! - `sync_scheduler`: when a session syncs: one run at a time, one quiet follow-up, one check of the inputs after the
//!   chain for a focus, one silent retry
//! - `table`: the branch table of a session as the events of its syncs leave it (`TableState::apply`), in the parts of
//!   `domain_types::session`, and what its own operations wrote, which no older run undoes: the outcome of a push
//!   (`begin_push`, `end_push`), a deleted archived or unapplied branch; `bd sync` folds its events into one too and
//!   summarizes it
//! - `views`: what the app keeps to answer `get_repository_view`: the table of the last sync and the memo, by repository

pub mod actions;
pub mod branch_table;
pub mod facts;
pub mod landing;
pub mod patch;
pub mod patronus_view;
pub mod push_guard;
pub mod runtime;
pub mod session;
pub mod start_guard;
pub mod state;
pub mod sync_scheduler;
pub mod table;
pub mod views;

//! What Rust derives of the branch table, so the page and `bd` read one rule (docs/design/git-operations.md,
//! "Session facts"). The session of a repository keeps the table of its last sync and derives the Patronus view of it
//! as a part of its view; `bd` builds the table from the report of its own sync or from a grouping, and derives the same
//! view (`facts::derive_patronus`).
//!
//! The crate grows into the model of a repository (docs/design/rust-domain-authority.md): the sessions of the process
//! own the table of each worktree and run its syncs (`session`, `runtime`, `sync_scheduler`), and hold its facts
//! (`facts`): the table of the last sync, the robots of Patronus and what they land, landed and hold with the landing
//! needs, the pull requests of GitHub with the rules of their bases, the merge profiles, the review readiness, the review
//! requests of others and the details of the robots a window demands (the scheduler of `remote-facts` reads them by the
//! rules of `domain-policy`, the session orders the answers and the app's writes). Besides them: the table of a sync
//! (`branch_table`), the rule of what the robots land (`patronus_view`) and the check of the app's push under the lock
//! (`push_guard`). It holds what the checks and the remote services found and runs no check and asks no remote service
//! itself (the hosts answer the landing needs); the actions on the remote are the engine's (`operations`).
//!
//! - `action_view`: the actions of each subject of the view (`actions_of`: every branch, whole stack and robot, by
//!   `domain_policy::actions`), derived with every patch over the next steps of the same patch
//! - `facts`: the facts of a session, each with the number of the observation that answered it (`Seq`, one counter per
//!   session) and the state of a fact without an answer (`FactRecord`); `FactStore`, the table of the last sync, the
//!   robots of Patronus (`RobotsFact`, by id) with the details a window demands, and the Patronus view with the landing
//!   needs (`PatronusFact`, `derive_patronus`), and what the poll of the robots asks about (`robots_digest`)
//! - `branch_table`: the branch table a sync left: the branches in the order of the table, their tips and copies, the
//!   stack branches, the parked branches, the remote facts of the sync (`RemoteFacts`, `remote_facts`), the input of
//!   the rules over them (`repository`)
//! - `derive`: what the view derives of the table by the rules of `domain-policy`: the input of the rules
//!   (`repository`), the structure of each branch with its section (`place`)
//! - `moved_remote`: the remote branches someone else pushed to: which ones a listing of the pull requests asks to fetch
//!   (`MovedFetches`, one fetch per head they show) and the host's fetch of them (`fetch_asked`)
//! - `one_shot`: the session of one command (`OneShot`, `bd`'s): the table of its own sync and the facts it reads for
//!   one intent, in a `FactStore` as a session holds them, the Patronus view derived on the caller's thread
//! - `patch`: the patches of a view by part, and the contract of applying one (`apply_patch`): only to the view at its
//!   base, in the same epoch
//! - `patronus_view`: what the robots land, landed and hold of the table, by the hashes and else by content
//!   (`sync_core::robot_landing`), once per state (`ContentMemo`)
//! - `pull_request_facts`: the pull requests of a session by branch and stack, as the listings and the app's writes
//!   (`PullRequestWrite`, `write_pull_request`) leave them in their order, and the pushed heads a listing checks
//! - `readiness_facts`: the review readiness of a session by subject: the ask of each from the view (its tip, the pull
//!   request it starts from, the reviews), what the scheduler asks by the demand (`ReadinessDigest`), the answers in
//!   their order, the news that approvals arrived
//! - `review_request_facts`: the pull requests of others that wait for the user's review, as the listings leave them,
//!   and the news of the requests a listing added
//! - `refusal_words`: the words of the refusals of the rules for `bd`, the engine and the push guard
//!   (`start_refusal_text`), and why the stack branch of a whole stack is no build of its commits
//! - `runtime`: the actor of a session: its mailbox, the runs of its syncs with the mailbox as their reporter, the
//!   patches at most every 16 ms; `GitSessionHost`, the syncs, the Patronus views and the landing needs of the app's
//!   sessions with git
//! - `push_guard`: the checks of the app's push under the repository lock, by the rules of `bd push`: parked, landed
//!   whole, held by a running Safe Push; the words of the holds for both sides (the push itself is `operations::push`)
//! - `session`: the sessions of the process by worktree and branch prefix (`Sessions`, `SessionKey`): open, subscribe,
//!   snapshot, focus, the demand of each window (`set_demand`), sync tickets, `apply` (the result of an operation, with
//!   the version of its patch), `begin_push` and `tell_push` (a push that sends and how it ended), `apply_with` (a
//!   change on the actor that answers, as the registry of the operations admits one) and `hold` (a session kept while an
//!   operation runs), and their removal once idle (`sweep`, on the session clock); the
//!   timers of a session wait on its clock (`runtime::sleep_until`); the events of the sessions for the scheduler of the
//!   facts (`events`, `SessionEvent`), which reads with `begin` and applies with `apply_facts`; `write_robot`,
//!   `write_pull_request`
//! - `state`: what a session sent its windows of the table (`View`): the patch of the parts that changed since
//!   (`diff`), the snapshot at the version of the last patch
//! - `sync_scheduler`: when a session syncs: one run at a time, one quiet follow-up, one check of the inputs after the
//!   chain for a focus, one silent retry
//! - `table`: the branch table of a session as the events of its syncs leave it (`TableState::apply`), in the parts of
//!   `domain_types::session`, and what its own operations wrote, which no older run undoes: the outcome of a push
//!   (`begin_push`, `end_push`), a deleted archived or unapplied branch; the operations of the remote that run or wait
//!   (`operations`, `apply_operation`), which only `operations::registry` changes; `bd sync` folds its events into one
//!   too and summarizes it

pub mod action_view;
pub mod branch_table;
pub mod derive;
pub mod facts;
pub mod moved_remote;
pub mod one_shot;
pub mod patch;
pub mod patronus_view;
pub mod pull_request_facts;
pub mod push_guard;
pub mod readiness_facts;
pub mod ref_claims;
pub mod refusal_words;
pub mod review_request_facts;
pub mod runtime;
pub mod session;
pub mod state;
pub mod sync_scheduler;
pub mod table;

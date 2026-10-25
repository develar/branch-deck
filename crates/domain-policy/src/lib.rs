//! The rules of the domain that the app and `bd` decide alike, as pure functions over explicit slices of facts: no IO,
//! no runtime, no git, only the types of `domain-types` (docs/design/rust-domain-authority.md, "Индекс правил"; the
//! architecture test keeps the crate away from git, the network, tokio, Tauri and clap). One file per rule, each tested
//! by tables of its own:
//!
//! - `facts`: the input of the rules, the branch table as they read it (`Repository`), borrowed from the view of a
//!   session or from the grouped branches of a sync, the robots of the user by id (`Robots`) and what is known of a
//!   remote branch (`RemoteBranch`)
//! - `holds`: what holds the push of a branch (`branch_hold`: landed, its own Safe Push, one that lands it) and of the
//!   stack branch of a whole stack (`stack_branch_hold`), the push an action can't make for the user (`push_blocker`),
//!   and what a restart of a robot does (`restart_action`)
//! - `land`: why a Safe Push or a Dry Run of a branch or a whole stack can't start (`start_refusal`: blocked, a robot on
//!   the last commit, one that lands it or a part of it, the hold of the push of a Dry Run), and a restart
//!   (`restart_refusal`); the decision of a start (`decide`: its blockers, the confirmations the user accepts, the facts
//!   it waits for and its effects, retarget, cancel, push and the start), what the table alone refuses
//!   (`table_blocker`), the confirmations `bd` accepts by a flag (`unaccepted`), the modes a profile allows
//!   (`allows_mode`) and the default profile (`default_profile`)
//! - `landing_needs`: the running Safe Push a missing declaration of the landing needs names (`name_landing_robots`)
//! - `parked`: why a branch stays local (`parked_state`: its own line, or a parked branch it is built on), and the
//!   parked member that keeps a stack branch local (`whole_stack_parked`)
//! - `push`: what a push of a branch does to its remote branch (`action`: create, push, force, up to date), whether it
//!   has work, replaces older versions or drops remote commits, whether a push over a remote that moved goes again
//!   (`over_moved_remote`), what a push of several branches covers and what each of them does (`push_plan`: Push Stack,
//!   Push with Children), the pushes of a branch the view sends (`branch_push`) and the stack branch Push with Children
//!   of a root takes along (`stack_branch_along`)
//! - `pull_request`: where an open pull request goes (`base_of`: the target, a virtual branch it leaves for the target,
//!   another base that may be meant), the pull request a robot starts from (`start_pull_request`), and what a push does
//!   to its approvals by the rules of its base (`dismisses_approvals`)
//! - `reasons`: the reasons the rules give (`domain_types::reason::Reason`), with the facts their words need
//! - `readiness`: when an answer of the review readiness is the news that approvals arrived (`approval_news`), and
//!   whether a subject waits for them (`awaits_approval`)
//! - `review`: whom Request Review… offers and `bd request-review` asks (`candidates`), what an answer of the readiness
//!   means for a start and where the approvals count, a repository only on Space read once (`review_state`), and when a
//!   Safe Push starts without them (`skips_approvals`)
//! - `refresh`: when a fact of the remote is asked again: the cadence of the robots of Patronus (the list, the poll, the
//!   failing robots, the problems, the details), of the merge profiles, of the pull requests and the branch rules of
//!   GitHub, of the review readiness and of the review requests, by the reason that asks and the clock of the session
//! - `robot_match`: which robot belongs to a virtual branch and which to the whole stack of a root (`RobotIndex`,
//!   `match_table`)
//! - `stack`: the stacks of a table: the declared parent (D8), the branch a branch is built on, the ancestors by
//!   effective base, the children and the descendants, the main line, the whole stack of a branch by declared parent
//!   (`whole_stack_of`), its branches as a sentence names them and what is left of it to land (`whole_stack_rest`)

pub mod facts;
pub mod holds;
pub mod land;
pub mod landing_needs;
pub mod parked;
pub mod pull_request;
pub mod push;
pub mod readiness;
pub mod reasons;
pub mod refresh;
pub mod review;
pub mod robot_match;
pub mod stack;

#[cfg(test)]
mod holds_test;
#[cfg(test)]
mod land_test;
#[cfg(test)]
mod landing_needs_test;
#[cfg(test)]
mod parked_test;
#[cfg(test)]
mod pull_request_test;
#[cfg(test)]
mod push_test;
#[cfg(test)]
mod readiness_test;
#[cfg(test)]
mod reason_catalog_test;
#[cfg(test)]
mod refresh_test;
#[cfg(test)]
mod review_test;
#[cfg(test)]
mod robot_match_test;
#[cfg(test)]
mod stack_test;

//! The one backend of a user (docs/design/backend-protocol.md; openspec/specs/repository/backend-protocol/spec.md): an
//! AHP-shaped host of app-api, in the process of the desktop app, for every client: the Tauri shell over the
//! in-process transport, the bridge of the IDE (`branch-deck ide`) over a local socket, and later brd. It reaches no
//! Tauri (`architecture_test.rs` of `bd-cli`).
//!
//! - `host`: the host over the `AppContext` of the process: the connections, the global `serverSeq` and its publish
//!   lock, the channels of the repositories, the news claimed, the hooks of the app.
//! - `connection`: a connection: its lines, `initialize`, the life of its windows, its host requests, its end.
//! - `channel`: the repository channels: the subscriptions of the windows, `viewPatched`, the resync, the end of a
//!   session, the demand, the fence.
//! - `call`: the root commands over `app_api::dispatch`, their call events and their cancel.
//! - `client`: the desktop and the events of a client: its host requests and its capabilities.
//! - `outbox`: the ordered queue of a connection and the cap of a window that reads too slowly.
//! - `wire`: the frames, the errors of the protocol, the version of AHP.
//! - `identity`: the build of the binary and the rule of another build.
//! - `in_process` and `stream`: the transports in the process and over a stream of bytes.
//! - `socket`: the local socket of a key (a named pipe on Windows), its owner-only directory, `backend.lock`, the check
//!   of the user of the peer, and `showMain` of a second start.

pub mod call;
pub mod channel;
pub mod client;
pub mod connection;
pub mod host;
pub mod identity;
pub mod in_process;
pub mod outbox;
pub mod socket;
pub mod stream;
pub mod wire;

#[cfg(test)]
mod corpus_test;

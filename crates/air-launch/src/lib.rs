//! The local app client of the local AHP socket of Air (openspec/specs/branch/start-in-air/spec.md): Branch Deck starts
//! an agent session in the IDE that has the repository open, and follows its row. No git, no network and no runtime:
//! synchronous JSON-RPC over a Unix-domain socket, which a caller runs on a blocking thread.
//!
//! - `discovery`: the instance files of the running IDEs, their hosts, and the IDE of a directory
//! - `protocol`: the lines of the handshake, the frames of JSON-RPC, the results and the refusals
//! - `client`: the handshake with `initialize`, then the requests of `ahp-root://` over the socket of an IDE, each with
//!   the timeout of its answer
//! - `watch`: the rows of an IDE and their changes over one connection

pub mod client;
pub mod discovery;
pub mod protocol;
pub mod watch;

#[cfg(test)]
mod client_test;
#[cfg(test)]
mod discovery_test;
#[cfg(test)]
mod fake_ide_test;
#[cfg(test)]
mod protocol_test;

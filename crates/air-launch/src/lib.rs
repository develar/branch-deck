//! The client of the local launch socket of Air, the local peer handshake v2 and the requests of protocol v1
//! (spec/branch/start-in-air.spec.md, IJAI-5465): Branch Deck starts an agent session in the IDE that has the repository
//! open. No git, no network and no runtime: one synchronous exchange over a Unix-domain socket, which a caller runs on a
//! blocking thread.
//!
//! - `discovery`: the instance files of the running IDEs, and the IDE of a directory
//! - `protocol`: the lines of the handshake, the requests, the responses and the refusal codes
//! - `client`: the handshake, then one request and one response over the socket of an IDE, with the timeout of the
//!   response

pub mod client;
pub mod discovery;
pub mod protocol;

#[cfg(test)]
mod client_test;
#[cfg(test)]
mod discovery_test;
#[cfg(test)]
mod protocol_test;

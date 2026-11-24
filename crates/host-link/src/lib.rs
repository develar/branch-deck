//! The link of the remote runs (openspec/specs/cli/host/spec.md, "The link"; docs/design/host-runs.md): what the client and the
//! agent of `brd-host` share. No git: the lease pool and git stay with the agent and the client. The link runs on tokio,
//! whose runtime the client and the agent own.
//!
//! - `identity`: the self-signed certificate of one end and its fingerprint
//! - `allow`: the fingerprints of the clients that an agent accepts
//! - `channel`: TLS 1.3 over TCP, each end pinned by its fingerprint, and the frames on it, with a deadline on each read
//!   and write
//! - `relay`: the connection through `/usr/bin/nc` that Local Network privacy of macOS lets pass (`disclaim`), its pipes
//!   as one stream
//! - `proto`: the protocol host-link, its schema, its port, its requests and replies, and the features of an agent
//! - `refusal`: the refusal codes of brd-host and their exit codes
//! - `config`: `[host]` of `.branchdeck.toml` of a commit, which both ends parse
//! - `heavy`: what the command of a run runs, through the script of a shell, for `heavy` of `[host]`
//! - `digest`: the SHA-256 of a binary or a certificate as hex, the name of a build and a fingerprint
//! - `tls`: TLS 1.3 on the provider of the app, and the self-signed certificate of an end

pub mod allow;
pub mod channel;
pub mod config;
pub mod digest;
#[cfg(target_os = "macos")]
#[expect(unsafe_code, reason = "posix_spawn with the private disclaim attribute of libSystem, which std cannot pass")]
mod disclaim;
pub mod heavy;
pub mod identity;
pub mod proto;
pub mod refusal;
pub mod relay;
pub mod tls;

#[cfg(test)]
mod digest_test;
#[cfg(test)]
mod heavy_test;
#[cfg(test)]
mod proto_test;
#[cfg(test)]
mod tls_test;

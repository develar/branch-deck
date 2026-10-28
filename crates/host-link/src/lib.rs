//! The link of the remote runs (spec/cli/host.spec.md, "The link"; docs/design/host-runs.md): what the client and the
//! agent of `bd-host` share. No git and no runtime: the lease pool and git stay with the agent and the client.
//!
//! - `digest`: the SHA-256 of a binary or a certificate as hex, the name of a build and a fingerprint
//! - `proto`: the protocol host-link, its schema, its port and the features of an agent
//! - `tls`: TLS 1.3 on the provider of the app, and the self-signed certificate of an end
//!
//! The port of `yonder-link` (`community/tools/yonder` of the IntelliJ monorepo) adds the identity files, the TLS 1.3
//! channel with pinned certificates, the frames, the allow list, the relay through `/usr/bin/nc`, `[host]` of
//! `.branchdeck.toml` and the refusal codes.

pub mod digest;
pub mod proto;
pub mod tls;

#[cfg(test)]
mod digest_test;
#[cfg(test)]
mod proto_test;
#[cfg(test)]
mod tls_test;

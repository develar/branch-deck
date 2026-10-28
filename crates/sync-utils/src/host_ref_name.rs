//! The id of a remote run and the name of a base of a host (spec/cli/host.spec.md, "The link"; spec/cli/lease.spec.md,
//! "Host objects"). Each one names one ref component under `refs/branch-deck/host/` and one directory, so the client,
//! the protocol and the agent check it with this one rule

/// The most characters of the id of a run or the name of a base
pub const MAX_HOST_REF_NAME: usize = 128;

/// Whether `name` may be the id of a run or the name of a base under `refs/branch-deck/host/`: 1 to 128 characters of
/// `A-Z a-z 0-9 . _ -`, the first a letter or a digit, no `..`, not ending in `.lock`, so the ref is valid and one
/// component
pub fn is_host_ref_name(name: &str) -> bool {
  name.len() <= MAX_HOST_REF_NAME
    && name.bytes().next().is_some_and(|first| first.is_ascii_alphanumeric())
    && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    && !name.contains("..")
    && !name.to_ascii_lowercase().ends_with(".lock")
}

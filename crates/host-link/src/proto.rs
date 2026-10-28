//! The protocol host-link: one hello and one request per connection, over TLS 1.3 (spec/cli/host.spec.md, "The link")

/// The schema of the protocol. A change that an older end cannot read raises it. The hello of another schema refuses
/// with `schema`
pub const SCHEMA_VERSION: u32 = 1;

/// The TCP port of `bd-host serve`. yonderd listens on 7741, so both agents serve one host side by side
pub const DEFAULT_PORT: u16 = 7742;

/// The label of the `LaunchAgent` that starts `bd-host serve` on a macOS host
pub const LAUNCHD_LABEL: &str = "com.jetbrains.bd-host";

/// What an agent does beyond the base of `SCHEMA_VERSION`, named in its hello and in `bd-host version`
pub mod feature {
  /// The upgrade request and the watchdog: only on macOS, because only launchd starts the new binary
  pub const UPGRADE: &str = "upgrade";
}

/// The features of an agent of this build on this platform: `upgrade` on macOS
pub fn features() -> Vec<&'static str> {
  if cfg!(target_os = "macos") { vec![feature::UPGRADE] } else { Vec::new() }
}

//! The git of a host that serves remote runs (`bd lease host git`, spec/cli/lease.spec.md, "Host objects"): the floor
//! of its version, above `MIN_VERSION` of bd itself. The runs of other machines and the agent of the host use the git
//! bd discovers (`GitInfo::discover`, Homebrew first), so a host keeps one floor that moves only on purpose instead of
//! a check of each feature they use. The floor is the newest release of git when the user set the policy (2026-10-27)

/// The lowest git of a host
pub const HOST_GIT_FLOOR: (u32, u32, u32) = (2, 56, 0);

/// `HOST_GIT_FLOOR` as git prints a version
pub fn floor_text() -> String {
  let (major, minor, patch) = HOST_GIT_FLOOR;
  format!("{major}.{minor}.{patch}")
}

/// Whether `version` (`2.56.0`, `2.56.1.windows.1`, `2.57.0-rc1`, as `git version` prints it) is at the floor or above;
/// a release candidate comes before its release, and a version that does not parse is below it
pub fn meets_floor(version: &str) -> bool {
  let parts: Vec<&str> = version.split(['.', '-']).collect();
  let number = |index: usize| parts.get(index).and_then(|part| part.parse::<u32>().ok());
  let (Some(major), Some(minor)) = (number(0), number(1)) else {
    return false;
  };
  let patch = number(2).unwrap_or(0);
  let candidate = parts.iter().skip(2).any(|part| part.starts_with("rc"));
  let version = (major, minor, patch);
  version > HOST_GIT_FLOOR || (version == HOST_GIT_FLOOR && !candidate)
}

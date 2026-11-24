use crate::start::{Reached, StartFacts, Step, Why, app_environment, app_of, decide};
use pretty_assertions::assert_eq;
use std::ffi::OsString;

/// The facts of a bridge that finds no backend, on a machine with a display and the app beside it
const NOTHING: StartFacts = StartFacts {
  in_process: false,
  reached: Reached::Nothing,
  lock_held: false,
  display: true,
  app: true,
  killed: false,
};

/// The start of the bridge by what it finds: it joins a backend of its build, waits for one that starts, starts the app
/// when none runs, lets a stale backend of another build exit first, and serves the IDE alone where the app cannot run
/// or another build keeps a window open
#[test]
fn the_bridge_joins_waits_starts_the_app_or_serves_alone_by_what_it_finds() {
  let cases = [
    (
      "a backend of its build",
      StartFacts {
        reached: Reached::Joined,
        lock_held: true,
        ..NOTHING
      },
      Step::Join,
    ),
    ("an app that starts holds the lock", StartFacts { lock_held: true, ..NOTHING }, Step::AwaitSocket),
    ("no backend", NOTHING, Step::StartApp),
    (
      "a stale backend without a window exits",
      StartFacts {
        reached: Reached::OtherBuild { exits: true },
        lock_held: true,
        ..NOTHING
      },
      Step::AwaitExit,
    ),
    (
      "another build with a window open stays",
      StartFacts {
        reached: Reached::OtherBuild { exits: false },
        lock_held: true,
        ..NOTHING
      },
      Step::Fallback(Why::OtherBuild),
    ),
    (
      "another version of the protocol",
      StartFacts {
        reached: Reached::Refused,
        ..NOTHING
      },
      Step::Fallback(Why::Refused),
    ),
    (
      "Linux without DISPLAY and WAYLAND_DISPLAY",
      StartFacts { display: false, ..NOTHING },
      Step::Fallback(Why::NoDisplay),
    ),
    (
      "an app that starts needs no display of the bridge",
      StartFacts {
        display: false,
        lock_held: true,
        ..NOTHING
      },
      Step::AwaitSocket,
    ),
    ("no binary of the app", StartFacts { app: false, ..NOTHING }, Step::Fallback(Why::NoApp)),
    (
      "the system stopped the app in the background lately",
      StartFacts { killed: true, ..NOTHING },
      Step::Fallback(Why::KilledInBackground),
    ),
    (
      "an app that runs is joined after a kill too",
      StartFacts {
        reached: Reached::Joined,
        lock_held: true,
        killed: true,
        ..NOTHING
      },
      Step::Join,
    ),
    (
      "an app the user starts after a kill is waited for",
      StartFacts {
        lock_held: true,
        killed: true,
        ..NOTHING
      },
      Step::AwaitSocket,
    ),
    (
      "the tests ask for it",
      StartFacts {
        in_process: true,
        reached: Reached::Joined,
        ..NOTHING
      },
      Step::Fallback(Why::Asked),
    ),
  ];
  for (case, facts, step) in cases {
    assert_eq!(decide(&facts), step, "{case}");
  }
  assert!(Why::BreakawayDenied.may_come_back(), "the app started later is joined at the next start of the bridge");
  assert!(Why::KilledInBackground.may_come_back(), "the app the user opens after a kill is joined");
  assert!(!Why::Asked.may_come_back());
}

/// The app the bridge starts gets the environment of the user, none of Air or of the IDE
#[test]
fn the_app_the_bridge_starts_gets_no_variable_of_air_or_of_the_ide() {
  let of = |pairs: &[(&str, &str)]| pairs.iter().map(|(name, value)| (OsString::from(name), OsString::from(value))).collect::<Vec<_>>();
  let variables = of(&[
    ("HOME", "/home/u"),
    ("PATH", "/usr/bin"),
    ("IDEA_VM_OPTIONS", "/x"),
    ("JETBRAINS_CLIENT_ID", "1"),
    ("AIR_APPS_TOKEN", "t"),
    ("JAVA_TOOL_OPTIONS", "-X"),
    ("__CFBundleIdentifier", "com.jetbrains.intellij"),
    ("BRANCH_DECK_IDE_BACKEND", "in-process"),
    ("LANG", "en_US.UTF-8"),
  ]);
  assert_eq!(app_environment(variables), of(&[("HOME", "/home/u"), ("PATH", "/usr/bin"), ("LANG", "en_US.UTF-8")]));
}

/// The app of the bridge is its own binary, but for `brd`, whose app is the binary beside it
#[test]
fn the_app_of_brd_is_the_binary_beside_it() {
  let directory = tempfile::tempdir().unwrap();
  let app = directory.path().join(if cfg!(windows) { "branch-deck.exe" } else { "branch-deck" });
  let brd = directory.path().join(if cfg!(windows) { "brd.exe" } else { "brd" });
  assert_eq!(app_of(&brd), None, "no app beside brd");
  std::fs::write(&app, b"").unwrap();
  std::fs::write(&brd, b"").unwrap();
  let resolved = std::fs::canonicalize(&app).unwrap();
  assert_eq!(app_of(&brd), Some(resolved.clone()));
  assert_eq!(app_of(&app), Some(resolved));
}

use std::path::PathBuf;

use pretty_assertions::assert_eq;

use super::*;
use host_link::config::EnvValue;
use std::net::TcpListener;

fn spec() -> RunSpec {
  RunSpec {
    run: "1-2".to_owned(),
    affinity: "checkout-h".to_owned(),
    commit: "c".repeat(40),
    ..RunSpec::default()
  }
}

fn var<'a>(env: &'a BTreeMap<OsString, OsString>, name: &str) -> Option<&'a str> {
  env.get(&OsString::from(name)).and_then(|value| value.to_str())
}

#[test]
fn the_number_of_a_slot_comes_from_the_pool_directory_or_from_the_path() {
  assert_eq!(slot_number(Path::new("/pool/branch-deck-1234/7/branch-deck")), 7);
  let hashed = slot_number(Path::new("/elsewhere/slots/holder"));
  assert!(hashed < 100, "{hashed}");
  assert_eq!(slot_number(Path::new("/elsewhere/slots/holder")), hashed);
  assert!(slot_number(Path::new("/pool/99999/repo")) < 100);
}

#[test]
fn a_port_variable_skips_a_port_that_answers() {
  let held = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a listener");
  let port = held.local_addr().expect("its address").port();
  let found = free_port(port).expect("a free port");
  assert_ne!(found, port);
  assert!(found > port && found < port.saturating_add(PORT_TRIES), "{found}");
}

#[test]
fn the_variables_of_the_run_and_of_the_config_go_on_top_of_the_base() {
  let home = Home::new(PathBuf::from("/home/u"));
  let base: BTreeMap<OsString, OsString> = [("PATH", "/usr/bin:/bin"), ("CI", "1"), ("KEEP", "k")]
    .into_iter()
    .map(|(name, value)| (OsString::from(name), OsString::from(value)))
    .collect();
  let config = vec![
    ("CI".to_owned(), EnvValue::Unset),
    (
      "NEW_LIST".to_owned(),
      EnvValue::List {
        prepend: vec!["/a".to_owned()],
        append: vec!["tools.local/bin".to_owned()],
      },
    ),
    (
      "PATH".to_owned(),
      EnvValue::List {
        prepend: vec!["~/.cargo/bin".to_owned()],
        append: vec!["/opt/last".to_owned()],
      },
    ),
    ("PORT".to_owned(), EnvValue::Port(23_000)),
    ("TEXT".to_owned(), EnvValue::Text("~/not/expanded".to_owned())),
  ];
  let slot = Path::new("/pool/x/12/repo");
  let env = run_env(base, &config, &home, &spec(), slot, None).expect("the env");
  assert_eq!(var(&env, "PATH"), Some("/home/u/.cargo/bin:/usr/bin:/bin:/opt/last"));
  assert_eq!(var(&env, "NEW_LIST"), Some("/a:/pool/x/12/repo/tools.local/bin"));
  assert_eq!(var(&env, "CI"), None);
  assert_eq!(var(&env, "KEEP"), Some("k"));
  assert_eq!(var(&env, "TEXT"), Some("~/not/expanded"));
  assert_eq!(var(&env, "BD_HOST_RUN"), Some("1-2"));
  assert_eq!(var(&env, "BD_HOST_AFFINITY"), Some("checkout-h"));
  assert_eq!(var(&env, "BD_HOST_SLOT"), Some("12"));
  let port: u16 = var(&env, "PORT").expect("a port").parse().expect("a number");
  assert!((23_012..23_012 + PORT_TRIES).contains(&port), "{port}");
  for name in JOBS_VARIABLES {
    assert_eq!(var(&env, name), None, "a host without branchdeck.hostJobs sets no {name}");
  }
}

/// `branchdeck.hostJobs` of a host gives each command its count of jobs, and `[host.env]` of the commit still decides
#[test]
fn the_jobs_of_the_host_go_to_the_command_under_the_config_of_the_commit() {
  let home = Home::new(PathBuf::from("/home/u"));
  let config = vec![("NEXTEST_TEST_THREADS".to_owned(), EnvValue::Text("2".to_owned()))];
  let env = run_env(BTreeMap::new(), &config, &home, &spec(), Path::new("/pool/x/3/repo"), Some(6)).expect("the env");
  assert_eq!(
    JOBS_VARIABLES.map(|name| var(&env, name)),
    [Some("6"), Some("2"), Some("6")],
    "CARGO_BUILD_JOBS, NEXTEST_TEST_THREADS of [host.env], BRD_HOST_JOBS"
  );
}

#[test]
fn the_open_files_limit_goes_up_to_the_target_or_the_hard_limit() {
  let soft = raise_open_files().expect("the limit");
  let (current, hard) = crate::sys::open_files().expect("the limits");
  assert_eq!(current, soft);
  assert!(soft >= OPEN_FILES.min(hard).min(crate::sys::max_files_per_process()), "{soft}");
  assert!(load().is_some_and(|load| load >= 0.0));
}

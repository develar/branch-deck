use super::*;
use pretty_assertions::assert_eq;

/// A checkout in a temporary directory whose git directory holds run records: `(run, host, ended)` each
fn checkout_with_runs(runs: &[(&str, &str, bool)]) -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  let records = host_runs_dir(&dir.path().join(".git"));
  std::fs::create_dir_all(&records).unwrap();
  for (run, host, ended) in runs {
    std::fs::write(records.join(format!("{run}.json")), "{}").unwrap();
    std::fs::write(records.join(format!("{run}.host")), format!("{host}\n")).unwrap();
    if *ended {
      std::fs::write(records.join(format!("{run}.end.json")), "{}").unwrap();
    }
  }
  dir
}

/// The runs without an end are the ones of a `<run>.host` without `<run>.end.json`, in the order of their ids, with the
/// alias of each host; a record without `<run>.host`, and a checkout without run records, name none
#[test]
fn the_unfinished_runs_are_the_records_of_a_host_without_an_end() {
  let checkout = checkout_with_runs(&[("2-2", "studio", false), ("1-1", "studio", true), ("3-3", "local", false)]);
  std::fs::write(host_runs_dir(&checkout.path().join(".git")).join("4-4.json"), "{}").unwrap();
  assert_eq!(
    unfinished_runs(checkout.path()),
    Unfinished {
      runs: vec![("2-2".to_owned(), "studio".to_owned()), ("3-3".to_owned(), "local".to_owned())],
      unreadable: Vec::new(),
    }
  );
  let empty = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(empty.path().join(".git")).unwrap();
  assert_eq!(unfinished_runs(empty.path()), Unfinished::default(), "a slot without the run directory has none");
}

/// Without a `brd-host` each run without an end stays, with the code `no_brd_host` and the command that cancels it
#[test]
fn without_a_brd_host_the_runs_stay_with_the_command_that_cancels_each() {
  let checkout = checkout_with_runs(&[("1-1", "studio", false)]);
  let runs = cancel_unfinished_runs(checkout.path(), &HostCancel::NONE);
  assert_eq!(runs.cancelled, Vec::new());
  assert_eq!(runs.not_cancelled.len(), 1, "{runs:?}");
  let left = &runs.not_cancelled[0];
  assert_eq!((left.run.as_str(), left.host.as_str(), left.code.as_str()), ("1-1", "studio", "no_brd_host"));
  assert!(left.message.ends_with("`brd-host cancel studio 1-1` cancels it"), "{}", left.message);
}

/// The binary of the agent is `brd-host` itself; any other binary has the `brd-host` beside it, by its real path, or
/// none
#[cfg(unix)]
#[test]
fn the_brd_host_of_a_binary_is_itself_or_the_one_beside_it() {
  let dir = tempfile::tempdir().unwrap();
  let bundle = dir.path().join("bundle");
  std::fs::create_dir_all(&bundle).unwrap();
  std::fs::write(bundle.join("app"), "").unwrap();
  let link = dir.path().join("brd");
  std::os::unix::fs::symlink(bundle.join("app"), &link).unwrap();
  assert_eq!(HostCancel::of_binary(Some(&link)).program, None, "no brd-host beside the app");
  std::fs::write(bundle.join(BRD_HOST), "").unwrap();
  let bundle = std::fs::canonicalize(&bundle).unwrap();
  assert_eq!(
    HostCancel::of_binary(Some(&link)).program,
    Some(bundle.join(BRD_HOST)),
    "the one beside the target of the link"
  );
  assert_eq!(HostCancel::of_binary(Some(&bundle.join(BRD_HOST))).program, Some(bundle.join(BRD_HOST)));
  assert_eq!(HostCancel::of_binary(None), HostCancel::NONE);
}

/// A fake `brd-host` in `dir` that writes its arguments and its working directory beside itself, then runs `body`
#[cfg(unix)]
fn fake_brd_host(dir: &Path, body: &str) -> PathBuf {
  use std::os::unix::fs::PermissionsExt;
  let program = dir.join(BRD_HOST);
  std::fs::write(&program, format!("#!/bin/sh\necho \"$@\" > \"$0.args\"\npwd -P > \"$0.cwd\"\n{body}\n")).unwrap();
  std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
  program
}

/// A slot with a run without an end runs `brd-host cancel-unfinished --json --max <max>` in the slot and answers its
/// JSON; a slot without one starts no process
#[cfg(unix)]
#[test]
fn the_cancel_runs_brd_host_in_the_slot_only_for_a_run_without_an_end() {
  let bin = tempfile::tempdir().unwrap();
  let program = fake_brd_host(
    bin.path(),
    r#"echo '{"schema":1,"cancelled":[{"run":"2-2","host":"studio","answer":"the supervisor of the run 2-2 has the cancel request"}],"not_cancelled":[]}'"#,
  );
  let cancel = HostCancel {
    program: Some(program),
    max: Duration::from_secs(5),
  };
  let ended = checkout_with_runs(&[("1-1", "studio", true)]);
  assert_eq!(cancel_unfinished_runs(ended.path(), &cancel), HostRuns::default());
  assert!(!bin.path().join("brd-host.args").exists(), "no process for a slot whose runs all ended");

  let checkout = checkout_with_runs(&[("1-1", "studio", true), ("2-2", "studio", false)]);
  let runs = cancel_unfinished_runs(checkout.path(), &cancel);
  assert_eq!(
    runs,
    HostRuns {
      cancelled: vec![CancelledRun {
        run: "2-2".to_owned(),
        host: "studio".to_owned(),
        answer: "the supervisor of the run 2-2 has the cancel request".to_owned(),
      }],
      not_cancelled: Vec::new(),
    }
  );
  assert_eq!(std::fs::read_to_string(bin.path().join("brd-host.args")).unwrap(), "cancel-unfinished --json --max 5s\n");
  let cwd = std::fs::read_to_string(bin.path().join("brd-host.cwd")).unwrap();
  assert_eq!(Path::new(cwd.trim()), std::fs::canonicalize(checkout.path()).unwrap());
}

/// A `brd-host` that does not end is killed after its `--max` and the grace, and each run stays with `timed_out`; one
/// that answers no JSON leaves each run with `brd_host_failed` and its words
#[cfg(unix)]
#[test]
fn a_brd_host_that_hangs_or_answers_no_json_leaves_each_run() {
  let bin = tempfile::tempdir().unwrap();
  let checkout = checkout_with_runs(&[("1-1", "studio", false), ("2-2", "local", false)]);
  let hangs = HostCancel {
    program: Some(fake_brd_host(bin.path(), "exec sleep 60")),
    max: Duration::from_secs(1),
  };
  let started = std::time::Instant::now();
  let runs = cancel_unfinished_runs(checkout.path(), &hangs);
  let took = started.elapsed();
  assert!(took < Duration::from_secs(30), "the end of the lease waits past the bound of the child: {took:?}");
  let codes: Vec<(&str, &str)> = runs.not_cancelled.iter().map(|run| (run.run.as_str(), run.code.as_str())).collect();
  assert_eq!((runs.cancelled.len(), codes), (0, vec![("1-1", "timed_out"), ("2-2", "timed_out")]));

  let refuses = HostCancel {
    program: Some(fake_brd_host(bin.path(), "echo 'brd-host: config: cannot read the host file' >&2; exit 255")),
    max: Duration::from_secs(1),
  };
  let runs = cancel_unfinished_runs(checkout.path(), &refuses);
  assert_eq!(runs.not_cancelled.len(), 2, "{runs:?}");
  assert_eq!(runs.not_cancelled[0].code, "brd_host_failed");
  assert!(
    runs.not_cancelled[0].message.contains("brd-host: config: cannot read the host file"),
    "{}",
    runs.not_cancelled[0].message
  );
}

/// A `brd-host` that leaves a process with its stdout open answers as soon as it exits: the end of the lease reads the
/// pipes for a moment after the exit, not until that process closes them
#[cfg(unix)]
#[test]
fn a_process_that_inherits_the_pipes_of_brd_host_holds_no_end_of_a_lease() {
  let bin = tempfile::tempdir().unwrap();
  let pid_file = bin.path().join("sleep.pid");
  let program = fake_brd_host(
    bin.path(),
    &format!(
      "sleep 30 &\necho $! > '{}'\necho '{{\"schema\":1,\"cancelled\":[],\"not_cancelled\":[]}}'",
      pid_file.display()
    ),
  );
  let checkout = checkout_with_runs(&[("1-1", "studio", false)]);
  let started = std::time::Instant::now();
  let runs = cancel_unfinished_runs(
    checkout.path(),
    &HostCancel {
      program: Some(program),
      max: Duration::from_secs(5),
    },
  );
  let took = started.elapsed();
  if let Ok(pid) = std::fs::read_to_string(&pid_file) {
    let _ = std::process::Command::new("kill").arg(pid.trim()).status();
  }
  assert_eq!(runs, HostRuns::default());
  assert!(took < Duration::from_secs(8), "the end of the lease waited for the process that holds the pipe: {took:?}");
}

/// A run directory or a `<run>.host` that cannot be read is no slot without runs: each stays in `not_cancelled` with
/// `records_unreadable`, and the readable records are cancelled beside it
#[cfg(unix)]
#[test]
fn an_unreadable_record_stays_as_records_unreadable() {
  use std::os::unix::fs::PermissionsExt;
  let bin = tempfile::tempdir().unwrap();
  let program = fake_brd_host(
    bin.path(),
    r#"echo '{"schema":1,"cancelled":[{"run":"2-2","host":"studio","answer":"done"}],"not_cancelled":[]}'"#,
  );
  let cancel = HostCancel {
    program: Some(program),
    max: Duration::from_secs(5),
  };
  let checkout = checkout_with_runs(&[("1-1", "studio", false), ("2-2", "studio", false)]);
  let records = host_runs_dir(&checkout.path().join(".git"));
  let host = records.join("1-1.host");
  std::fs::set_permissions(&host, std::fs::Permissions::from_mode(0o000)).unwrap();
  let runs = cancel_unfinished_runs(checkout.path(), &cancel);
  std::fs::set_permissions(&host, std::fs::Permissions::from_mode(0o600)).unwrap();
  let cancelled: Vec<&str> = runs.cancelled.iter().map(|run| run.run.as_str()).collect();
  let kept: Vec<(&str, &str, &str)> = runs.not_cancelled.iter().map(|run| (run.run.as_str(), run.host.as_str(), run.code.as_str())).collect();
  assert_eq!((cancelled, kept), (vec!["2-2"], vec![("1-1", "", "records_unreadable")]), "{runs:?}");

  std::fs::remove_file(bin.path().join("brd-host.args")).unwrap();
  std::fs::set_permissions(&records, std::fs::Permissions::from_mode(0o000)).unwrap();
  let runs = cancel_unfinished_runs(checkout.path(), &cancel);
  std::fs::set_permissions(&records, std::fs::Permissions::from_mode(0o700)).unwrap();
  assert_eq!(runs.cancelled, Vec::new());
  assert_eq!(runs.not_cancelled.len(), 1, "{runs:?}");
  assert_eq!((runs.not_cancelled[0].run.as_str(), runs.not_cancelled[0].code.as_str()), ("", "records_unreadable"));
  assert!(runs.not_cancelled[0].message.contains("cannot read "), "{}", runs.not_cancelled[0].message);
  assert!(!bin.path().join("brd-host.args").exists(), "no brd-host for a directory that it cannot read either");
}

use super::*;
use crate::record::{RunEnd, RunRecord};
use crate::testkit::{ALIAS, FakeProcesses, Said, Setup, calm, host_file_of};
use git_ops::lease_slot::host_runs_dir;
use host_link::proto::{Reply, Request};
use pretty_assertions::assert_eq;
use std::net::SocketAddr;

/// The alias of a host whose address no agent listens on
const GONE: &str = "gone";

/// A record of `run` on the host `alias` in `files`, ended or not
fn record(files: &RunFiles, run: &str, alias: &str, ended: bool) {
  files.write(run, &RunRecord::default()).unwrap();
  files.write_host(run, alias).unwrap();
  if ended {
    files.write_end(run, &RunEnd::default()).unwrap();
  }
}

/// The names in the run directory, sorted
fn names(files: &RunFiles) -> Vec<String> {
  let mut names: Vec<String> = std::fs::read_dir(files.dir())
    .unwrap()
    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
    .collect();
  names.sort();
  names
}

/// Only the runs without an end are cancelled, each on the host that its record names, the hosts in parallel: the agent
/// that took the cancel, a run that the agent no longer knows and a run that ended there are cancelled, and their records
/// go, but the record of a run whose end another process holds the lock of. A run that the agent does not know whose
/// record is young stays, as its start can still come. A host that does not answer within `max` and a host that no agent
/// serves keep their runs, with the refusal of each
#[tokio::test(start_paused = true)]
async fn only_the_runs_without_an_end_are_cancelled_and_a_silent_host_keeps_its_runs_past_max() {
  let setup = Setup::new(|request, channel| {
    Box::pin(async move {
      match request {
        Request::Cancel { run } if run == "2-2" || run == "2-3" => {
          let _ = channel
            .send(&Reply::Done {
              message: format!("the supervisor of the run {run} has the cancel request"),
            })
            .await;
        }
        Request::Cancel { run } if run == "3-3" || run == "3-5" => {
          let _ = channel.send(&Reply::refused(Code::UnknownRun, format!("no run {run}"))).await;
        }
        // a host that took the connection and never answers
        Request::Cancel { .. } => tokio::time::sleep(Duration::from_hours(1)).await,
        _ => {}
      }
    })
  });
  let mut gone = host_file_of(GONE, &setup.agent, vec![SocketAddr::from(([127, 0, 0, 1], 9))]);
  gone.port = 9;
  setup.config.write_host(&gone).unwrap();
  let files = RunFiles::new(&host_runs_dir(&setup.repo.path().join(".git"))).unwrap();
  record(&files, "1-1", ALIAS, true);
  record(&files, "2-2", ALIAS, false);
  record(&files, "2-3", ALIAS, false);
  record(&files, "3-3", ALIAS, false);
  record(&files, "3-5", ALIAS, false);
  // the start of 3-3 is long past; 3-5 was written just now
  let old = std::time::SystemTime::now() - START_GRACE - Duration::from_secs(60);
  std::fs::File::options().write(true).open(files.dir().join("3-3.host")).unwrap().set_modified(old).unwrap();
  // another process finishes 2-3
  let held = files.try_lock("2-3", true).unwrap().expect("the lock of 2-3");
  record(&files, "4-4", ALIAS, false);
  record(&files, "5-5", GONE, false);
  record(&files, "6-6", GONE, false);
  // a record of a client before `<run>.host` names no host
  files.write("7-7", &RunRecord::default()).unwrap();

  let (processes, said) = (FakeProcesses::default(), Said::default());
  let say = |line: &str| said.push(line);
  let client = setup.client(&processes, &say, &calm);
  let started = Instant::now();
  let answer = client.cancel_unfinished(setup.repo.path(), Duration::from_secs(5)).await.unwrap();
  assert!(
    started.elapsed() <= Duration::from_secs(6),
    "the silent host holds the call past its max: {:?}",
    started.elapsed()
  );

  let cancels: Vec<String> = setup
    .agent
    .requests()
    .iter()
    .filter_map(|request| match request {
      Request::Cancel { run } => Some(run.clone()),
      _ => None,
    })
    .collect();
  drop(held);
  assert_eq!(
    cancels,
    ["2-2", "2-3", "3-3", "3-5", "4-4"],
    "no cancel of the ended run, the old record or the runs of the gone host"
  );
  assert_eq!(
    answer.cancelled,
    [
      CancelledRun {
        run: "2-2".to_owned(),
        host: ALIAS.to_owned(),
        answer: "the supervisor of the run 2-2 has the cancel request".to_owned(),
      },
      CancelledRun {
        run: "2-3".to_owned(),
        host: ALIAS.to_owned(),
        answer: "the supervisor of the run 2-3 has the cancel request; another process of this checkout finishes the run, so its record stays".to_owned(),
      },
      CancelledRun {
        run: "3-3".to_owned(),
        host: ALIAS.to_owned(),
        answer: "studio: no run 3-3".to_owned(),
      },
    ]
  );
  let refused: Vec<(&str, &str, Code)> = answer.not_cancelled.iter().map(|run| (run.run.as_str(), run.host.as_str(), run.code)).collect();
  assert_eq!(
    refused,
    [
      ("3-5", ALIAS, Code::UnknownRun),
      ("4-4", ALIAS, Code::TimedOut),
      ("5-5", GONE, Code::Unreachable),
      ("6-6", GONE, Code::Unreachable)
    ],
    "{:#?}",
    answer.not_cancelled
  );
  assert!(answer.not_cancelled[0].message.contains("its start can still come"), "{}", answer.not_cancelled[0].message);
  assert!(
    answer.not_cancelled[1].message.contains("did not answer the cancel within 5.0s"),
    "{}",
    answer.not_cancelled[1].message
  );
  assert_eq!(
    names(&files),
    [
      "1-1.end.json",
      "1-1.host",
      "1-1.json",
      "2-3.host",
      "2-3.json",
      "2-3.lock",
      "3-5.host",
      "3-5.json",
      "4-4.host",
      "4-4.json",
      "5-5.host",
      "5-5.json",
      "6-6.host",
      "6-6.json",
      "7-7.json",
    ],
    "the records of the cancelled runs go, the others stay for the next end of a lease, and so does a lock file"
  );

  // the next call cancels only what stayed
  let again = client.cancel_unfinished(setup.repo.path(), Duration::from_secs(1)).await.unwrap();
  let cancelled: Vec<&str> = again.cancelled.iter().map(|run| run.run.as_str()).collect();
  let runs: Vec<&str> = again.not_cancelled.iter().map(|run| run.run.as_str()).collect();
  assert_eq!((cancelled, runs), (vec!["2-3"], vec!["3-5", "4-4", "5-5", "6-6"]));
  assert!(!files.dir().join("2-3.json").exists() && files.dir().join("2-3.lock").exists(), "{:?}", names(&files));
}

/// A checkout without a run directory connects to no host and makes no directory
#[tokio::test(start_paused = true)]
async fn a_checkout_without_run_records_connects_to_no_host() {
  let setup = Setup::new(|_, _| Box::pin(async {}));
  let (processes, said) = (FakeProcesses::default(), Said::default());
  let say = |line: &str| said.push(line);
  let answer = setup.client(&processes, &say, &calm).cancel_unfinished(setup.repo.path(), DEFAULT_MAX).await.unwrap();
  assert_eq!(answer, Unfinished::default());
  assert_eq!(setup.agent.requests(), Vec::new());
  assert!(!host_runs_dir(&setup.repo.path().join(".git")).exists());
}

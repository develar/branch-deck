use super::*;
use lease_pool::host_series::{IdleCause, SCHEMA};
use pretty_assertions::assert_eq;

const DAY: u64 = 24 * 60 * 60 * 1000;
/// 2026-11-25 00:00 UTC
const NOV_25: u64 = 1_795_564_800_000;

fn minute(at_ms: u64) -> TurnMinute {
  TurnMinute {
    schema: SCHEMA,
    at_ms,
    cores: 20,
    queue: 2,
    cause: Some(IdleCause::Drain),
    ..TurnMinute::default()
  }
}

/// A minute goes into the file of its UTC day, a day stops at its most bytes, a read gives the minutes of its window
/// and counts a line that does not parse, and the cleanup removes the days past their age, nothing else
#[test]
fn the_series_keeps_a_file_a_day_bounded_by_its_bytes_and_its_age() {
  let dir = tempfile::tempdir().unwrap();
  let history = dir.path().join("history");
  assert!(append(&history, &minute(NOV_25 + 60_000)).unwrap());
  assert!(append(&history, &minute(NOV_25 + DAY - 1)).unwrap());
  assert!(append(&history, &minute(NOV_25 + DAY)).unwrap());
  let day = history.join("turn-2026-11-25.jsonl");
  assert_eq!(fs::read_to_string(&day).unwrap().lines().count(), 2);
  assert!(history.join("turn-2026-11-26.jsonl").is_file());

  let mut file = fs::OpenOptions::new().append(true).open(&day).unwrap();
  file.write_all(b"not json\n").unwrap();
  let (minutes, unread) = read(&history, NOV_25, NOV_25 + DAY + 1);
  assert_eq!((minutes.len(), unread), (3, 1));
  assert_eq!(read(&history, NOV_25 + DAY, NOV_25 + 2 * DAY).0, vec![minute(NOV_25 + DAY)], "the window of a read");

  file.set_len(DAY_BYTES).unwrap();
  assert!(!append(&history, &minute(NOV_25 + 120_000)).unwrap(), "a full day takes no more");
  assert_eq!(fs::metadata(&day).unwrap().len(), DAY_BYTES);

  fs::write(history.join("2026-11.jsonl"), "").unwrap();
  fs::write(history.join("turn-notes.jsonl"), "").unwrap();
  let said = expire(&history, NOV_25 + u64::from(KEEP_DAYS + 1) * DAY + DAY / 2);
  assert_eq!(said, ["removed the series of the heavy turn turn-2026-11-25.jsonl, past 30 days"]);
  assert!(!day.exists());
  for kept in ["turn-2026-11-26.jsonl", "2026-11.jsonl", "turn-notes.jsonl"] {
    assert!(history.join(kept).exists(), "{kept}");
  }
}

/// The samples of a minute give their mean and their most, and a new minute begins with none
#[test]
fn the_samples_of_a_minute_give_their_mean_and_their_most() {
  let mut sampled = Sampled::new();
  for busy in [40, 100, 75] {
    sampled.push(Cores(busy));
  }
  assert_eq!(
    sampled.take(),
    Some(MinuteBusy {
      mean: Cores(72),
      most: Cores(100)
    })
  );
  assert_eq!(sampled.take(), None);
}

/// The sampler hands each sample to the series without a wait: a writer that takes no sample (a slow volume) or one that
/// is gone (a panic, an error) leaves each sample of `host.json` fresh, and each note at once
#[test]
fn a_slow_or_failing_series_leaves_the_samples_of_the_host_fresh() {
  let dir = tempfile::tempdir().unwrap();
  let mut window = lease_pool::host_turn::HostWindow::default();
  let (feed, samples) = channel(4);
  let started = Instant::now();
  for at in 0..100u64 {
    crate::turn::note_sample(dir.path(), &mut window, NOV_25 + at * 2_000, Cores(50), &feed).unwrap();
    let sample: serde_json::Value = serde_json::from_slice(&fs::read(dir.path().join("host.json")).unwrap()).unwrap();
    assert_eq!(sample["at_ms"], NOV_25 + at * 2_000, "the sample of this note");
  }
  assert!(started.elapsed() < Duration::from_secs(5), "a full queue holds no note back: {:?}", started.elapsed());
  assert_eq!(samples.try_iter().count(), 4, "the queue keeps its 4 samples, and the rest go");
  drop(samples);
  crate::turn::note_sample(dir.path(), &mut window, NOV_25 + 300_000, Cores(60), &feed).unwrap();
  let sample: serde_json::Value = serde_json::from_slice(&fs::read(dir.path().join("host.json")).unwrap()).unwrap();
  assert_eq!(sample["at_ms"], NOV_25 + 300_000, "a writer that is gone holds no note back");
}

/// The writer hands the samples of each minute to the minute, outlives a panic of it, and says a failure once per state:
/// again after a minute was written, or for another state
#[test]
fn the_writer_of_the_series_says_a_failure_once_per_state_and_outlives_a_panic() {
  let (feed, samples) = channel(16);
  let outcomes = [
    None,
    Some(Failure::Read("locked".to_owned())),
    Some(Failure::Read("locked".to_owned())),
    Some(Failure::Write("disk full".to_owned())),
    None,
    Some(Failure::Write("disk full".to_owned())),
    Some(Failure::Full("2026-11-25".to_owned())),
    Some(Failure::Full("2026-11-25".to_owned())),
  ];
  for busy in 0..=u32::try_from(outcomes.len()).unwrap() {
    feed.push(Cores(busy * 10));
  }
  drop(feed);
  let mut minutes = Vec::new();
  let mut said = Vec::new();
  write_minutes(
    &samples,
    Duration::ZERO,
    &mut |busy| {
      minutes.push(busy.map(|busy| busy.mean));
      match minutes.len() {
        1 => panic!("a fault of the minute"),
        n => outcomes[n - 2].clone().map_or(Ok(()), Err),
      }
    },
    &mut |line| said.push(line.to_owned()),
  );
  assert_eq!(minutes.len(), 9, "every minute after the panic too");
  assert_eq!(minutes[1], Some(Cores(10)));
  assert_eq!(
    said,
    [
      "the writer of the series of the heavy turn panicked in a minute; it goes on with the next",
      "cannot read the heavy turn for its series: locked; said once until a minute is written",
      "cannot keep a minute of the heavy turn in its series: disk full; said once until a minute is written",
      "cannot keep a minute of the heavy turn in its series: disk full; said once until a minute is written",
      "the series of the heavy turn of 2026-11-25 holds 4194304 bytes: no more minutes of the day",
    ]
  );
}

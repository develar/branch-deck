use crate::transfer_progress::{TransferProgress, TransferProgressAnalyzer, TransferProgressReporter};
use pretty_assertions::assert_eq;

fn percents(lines: &[&str]) -> Vec<Option<u8>> {
  let mut analyzer = TransferProgressAnalyzer::new();
  lines.iter().map(|line| analyzer.analyze(line).map(|it| it.percent)).collect()
}

#[test]
fn stages_have_the_shares_of_idea() {
  // as `read_progress_lines` passes them from git 2.56: trimmed, `remote:` lines from the server
  assert_eq!(
    percents(&[
      "remote: Enumerating objects: 302, done.",
      "remote: Counting objects:  50% (151/302)",
      "remote: Counting objects: 100% (302/302), done.",
      "remote: Compressing objects:  50% (151/302)",
      "remote: Compressing objects: 100% (302/302), done.",
      "Receiving objects:  45% (136/302)",
      "remote: Total 302 (delta 0), reused 0 (delta 0), pack-reused 0 (from 0)",
      "Receiving objects: 100% (302/302), 5.79 MiB | 32.57 MiB/s, done.",
      "Resolving deltas:  50% (1/2)",
      "Resolving deltas: 100% (2/2), done.",
    ]),
    [None, Some(3), Some(5), Some(10), Some(15), Some(51), None, Some(95), Some(98), Some(100)]
  );
}

#[test]
fn receiving_reports_the_amount_and_the_rate() {
  let mut analyzer = TransferProgressAnalyzer::new();
  assert_eq!(analyzer.analyze("Receiving objects:  20% (61/302)").unwrap().transferred, None);
  assert_eq!(
    analyzer.analyze("Receiving objects:  45% (136/302), 2.61 MiB | 5.20 MiB/s").unwrap(),
    TransferProgress {
      percent: 51,
      transferred: Some("2.61 MiB".to_string()),
      speed: Some("5.20 MiB/s".to_string()),
      line: "Receiving objects:  45% (136/302), 2.61 MiB | 5.20 MiB/s".to_string(),
      waiting: false,
    }
  );
  // resolving deltas keeps the last amount
  let resolving = analyzer.analyze("Resolving deltas:  10% (1/10)").unwrap();
  assert_eq!(resolving.transferred.as_deref(), Some("2.61 MiB"));
  assert_eq!(resolving.line, "Resolving deltas:  10% (1/10)");
}

#[test]
fn small_fetch_unpacks_instead_of_receiving() {
  assert_eq!(
    percents(&["Unpacking objects:  50% (2/4)", "Unpacking objects: 100% (4/4), 1.02 KiB | 348.00 KiB/s, done."]),
    [Some(55), Some(95)]
  );
}

#[test]
fn counting_without_a_total_counts_5000_objects_as_done() {
  assert_eq!(percents(&["remote: Counting objects: 2500", "remote: Counting objects: 9000"]), [Some(3), Some(5)]);
}

#[test]
fn percent_never_decreases() {
  // a stage that starts again (another pack) must not move the button back
  assert_eq!(percents(&["Receiving objects: 100% (3/3), done.", "Receiving objects:  10% (1/10)"]), [Some(95), Some(95)]);
}

#[test]
fn other_lines_change_nothing() {
  assert_eq!(
    percents(&["From ../upstream", "fatal: early EOF", "Receiving objects: lots", "warning: x: 5%"]),
    [None, None, None, None]
  );
}

#[test]
fn push_stages_have_the_same_shares() {
  // a push counts, compresses and writes locally; the server resolves the deltas and ends without `, done.`
  assert_eq!(
    percents(&[
      "Enumerating objects: 7, done.",
      "Counting objects: 100% (7/7), done.",
      "Delta compression using up to 10 threads",
      "Compressing objects: 100% (3/3), done.",
      "Writing objects:  50% (2/4)",
      "Writing objects: 100% (4/4), 1.23 KiB | 1.23 MiB/s, done.",
      "Total 4 (delta 2), reused 0 (delta 0), pack-reused 0 (from 0)",
      "remote: Resolving deltas:  50% (1/2)",
      "remote: Resolving deltas: 100% (2/2), completed with 2 local objects.",
    ]),
    [None, Some(5), None, Some(15), Some(55), Some(95), None, Some(98), Some(100)]
  );
}

#[test]
fn writing_reports_the_amount_and_the_rate() {
  let mut analyzer = TransferProgressAnalyzer::new();
  let writing = analyzer.analyze("Writing objects:  45% (136/302), 2.61 MiB | 5.20 MiB/s").unwrap();
  assert_eq!(writing.transferred.as_deref(), Some("2.61 MiB"));
  assert_eq!(writing.speed.as_deref(), Some("5.20 MiB/s"));
}

fn reported(lines: &[&str]) -> Vec<String> {
  let mut sent = Vec::new();
  let mut on_progress = |progress: TransferProgress| sent.push(progress.line);
  let mut reporter = TransferProgressReporter::new(&mut on_progress);
  for line in lines {
    reporter.on_line(line);
  }
  sent
}

#[test]
fn reporter_sends_the_first_progress_and_every_stage_end() {
  // all lines arrive within the interval: only the first progress and the stage ends go out
  assert_eq!(
    reported(&[
      "Enumerating objects: 7, done.",
      "Writing objects:  10% (1/10)",
      "Writing objects:  50% (5/10)",
      "Writing objects: 100% (10/10), 1.23 KiB | 1.23 MiB/s, done.",
      "remote: Resolving deltas:  50% (1/2)",
      "remote: Resolving deltas: 100% (2/2), completed with 2 local objects.",
    ]),
    [
      "Writing objects:  10% (1/10)",
      "Writing objects: 100% (10/10), 1.23 KiB | 1.23 MiB/s, done.",
      "Resolving deltas: 100% (2/2), completed with 2 local objects.",
    ]
  );
}

#[test]
fn reporter_skips_a_stage_end_that_changes_nothing() {
  // git repeats the last update with `, done.`: the same percent and amount are not sent again
  assert_eq!(
    reported(&[
      "Writing objects: 100% (10/10), 1.23 KiB | 1.23 MiB/s",
      "Writing objects: 100% (10/10), 1.23 KiB | 1.23 MiB/s, done."
    ]),
    ["Writing objects: 100% (10/10), 1.23 KiB | 1.23 MiB/s"]
  );
}

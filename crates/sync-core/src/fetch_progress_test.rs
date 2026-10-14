use crate::fetch_progress::{FetchProgress, FetchProgressAnalyzer};
use pretty_assertions::assert_eq;

fn percents(lines: &[&str]) -> Vec<Option<u8>> {
  let mut analyzer = FetchProgressAnalyzer::new();
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
  let mut analyzer = FetchProgressAnalyzer::new();
  assert_eq!(analyzer.analyze("Receiving objects:  20% (61/302)").unwrap().received, None);
  assert_eq!(
    analyzer.analyze("Receiving objects:  45% (136/302), 2.61 MiB | 5.20 MiB/s").unwrap(),
    FetchProgress {
      percent: 51,
      received: Some("2.61 MiB".to_string()),
      speed: Some("5.20 MiB/s".to_string()),
      line: "Receiving objects:  45% (136/302), 2.61 MiB | 5.20 MiB/s".to_string(),
    }
  );
  // resolving deltas keeps the last amount
  let resolving = analyzer.analyze("Resolving deltas:  10% (1/10)").unwrap();
  assert_eq!(resolving.received.as_deref(), Some("2.61 MiB"));
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

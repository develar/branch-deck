use super::TrimReport;
use lease_pool::lease::{SkipReason, SkippedSlot};
use lease_pool::trim::{RemovedBase, RemovedSlot, Trimmed, TrimmedPool};
use pretty_assertions::assert_eq;

const GIB: u64 = 1 << 30;

fn trimmed() -> Trimmed {
  Trimmed {
    pools: vec![
      TrimmedPool {
        git_common_dir: "/h/idea-1/.git".to_owned(),
        max_slots: 2,
        slots: 4,
        removed: vec![RemovedSlot {
          name: "idea-13".to_owned(),
          path: "/h/pool/3/idea-1".to_owned(),
          returned: Some(3),
          dir: "/h/pool/3".to_owned(),
          output_bases: vec![RemovedBase {
            path: "/h/bazel/aa".to_owned(),
            workspace: "/h/pool/3/idea-1".to_owned(),
            freed_bytes: GIB,
            error: None,
          }],
          dir_freed_bytes: 2 * GIB,
          errors: Vec::new(),
        }],
        skipped: vec![SkippedSlot {
          name: "idea-14".to_owned(),
          path: "/h/pool/4/idea-1".to_owned(),
          reason: SkipReason::Busy { processes: Vec::new() },
        }],
        config_error: None,
      },
      TrimmedPool {
        git_common_dir: "/h/accio/.git".to_owned(),
        max_slots: 10,
        slots: 2,
        removed: Vec::new(),
        skipped: Vec::new(),
        config_error: None,
      },
    ],
    orphans: vec![RemovedBase {
      path: "/h/bazel/bb".to_owned(),
      workspace: "/h/pool/9/idea-1".to_owned(),
      freed_bytes: GIB,
      error: None,
    }],
    process_scan: true,
  }
}

/// The report counts the bytes of a slot with its output bases, keeps the busy slots by path, and its lines name each
/// pool past its limit only; a plan says what would go, without sizes
#[test]
fn a_trim_report_counts_each_slot_with_its_output_bases_and_names_only_the_pools_it_touched() {
  let report = TrimReport::of(&trimmed(), false, Some(156 * GIB));
  assert_eq!(report.pools[0].removed[0].freed_bytes, 3 * GIB);
  assert_eq!(report.freed_bytes, 4 * GIB);
  assert_eq!(report.pools[0].busy, ["/h/pool/4/idea-1"]);
  assert_eq!(
    report.lines("studio"),
    [
      "studio: deleted 1 free slot(s) past the limit and 1 output base(s) of gone slots, 4.0 GiB; 156.0 GiB free",
      "  /h/idea-1/.git: 4 slot(s), limit 2",
      "    deleted /h/pool/3/idea-1, 3.0 GiB",
      "    kept /h/pool/4/idea-1: a process works in it",
    ]
  );
  let parsed: TrimReport = serde_json::from_value(serde_json::to_value(&report).unwrap()).unwrap();
  assert_eq!(parsed, report, "the client reads the JSON of the agent");

  let plan = TrimReport::of(&trimmed(), true, None);
  assert_eq!(
    plan.lines("studio")[0],
    "studio: a trim would delete 1 free slot(s) past the limit and 1 output base(s) of gone slots"
  );
  assert_eq!(plan.lines("studio")[2], "    would delete /h/pool/3/idea-1");
}

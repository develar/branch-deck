//! The steps of a disk report as a reader tells them (`brd lease disk` on stderr; openspec/specs/cli/lease/spec.md,
//! "Disk"): the saved report, the check of the history, each slot whose walk ended with where its bytes come from, and
//! the end with what the budget or another report left saved. The core owns the steps and their words, as
//! `lease_pool::lease::GetStep` of a get; a reader only adds its prefix

use crate::{DiskUsage, DiskUsageProgress, SlotWalk};
use lease_pool::lease_clean::describe_size;
use std::fmt;

/// A step of `disk_usage_core_with_progress`, told as it ends
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiskStep {
  /// The report from `disk/` is ready (`DiskUsageProgress::Cached`): `measuring` of the `slots` of the `pools` remain to
  /// check or to walk
  Start { pools: usize, slots: usize, measuring: u32 },
  /// The history was read in `history_ms` (`DiskUsageProgress::Planning`): `measuring` slots remain to walk, `late` keep
  /// their saved bytes because it came late
  Checked { measuring: u32, late: u32, history_ms: Option<u64> },
  /// The last walk of the slot at `path` ended (`DiskUsageProgress::Slot`), with its `bytes` and why it was walked:
  /// `measured` of `measuring`
  Walked {
    path: String,
    bytes: u64,
    walk: SlotWalk,
    parts: u32,
    measured: u32,
    measuring: u32,
  },
  /// The report is ready, after the walks of the output bases of the main worktrees and of the orphans, which tell no
  /// step of their own; `budget` slots keep their saved bytes as the walk budget ended, `busy` as another report walks
  Done {
    checkout_bases: usize,
    orphans: usize,
    budget: usize,
    busy: usize,
  },
}

impl DiskStep {
  pub fn of(progress: &DiskUsageProgress) -> Self {
    match progress {
      DiskUsageProgress::Cached { usage, measuring } => Self::Start {
        pools: usage.pools.len(),
        slots: usage.pools.iter().map(|pool| pool.slots.len()).sum(),
        measuring: *measuring,
      },
      DiskUsageProgress::Planning { measuring, late, history } => Self::Checked {
        measuring: *measuring,
        late: *late,
        history_ms: history.map(|history| u64::try_from(history.as_millis()).unwrap_or(u64::MAX)),
      },
      DiskUsageProgress::Slot {
        usage,
        pool_index,
        slot_index,
        measured,
        measuring,
      } => {
        let slot = &usage.pools[*pool_index].slots[*slot_index];
        Self::Walked {
          path: slot.path.clone(),
          bytes: slot.bytes(),
          walk: slot.walk,
          parts: slot.parts,
          measured: *measured,
          measuring: *measuring,
        }
      }
    }
  }

  pub fn done(usage: &DiskUsage) -> Self {
    let slots = || usage.pools.iter().flat_map(|pool| &pool.slots);
    Self::Done {
      checkout_bases: usage.pools.iter().map(|pool| pool.checkout_bases.len()).sum(),
      orphans: usage.orphans.len(),
      budget: slots().filter(|slot| slot.walk == SlotWalk::Budget).count(),
      busy: slots().filter(|slot| slot.walk == SlotWalk::Busy).count(),
    }
  }
}

/// Where the bytes of a walked slot come from, in words
pub fn walk_words(walk: SlotWalk, parts: u32) -> String {
  match walk {
    SlotWalk::Parts => format!("{parts} changed part(s) again"),
    SlotWalk::Saved => "the saved bytes".to_owned(),
    SlotWalk::Unchanged => "unchanged since its measure".to_owned(),
    SlotWalk::Busy => "the saved bytes: another report walks the pool".to_owned(),
    SlotWalk::HistoryLate => "the saved bytes: the history of the file system came late".to_owned(),
    SlotWalk::Budget => "the saved bytes: the walk budget ended".to_owned(),
    SlotWalk::NoSaved => "in full: nothing saved".to_owned(),
    SlotWalk::Fresh => "in full: --fresh".to_owned(),
    SlotWalk::Stale => "in full: the saved bytes are of another lease or too old".to_owned(),
    SlotWalk::HistoryLost => "in full: the history lost events under it".to_owned(),
    SlotWalk::NoMark => "in full: no mark of the history of its volume".to_owned(),
    SlotWalk::HistoryLateLong => "in full: the history came late for a day".to_owned(),
    SlotWalk::HistoryLateWalked => "in full: the history of the file system came late".to_owned(),
    SlotWalk::NoHistory => "in full: no history of the file system".to_owned(),
  }
}

impl fmt::Display for DiskStep {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Start { pools, slots, measuring } => write!(f, "{measuring} of {slots} slot(s) in {pools} pool(s) to check or measure"),
      Self::Checked { measuring, late, history_ms } => {
        write!(f, "{measuring} slot(s) to walk after the FSEvents check")?;
        if let Some(ms) = history_ms {
          write!(f, " of {ms} ms")?;
        }
        if *late > 0 {
          write!(f, "; {late} keep their saved bytes: the history came late")?;
        }
        Ok(())
      }
      Self::Walked {
        path,
        bytes,
        walk,
        parts,
        measured,
        measuring,
      } => write!(f, "walked {path}: {} ({measured} of {measuring}), {}", describe_size(*bytes), walk_words(*walk, *parts)),
      Self::Done {
        checkout_bases,
        orphans,
        budget,
        busy,
      } => {
        write!(f, "the report is ready, with {checkout_bases} output base(s) of the main worktrees and {orphans} orphan(s)")?;
        if *budget > 0 {
          write!(f, "; {budget} slot(s) keep their saved bytes: the walk budget ended, the next report walks them")?;
        }
        if *busy > 0 {
          write!(f, "; {busy} slot(s) show their saved bytes: another report walks the pool")?;
        }
        Ok(())
      }
    }
  }
}

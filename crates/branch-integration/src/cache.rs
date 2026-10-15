use anyhow::{Context, Result};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_writer::write_notes;
use serde_json::{Value, json};
use sync_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus, IntegrationConfidence};
use sync_types::upstream_commit::UpstreamCommit;
use tracing::{debug, instrument};

// Git notes ref for detection cache - this is the namespace where notes are stored
pub const NOTES_REF: &str = "refs/notes/branch-deck/detection";

/// Notes on the tips of stacked virtual branches: the commit the branch was built on (its parent's head), so detection
/// of the archived branch looks only at its own commits
pub const STACK_NOTES_REF: &str = "refs/notes/branch-deck/stack";

/// Current detection cache version: 2 records the baseline a `NotIntegrated` result was checked against (`b`) and the
/// commits without an equivalent (`u`); a note without `b` is detected again in full, within the retention period
pub const DETECTION_CACHE_VERSION: u8 = 2;

/// A note on a branch tip: the detection result and, for a `NotIntegrated` one, what a recheck needs
#[derive(Debug, Clone)]
pub struct CachedDetection {
  pub info: BranchIntegrationInfo,
  /// `None` for a final result (`Integrated`, `Partial`) and for a `NotIntegrated` note without a baseline (written by
  /// version 1)
  pub check: Option<Recheck>,
}

/// What a `NotIntegrated` result was checked against: when the baseline moves on from it, only the baseline commits
/// since then are compared (`^<baseline>` in the cherry-mark scan)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recheck {
  /// The baseline commit the result was checked against
  pub baseline: String,
  /// The commits of the branch without an equivalent in that baseline; `None` when it is all of them
  pub orphaned: Option<Vec<String>>,
}

/// Serialize a detection result to compact JSON for git notes storage; `check` is written only with a `NotIntegrated`
/// result
pub fn serialize_for_cache(info: &BranchIntegrationInfo, check: Option<&Recheck>) -> Result<String> {
  let status_json = match &info.status {
    BranchIntegrationStatus::Integrated {
      integrated_at,
      confidence,
      commit_count,
      commit,
    } => {
      let mut status = json!({
        "k": "i",
        "c": match confidence {
          IntegrationConfidence::Exact => "e",
          IntegrationConfidence::High => "h"
        }
      });
      if *commit_count != 0 {
        status.as_object_mut().unwrap().insert("cc".to_string(), json!(commit_count));
      }
      if let Some(ia) = integrated_at {
        status.as_object_mut().unwrap().insert("ia".to_string(), json!(ia));
      }
      // `[id, author, committer]`; the time is `ia`
      if let Some(commit) = commit {
        status.as_object_mut().unwrap().insert("x".to_string(), json!([commit.id, commit.author, commit.committer]));
      }
      status
    }
    BranchIntegrationStatus::NotIntegrated {
      total_commit_count,
      integrated_count,
      orphaned_count,
      integrated_at,
    } => {
      let mut status = json!({"k": "n"});
      if *total_commit_count != 0 {
        status.as_object_mut().unwrap().insert("tc".to_string(), json!(total_commit_count));
      }
      if *integrated_count != 0 {
        status.as_object_mut().unwrap().insert("ic".to_string(), json!(integrated_count));
      }
      if *orphaned_count != 0 {
        status.as_object_mut().unwrap().insert("oc".to_string(), json!(orphaned_count));
      }
      if let Some(ia) = integrated_at {
        status.as_object_mut().unwrap().insert("ia".to_string(), json!(ia));
      }
      if let Some(check) = check {
        status.as_object_mut().unwrap().insert("b".to_string(), json!(check.baseline));
        if let Some(orphaned) = &check.orphaned {
          status.as_object_mut().unwrap().insert("u".to_string(), json!(orphaned));
        }
      }
      status
    }
    BranchIntegrationStatus::Partial { missing } => {
      let mut status = json!({"k": "p"});
      if *missing != 0 {
        status.as_object_mut().unwrap().insert("m".to_string(), json!(missing));
      }
      status
    }
  };

  let cache_entry = if info.summary.is_empty() {
    json!({
      "v": DETECTION_CACHE_VERSION,
      "s": status_json
    })
  } else {
    json!({
      "v": DETECTION_CACHE_VERSION,
      "s": status_json,
      "sum": info.summary
    })
  };

  Ok(serde_json::to_string(&cache_entry)?)
}

/// Reads a numeric field as `u32`, saturating values that do not fit.
fn read_u32(value: &serde_json::Value, key: &str) -> Option<u32> {
  value.get(key).and_then(serde_json::Value::as_u64).map(|v| u32::try_from(v).unwrap_or(u32::MAX))
}

/// Reads the integration commit `x: [id, author, committer]`, committed at `time`
fn read_upstream_commit(value: &serde_json::Value, time: u32) -> Option<UpstreamCommit> {
  let fields = value.get("x")?.as_array()?;
  let field = |i: usize| fields.get(i).and_then(Value::as_str).map(str::to_string);
  Some(UpstreamCommit {
    id: field(0).filter(|id| !id.is_empty())?,
    author: field(1)?,
    committer: field(2)?,
    time,
  })
}

/// The recheck data of a `NotIntegrated` status: `None` without a baseline or with an unreadable `u`
fn read_recheck(value: &serde_json::Value) -> Option<Recheck> {
  let baseline = value.get("b")?.as_str().filter(|baseline| !baseline.is_empty())?.to_string();
  let orphaned = match value.get("u") {
    None => None,
    Some(ids) => Some(ids.as_array()?.iter().map(|id| id.as_str().map(str::to_string)).collect::<Option<Vec<String>>>()?),
  };
  Some(Recheck { baseline, orphaned })
}

/// Deserialize compact JSON from git notes (the info with an empty name)
pub fn deserialize_from_cache(json: &str) -> Result<CachedDetection> {
  let value: Value = serde_json::from_str(json)?;

  let summary = value.get("sum").and_then(|v| v.as_str()).unwrap_or("").to_string();

  let status_value = value.get("s").ok_or_else(|| anyhow::anyhow!("Missing status field"))?;

  let mut check = None;
  let status = match status_value.get("k").and_then(|v| v.as_str()) {
    Some("i") => {
      let commit_count = read_u32(status_value, "cc").unwrap_or(0);

      let integrated_at = read_u32(status_value, "ia");

      let confidence = match status_value.get("c").and_then(|v| v.as_str()) {
        Some("e") => IntegrationConfidence::Exact,
        // "h", missing, or unknown: default to High for backward compatibility
        _ => IntegrationConfidence::High,
      };

      // absent in notes written before the commit was recorded
      let commit = integrated_at.and_then(|time| read_upstream_commit(status_value, time));

      BranchIntegrationStatus::Integrated {
        integrated_at,
        confidence,
        commit_count,
        commit,
      }
    }
    Some("n") => {
      let total_commit_count = read_u32(status_value, "tc").unwrap_or(0);

      let integrated_count = read_u32(status_value, "ic").unwrap_or(0);

      let orphaned_count = read_u32(status_value, "oc").unwrap_or(0);

      let integrated_at = read_u32(status_value, "ia");

      check = read_recheck(status_value);

      BranchIntegrationStatus::NotIntegrated {
        total_commit_count,
        integrated_count,
        orphaned_count,
        integrated_at,
      }
    }
    Some("p") => {
      let missing = read_u32(status_value, "m").unwrap_or(0);

      BranchIntegrationStatus::Partial { missing }
    }
    _ => return Err(anyhow::anyhow!("Unknown status kind")),
  };

  Ok(CachedDetection {
    info: BranchIntegrationInfo {
      name: String::new(), // Empty name - to be filled by caller
      summary,
      status,
    },
    check,
  })
}

/// Cache operations wrapper
pub struct CacheOps<'a> {
  git: &'a GitCommandExecutor,
  repo: &'a str,
}

impl<'a> CacheOps<'a> {
  pub fn new(git: &'a GitCommandExecutor, repo: &'a str) -> Self {
    Self { git, repo }
  }

  /// Write one cached detection result as a note
  pub fn write(&self, commit: &str, detection: &CachedDetection) -> Result<()> {
    self.write_all(&[(commit.to_string(), detection.clone())])
  }

  /// Write the cached detection results as notes, all in one notes commit
  #[instrument(skip(self, entries), fields(count = entries.len()))]
  pub fn write_all(&self, entries: &[(String, CachedDetection)]) -> Result<()> {
    let notes = entries
      .iter()
      .map(|(commit, detection)| Ok((commit.clone(), serialize_for_cache(&detection.info, detection.check.as_ref())?)))
      .collect::<Result<Vec<_>>>()?;
    write_notes(self.git, self.repo, NOTES_REF, &notes).context("Failed to write notes")?;
    debug!("Cached detection for {} branches", notes.len());
    Ok(())
  }
}

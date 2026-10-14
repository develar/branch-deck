use anyhow::{Context, Result};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_writer::write_notes;
use serde_json::{Value, json};
use sync_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus, IntegrationConfidence};
use tracing::{debug, instrument};

// Git notes ref for detection cache - this is the namespace where notes are stored
pub const NOTES_REF: &str = "refs/notes/branch-deck/detection";

/// Notes on the tips of stacked virtual branches: the commit the branch was built on (its parent's head), so detection
/// of the archived branch looks only at its own commits
pub const STACK_NOTES_REF: &str = "refs/notes/branch-deck/stack";

// Current detection cache version
pub const DETECTION_CACHE_VERSION: u8 = 1;

/// Serialize `BranchIntegrationInfo` to compact JSON for git notes storage
pub fn serialize_for_cache(info: &BranchIntegrationInfo) -> Result<String> {
  let status_json = match &info.status {
    BranchIntegrationStatus::Integrated {
      integrated_at,
      confidence,
      commit_count,
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

/// Deserialize compact JSON from git notes to `BranchIntegrationInfo` (with empty name)
pub fn deserialize_from_cache(json: &str) -> Result<BranchIntegrationInfo> {
  let value: Value = serde_json::from_str(json)?;

  let summary = value.get("sum").and_then(|v| v.as_str()).unwrap_or("").to_string();

  let status_value = value.get("s").ok_or_else(|| anyhow::anyhow!("Missing status field"))?;

  let status = match status_value.get("k").and_then(|v| v.as_str()) {
    Some("i") => {
      let commit_count = read_u32(status_value, "cc").unwrap_or(0);

      let integrated_at = read_u32(status_value, "ia");

      let confidence = match status_value.get("c").and_then(|v| v.as_str()) {
        Some("e") => IntegrationConfidence::Exact,
        // "h", missing, or unknown: default to High for backward compatibility
        _ => IntegrationConfidence::High,
      };

      BranchIntegrationStatus::Integrated {
        integrated_at,
        confidence,
        commit_count,
      }
    }
    Some("n") => {
      let total_commit_count = read_u32(status_value, "tc").unwrap_or(0);

      let integrated_count = read_u32(status_value, "ic").unwrap_or(0);

      let orphaned_count = read_u32(status_value, "oc").unwrap_or(0);

      let integrated_at = read_u32(status_value, "ia");

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

  Ok(BranchIntegrationInfo {
    name: String::new(), // Empty name - to be filled by caller
    summary,
    status,
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
  pub fn write(&self, commit: &str, info: &BranchIntegrationInfo) -> Result<()> {
    self.write_all(&[(commit.to_string(), info.clone())])
  }

  /// Write the cached detection results as notes, all in one notes commit
  #[instrument(skip(self, entries), fields(count = entries.len()))]
  pub fn write_all(&self, entries: &[(String, BranchIntegrationInfo)]) -> Result<()> {
    let notes = entries
      .iter()
      .map(|(commit, info)| Ok((commit.clone(), serialize_for_cache(info)?)))
      .collect::<Result<Vec<_>>>()?;
    write_notes(self.git, self.repo, NOTES_REF, &notes).context("Failed to write notes")?;
    debug!("Cached detection for {} branches", notes.len());
    Ok(())
  }
}

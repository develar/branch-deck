use anyhow::{Context, Result};
use git_executor::git_command_executor::GitCommandExecutor;
use serde_json::{Value, json};
use sync_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus, IntegrationConfidence};
use tracing::{debug, instrument, trace};

// Git notes ref for detection cache - this is the namespace where notes are stored
pub const NOTES_REF: &str = "refs/notes/branch-deck/detection";

// Current detection cache version
pub const DETECTION_CACHE_VERSION: u8 = 1;

/// Serialize BranchIntegrationInfo to compact JSON for git notes storage
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

/// Deserialize compact JSON from git notes to BranchIntegrationInfo (with empty name)
pub fn deserialize_from_cache(json: &str) -> Result<BranchIntegrationInfo> {
  let value: Value = serde_json::from_str(json)?;

  let summary = value.get("sum").and_then(|v| v.as_str()).unwrap_or("").to_string();

  let status_value = value.get("s").ok_or_else(|| anyhow::anyhow!("Missing status field"))?;

  let status = match status_value.get("k").and_then(|v| v.as_str()) {
    Some("i") => {
      let commit_count = status_value.get("cc").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

      let integrated_at = status_value.get("ia").and_then(|v| v.as_u64()).map(|v| v as u32);

      let confidence = match status_value.get("c").and_then(|v| v.as_str()) {
        Some("e") => IntegrationConfidence::Exact,
        Some("h") | None => IntegrationConfidence::High, // Default to High for backward compatibility
        _ => IntegrationConfidence::High,
      };

      BranchIntegrationStatus::Integrated {
        integrated_at,
        confidence,
        commit_count,
      }
    }
    Some("n") => {
      let total_commit_count = status_value.get("tc").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

      let integrated_count = status_value.get("ic").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

      let orphaned_count = status_value.get("oc").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

      let integrated_at = status_value.get("ia").and_then(|v| v.as_u64()).map(|v| v as u32);

      BranchIntegrationStatus::NotIntegrated {
        total_commit_count,
        integrated_count,
        orphaned_count,
        integrated_at,
      }
    }
    Some("p") => {
      let missing = status_value.get("m").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

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

  /// Read cache from git note (returns info with empty name - caller must set it)
  #[instrument(skip(self), fields(commit = %commit))]
  pub fn read(&self, commit: &str) -> Option<BranchIntegrationInfo> {
    match self.git.execute_command(&["notes", "--ref", NOTES_REF, "show", commit], self.repo) {
      Ok(json) => {
        trace!("Found cache for {}", commit);
        deserialize_from_cache(&json).ok()
      }
      Err(_) => {
        trace!("No cache for {}", commit);
        None
      }
    }
  }

  /// Write cache to git note
  #[instrument(skip(self, info), fields(commit = %commit))]
  pub fn write(&self, commit: &str, info: &BranchIntegrationInfo) -> Result<()> {
    let json = serialize_for_cache(info)?;
    debug!(json = %json, "Writing cache note");
    self
      .git
      .execute_command(&["notes", "--ref", NOTES_REF, "add", "-f", "-m", &json, commit], self.repo)
      .context("Failed to write note")?;
    debug!("Successfully cached detection for {}", commit);
    Ok(())
  }
}

/// Parse cached note from JSON string (returns info with empty name - caller must set it)
pub fn parse_cached_note(json: &str) -> Option<BranchIntegrationInfo> {
  deserialize_from_cache(json).ok()
}

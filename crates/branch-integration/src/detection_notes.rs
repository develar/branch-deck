//! The result of an integration detection as a note on the archived branch's tip (`NOTES_REF`, `CacheOps`), so an
//! unchanged tip is not detected again, and the stack base notes on the tips of stacked branches (`STACK_NOTES_REF`).

use anyhow::{Context, Result};
use domain_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus, IntegrationConfidence};
use domain_types::upstream_commit::UpstreamCommit;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_writer::{write_cache_notes, write_notes};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
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
  /// `None` for a final result (`Integrated`, `Partial`) and for a `NotIntegrated` note without a baseline or without its
  /// orphaned commits (written by version 1)
  pub check: Option<Recheck>,
}

/// What a `NotIntegrated` result was checked against: when the baseline moves on from it, only the baseline commits
/// since then are compared (`^<baseline>` in the cherry-mark scan)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recheck {
  /// The baseline commit the result was checked against
  pub baseline: String,
  /// The commits of the branch without an equivalent in that baseline (full ids). Always listed, so a recheck compares
  /// exactly these: the branch side of a stacked archive grows by its parent's commits once the stack base is gone
  pub orphaned: Vec<String>,
}

/// A note as git stores it: short keys, zero counts, a missing time and an empty summary left out
#[derive(Serialize, Deserialize)]
struct NoteDto {
  #[serde(default)]
  v: u8,
  s: StatusDto,
  #[serde(default, skip_serializing_if = "String::is_empty", deserialize_with = "lenient")]
  sum: String,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "k")]
enum StatusDto {
  #[serde(rename = "i")]
  Integrated {
    #[serde(default, deserialize_with = "lenient")]
    c: ConfidenceDto,
    #[serde(default, skip_serializing_if = "is_zero", deserialize_with = "count")]
    cc: u32,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "time")]
    ia: Option<u32>,
    /// `[id, author, committer]` of the integration commit; its time is `ia`. Absent in notes written before the commit
    /// was recorded
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    x: Option<(String, String, String)>,
  },
  #[serde(rename = "n")]
  NotIntegrated {
    #[serde(default, skip_serializing_if = "is_zero", deserialize_with = "count")]
    tc: u32,
    #[serde(default, skip_serializing_if = "is_zero", deserialize_with = "count")]
    ic: u32,
    #[serde(default, skip_serializing_if = "is_zero", deserialize_with = "count")]
    oc: u32,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "time")]
    ia: Option<u32>,
    /// The baseline of the check (`Recheck::baseline`)
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    b: Option<String>,
    /// The commits without an equivalent (`Recheck::orphaned`)
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    u: Option<Vec<String>>,
  },
  #[serde(rename = "p")]
  Partial {
    #[serde(default, skip_serializing_if = "is_zero", deserialize_with = "count")]
    m: u32,
  },
}

/// "h", missing or unknown is High, as in the notes written before the confidence was recorded
#[derive(Default, Serialize, Deserialize)]
enum ConfidenceDto {
  #[serde(rename = "e")]
  Exact,
  #[default]
  #[serde(rename = "h")]
  High,
}

#[expect(clippy::trivially_copy_pass_by_ref, reason = "the signature of skip_serializing_if")]
const fn is_zero(value: &u32) -> bool {
  *value == 0
}

// The reader forgives a field of another type, as the notes of older versions may have one: the field is left out
// (a count is 0), the rest of the note still reads

fn lenient<'de, D: Deserializer<'de>, T: DeserializeOwned + Default>(deserializer: D) -> Result<T, D::Error> {
  Ok(serde_json::from_value(Value::deserialize(deserializer)?).unwrap_or_default())
}

/// A count as `u32`, saturating values that do not fit
fn count<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
  Ok(time(deserializer)?.unwrap_or(0))
}

/// Unix seconds as `u32`, saturating values that do not fit
fn time<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u32>, D::Error> {
  Ok(Value::deserialize(deserializer)?.as_u64().map(|value| u32::try_from(value).unwrap_or(u32::MAX)))
}

/// Serialize a detection result to compact JSON for git notes storage; `check` is written only with a `NotIntegrated`
/// result
pub fn serialize_for_cache(info: &BranchIntegrationInfo, check: Option<&Recheck>) -> Result<String> {
  let status = match &info.status {
    BranchIntegrationStatus::Integrated {
      integrated_at,
      confidence,
      commit_count,
      commit,
    } => StatusDto::Integrated {
      c: match confidence {
        IntegrationConfidence::Exact => ConfidenceDto::Exact,
        IntegrationConfidence::High => ConfidenceDto::High,
      },
      cc: *commit_count,
      ia: *integrated_at,
      x: commit.as_ref().map(|commit| (commit.id.clone(), commit.author.clone(), commit.committer.clone())),
    },
    BranchIntegrationStatus::NotIntegrated {
      total_commit_count,
      integrated_count,
      orphaned_count,
      integrated_at,
    } => StatusDto::NotIntegrated {
      tc: *total_commit_count,
      ic: *integrated_count,
      oc: *orphaned_count,
      ia: *integrated_at,
      b: check.map(|check| check.baseline.clone()),
      u: check.map(|check| check.orphaned.clone()),
    },
    BranchIntegrationStatus::Partial { missing } => StatusDto::Partial { m: *missing },
  };
  let note = NoteDto {
    v: DETECTION_CACHE_VERSION,
    s: status,
    sum: info.summary.clone(),
  };
  Ok(serde_json::to_string(&note)?)
}

/// Deserialize compact JSON from git notes (the info with an empty name)
pub fn deserialize_from_cache(json: &str) -> Result<CachedDetection> {
  let note: NoteDto = serde_json::from_str(json)?;
  let mut check = None;
  let status = match note.s {
    StatusDto::Integrated { c, cc, ia, x } => BranchIntegrationStatus::Integrated {
      integrated_at: ia,
      confidence: match c {
        ConfidenceDto::Exact => IntegrationConfidence::Exact,
        ConfidenceDto::High => IntegrationConfidence::High,
      },
      commit_count: cc,
      commit: ia
        .zip(x)
        .and_then(|(time, (id, author, committer))| (!id.is_empty()).then_some(UpstreamCommit { id, author, committer, time })),
    },
    StatusDto::NotIntegrated { tc, ic, oc, ia, b, u } => {
      // without a baseline or its orphaned commits (version 1) the note is detected again in full
      check = b.filter(|baseline| !baseline.is_empty()).zip(u).map(|(baseline, orphaned)| Recheck { baseline, orphaned });
      BranchIntegrationStatus::NotIntegrated {
        total_commit_count: tc,
        integrated_count: ic,
        orphaned_count: oc,
        integrated_at: ia,
      }
    }
    StatusDto::Partial { m } => BranchIntegrationStatus::Partial { missing: m },
  };
  Ok(CachedDetection {
    info: BranchIntegrationInfo {
      name: String::new(), // Empty name - to be filled by caller
      summary: note.sum,
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
  pub const fn new(git: &'a GitCommandExecutor, repo: &'a str) -> Self {
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
    // a cache: a write that lost to another writer runs once more, then is a warning; an identity git cannot form fails
    // the sync, as its mapping notes do
    write_cache_notes("integration", || write_notes(self.git, self.repo, NOTES_REF, &notes).context("Failed to write notes"))?;
    debug!("Cached detection for {} branches", notes.len());
    Ok(())
  }
}

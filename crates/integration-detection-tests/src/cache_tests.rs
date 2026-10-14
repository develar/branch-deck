//! Cache-specific unit tests for integration detection

use branch_integration::cache::{DETECTION_CACHE_VERSION, deserialize_from_cache, serialize_for_cache};
use sync_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus, IntegrationConfidence};
use sync_types::upstream_commit::UpstreamCommit;
use test_log::test;

/// Test cache JSON serialization and parsing
#[test]
fn test_cache_serialization() {
  let info = BranchIntegrationInfo {
    name: "test-branch".to_string(),
    summary: "test summary".to_string(),
    status: BranchIntegrationStatus::Integrated {
      integrated_at: Some(1234567890),
      confidence: IntegrationConfidence::High,
      commit_count: 3,
      commit: None,
    },
  };

  // Test JSON serialization with short field names
  let json = serialize_for_cache(&info).unwrap();
  assert!(json.contains(&format!("\"v\":{DETECTION_CACHE_VERSION}"))); // version field
  // The actual JSON structure uses "k" for the variant key
  assert!(json.contains("\"k\":\"i\"")); // status: integrated
  assert!(json.contains("\"cc\":3")); // commit_count
  assert!(json.contains("\"ia\":1234567890")); // integrated_at
  assert!(json.contains("\"c\":\"h\"")); // confidence: high

  // Test parsing with the actual parse function
  let mut parsed = deserialize_from_cache(&json).unwrap();
  if let BranchIntegrationStatus::Integrated {
    commit_count,
    integrated_at,
    confidence,
    commit,
  } = &parsed.status
  {
    assert_eq!(*commit_count, 3);
    assert_eq!(*integrated_at, Some(1234567890));
    assert_eq!(*confidence, IntegrationConfidence::High);
    assert_eq!(*commit, None);
  } else {
    panic!("Expected Integrated status");
  }

  // Test round-trip conversion (set branch name since parse doesn't know it)
  parsed.name = "test-branch".to_string();
  assert_eq!(parsed.name, "test-branch");
  assert_eq!(parsed.summary, "test summary");
}

/// The commit that integrated the branch survives the cache, its time taken from `ia`
#[test]
fn integration_commit_round_trips_through_the_cache() {
  let commit = UpstreamCommit {
    id: "0123456789abcdef0123456789abcdef01234567".to_string(),
    author: "Jane Doe".to_string(),
    committer: "Safe Push Robot".to_string(),
    time: 1_760_000_000,
  };
  let info = BranchIntegrationInfo {
    name: "test-branch".to_string(),
    summary: String::new(),
    status: BranchIntegrationStatus::Integrated {
      integrated_at: Some(commit.time),
      confidence: IntegrationConfidence::High,
      commit_count: 2,
      commit: Some(commit.clone()),
    },
  };

  let json = serialize_for_cache(&info).unwrap();
  assert!(json.contains(r#""x":["0123456789abcdef0123456789abcdef01234567","Jane Doe","Safe Push Robot"]"#), "{json}");
  let parsed = deserialize_from_cache(&json).unwrap();
  assert!(
    matches!(&parsed.status, BranchIntegrationStatus::Integrated { commit: Some(parsed), .. } if *parsed == commit),
    "{:?}",
    parsed.status
  );
}

/// A note written before the commit was recorded still parses, without a commit
#[test]
fn note_without_integration_commit_still_parses() {
  let parsed = deserialize_from_cache(r#"{"v":1,"s":{"k":"i","c":"h","cc":2,"ia":1760000000}}"#).unwrap();
  assert!(
    matches!(
      parsed.status,
      BranchIntegrationStatus::Integrated {
        commit: None,
        integrated_at: Some(1_760_000_000),
        commit_count: 2,
        ..
      }
    ),
    "{:?}",
    parsed.status
  );
}

/// Test that zero values are omitted from cache
#[test]
fn test_cache_omits_zero_values() {
  // Test NotIntegrated with zeros
  let info = BranchIntegrationInfo {
    name: "test-branch".to_string(),
    summary: String::new(), // empty summary should be omitted
    status: BranchIntegrationStatus::NotIntegrated {
      total_commit_count: 0, // should be omitted
      integrated_count: 0,   // should be omitted
      orphaned_count: 0,     // should be omitted
      integrated_at: None,   // should be omitted
    },
  };

  let json = serialize_for_cache(&info).unwrap();

  // Should only contain version and status kind, no numeric fields
  assert!(json.contains("\"k\":\"n\""));
  assert!(!json.contains("\"tc\":")); // total_commit_count omitted
  assert!(!json.contains("\"ic\":")); // integrated_count omitted  
  assert!(!json.contains("\"oc\":")); // orphaned_count omitted
  assert!(!json.contains("\"ia\":")); // integrated_at omitted
  assert!(!json.contains("\"sum\"")); // summary omitted

  // Test round-trip with zeros
  let parsed = deserialize_from_cache(&json).unwrap();
  if let BranchIntegrationStatus::NotIntegrated {
    total_commit_count,
    integrated_count,
    orphaned_count,
    integrated_at,
  } = parsed.status
  {
    assert_eq!(total_commit_count, 0);
    assert_eq!(integrated_count, 0);
    assert_eq!(orphaned_count, 0);
    assert_eq!(integrated_at, None);
  } else {
    panic!("Expected NotIntegrated status");
  }
  assert_eq!(parsed.summary, "");

  // Test Integrated with zero commit count
  let zero_commits_info = BranchIntegrationInfo {
    name: "test-branch".to_string(),
    summary: String::new(),
    status: BranchIntegrationStatus::Integrated {
      integrated_at: None,
      confidence: IntegrationConfidence::High,
      commit_count: 0, // should be omitted
      commit: None,
    },
  };

  let zero_json = serialize_for_cache(&zero_commits_info).unwrap();
  assert!(!zero_json.contains("\"cc\":")); // commit_count omitted
  assert!(!zero_json.contains("\"ia\":")); // integrated_at omitted

  let zero_parsed = deserialize_from_cache(&zero_json).unwrap();
  if let BranchIntegrationStatus::Integrated { commit_count, integrated_at, .. } = zero_parsed.status {
    assert_eq!(commit_count, 0);
    assert_eq!(integrated_at, None);
  } else {
    panic!("Expected Integrated status");
  }
}

/// Test cache with exact confidence
#[test]
fn test_cache_exact_confidence() {
  let info = BranchIntegrationInfo {
    name: "test-branch".to_string(),
    summary: "exact confidence".to_string(),
    status: BranchIntegrationStatus::Integrated {
      integrated_at: Some(1234567890),
      confidence: IntegrationConfidence::Exact,
      commit_count: 5,
      commit: None,
    },
  };

  let json = serialize_for_cache(&info).unwrap();
  assert!(json.contains("\"c\":\"e\"")); // confidence: exact

  let parsed = deserialize_from_cache(&json).unwrap();
  if let BranchIntegrationStatus::Integrated { confidence, .. } = parsed.status {
    assert_eq!(confidence, IntegrationConfidence::Exact);
  } else {
    panic!("Expected Integrated status");
  }
}

/// Test not integrated cache
#[test]
fn test_not_integrated_cache() {
  let info = BranchIntegrationInfo {
    name: "test-branch".to_string(),
    summary: "not integrated".to_string(),
    status: BranchIntegrationStatus::NotIntegrated {
      total_commit_count: 10,
      integrated_count: 7,
      orphaned_count: 3,
      integrated_at: Some(1234567890),
    },
  };

  let json = serialize_for_cache(&info).unwrap();
  assert!(json.contains("\"k\":\"n\"")); // not integrated
  assert!(json.contains("\"tc\":10")); // total count
  assert!(json.contains("\"ic\":7")); // integrated count
  assert!(json.contains("\"oc\":3")); // orphaned count

  let parsed = deserialize_from_cache(&json).unwrap();
  if let BranchIntegrationStatus::NotIntegrated {
    total_commit_count,
    integrated_count,
    orphaned_count,
    integrated_at,
  } = parsed.status
  {
    assert_eq!(total_commit_count, 10);
    assert_eq!(integrated_count, 7);
    assert_eq!(orphaned_count, 3);
    assert_eq!(integrated_at, Some(1234567890));
  } else {
    panic!("Expected NotIntegrated status");
  }
}

/// Test partial cache
#[test]
fn test_partial_cache() {
  let info = BranchIntegrationInfo {
    name: "test-branch".to_string(),
    summary: "partial".to_string(),
    status: BranchIntegrationStatus::Partial { missing: 2 },
  };

  let json = serialize_for_cache(&info).unwrap();
  assert!(json.contains("\"k\":\"p\"")); // partial
  assert!(json.contains("\"m\":2")); // missing count

  let parsed = deserialize_from_cache(&json).unwrap();
  if let BranchIntegrationStatus::Partial { missing } = parsed.status {
    assert_eq!(missing, 2);
  } else {
    panic!("Expected Partial status");
  }
}

/// Test that demonstrates the JSON size reduction benefit
#[test]
fn test_json_size_optimization() {
  let info = BranchIntegrationInfo {
    name: "test-branch".to_string(),
    summary: "december summary".to_string(),
    status: BranchIntegrationStatus::Integrated {
      integrated_at: Some(1703116800),
      confidence: IntegrationConfidence::High,
      commit_count: 5,
      commit: None,
    },
  };

  // Serialize to compact JSON (no spaces)
  let json = serialize_for_cache(&info).unwrap();

  // Verify it contains optimized field names
  assert!(json.contains(&format!("\"v\":{DETECTION_CACHE_VERSION}"))); // version
  assert!(json.contains("\"k\":\"i\"")); // status: integrated with variant key
  assert!(json.contains("\"cc\":5")); // commit_count
  assert!(json.contains("\"ia\":1703116800")); // integrated_at

  // Verify size is reasonable (should be much smaller than verbose field names)
  assert!(json.len() < 150, "Optimized JSON should be compact, got {} bytes", json.len());
}

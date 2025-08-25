//! Cache-specific unit tests for integration detection

use branch_integration::cache::{DETECTION_CACHE_VERSION, parse_cached_note, serialize_for_cache};
use sync_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus, IntegrationConfidence};
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
    },
  };

  // Test JSON serialization with short field names
  let json = serialize_for_cache(&info).unwrap();
  assert!(json.contains(&format!("\"v\":{}", DETECTION_CACHE_VERSION))); // version field
  // The actual JSON structure uses "k" for the variant key
  assert!(json.contains("\"k\":\"i\"")); // status: integrated
  assert!(json.contains("\"cc\":3")); // commit_count
  assert!(json.contains("\"ia\":1234567890")); // integrated_at
  assert!(json.contains("\"c\":\"h\"")); // confidence: high

  // Test parsing with the actual parse function
  let mut parsed = parse_cached_note(&json).unwrap();
  if let BranchIntegrationStatus::Integrated {
    commit_count,
    integrated_at,
    confidence,
  } = &parsed.status
  {
    assert_eq!(*commit_count, 3);
    assert_eq!(*integrated_at, Some(1234567890));
    assert_eq!(*confidence, IntegrationConfidence::High);
  } else {
    panic!("Expected Integrated status");
  }

  // Test round-trip conversion (set branch name since parse doesn't know it)
  parsed.name = "test-branch".to_string();
  assert_eq!(parsed.name, "test-branch");
  assert_eq!(parsed.summary, "test summary");
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
  let parsed = parse_cached_note(&json).unwrap();
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
    },
  };

  let zero_json = serialize_for_cache(&zero_commits_info).unwrap();
  assert!(!zero_json.contains("\"cc\":")); // commit_count omitted
  assert!(!zero_json.contains("\"ia\":")); // integrated_at omitted

  let zero_parsed = parse_cached_note(&zero_json).unwrap();
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
    },
  };

  let json = serialize_for_cache(&info).unwrap();
  assert!(json.contains("\"c\":\"e\"")); // confidence: exact

  let parsed = parse_cached_note(&json).unwrap();
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

  let parsed = parse_cached_note(&json).unwrap();
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

  let parsed = parse_cached_note(&json).unwrap();
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
    },
  };

  // Serialize to compact JSON (no spaces)
  let json = serialize_for_cache(&info).unwrap();

  // Verify it contains optimized field names
  assert!(json.contains(&format!("\"v\":{}", DETECTION_CACHE_VERSION))); // version
  assert!(json.contains("\"k\":\"i\"")); // status: integrated with variant key
  assert!(json.contains("\"cc\":5")); // commit_count
  assert!(json.contains("\"ia\":1703116800")); // integrated_at

  // Verify size is reasonable (should be much smaller than verbose field names)
  assert!(json.len() < 150, "Optimized JSON should be compact, got {} bytes", json.len());
}

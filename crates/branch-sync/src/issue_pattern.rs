//! Shared utilities for detecting issue number patterns in commit messages

/// Skip [category] prefix if present, returns new position or None if newline encountered
fn skip_bracket_prefix(bytes: &[u8], mut i: usize) -> Option<usize> {
  let len = bytes.len();

  if i < len && bytes[i] == b'[' {
    i += 1;
    // Find the closing bracket
    while i < len && bytes[i] != b']' {
      if bytes[i] == b'\n' {
        return None; // Hit newline before closing bracket
      }
      i += 1;
    }
    if i < len && bytes[i] == b']' {
      i += 1; // Skip the closing bracket
              // Skip any whitespace after bracket
      while i < len && bytes[i].is_ascii_whitespace() {
        if bytes[i] == b'\n' {
          return None; // Hit newline
        }
        i += 1;
      }
      return Some(i);
    }
    return None; // No closing bracket found
  }

  Some(i) // No bracket prefix, return unchanged
}

/// Skip semantic commit prefix (e.g., "fix:", "feat(scope):"), returns new position or None
fn skip_semantic_prefix(bytes: &[u8], mut i: usize) -> Option<usize> {
  let len = bytes.len();

  if i < len && bytes[i].is_ascii_lowercase() {
    let type_start = i;
    // Collect the type (fix, feat, chore, etc.)
    while i < len && bytes[i].is_ascii_lowercase() {
      i += 1;
    }

    // Check if we hit newline or end
    if i >= len || bytes[i] == b'\n' {
      return Some(type_start); // Not a semantic prefix
    }

    // Check for optional (scope)
    if bytes[i] == b'(' {
      i += 1;
      // Skip to closing paren
      while i < len && bytes[i] != b')' {
        if bytes[i] == b'\n' {
          return Some(type_start); // Not a semantic prefix
        }
        i += 1;
      }
      if i < len && bytes[i] == b')' {
        i += 1;
      } else {
        return Some(type_start); // No closing paren, not a semantic prefix
      }
    }

    // Check if we hit newline or end
    if i >= len || bytes[i] == b'\n' {
      return Some(type_start); // Not a semantic prefix
    }

    // Check for colon
    if bytes[i] == b':' {
      i += 1;
      // Skip any whitespace after colon
      while i < len && bytes[i].is_ascii_whitespace() {
        if bytes[i] == b'\n' {
          return None; // Hit newline after semantic prefix
        }
        i += 1;
      }
      return Some(i); // Successfully skipped semantic prefix
    }

    return Some(type_start); // Not a semantic prefix
  }

  Some(i) // No semantic prefix, return unchanged
}

/// Try to extract issue pattern at current position
fn extract_issue_at(text: &str, bytes: &[u8], mut i: usize) -> Option<String> {
  let len = bytes.len();

  if i >= len || bytes[i] == b'\n' || !bytes[i].is_ascii_uppercase() {
    return None;
  }

  let start = i;

  // Collect uppercase letters
  while i < len && bytes[i].is_ascii_uppercase() {
    i += 1;
  }

  // Check for hyphen
  if i < len && bytes[i] == b'-' {
    i += 1; // Skip hyphen

    // Must have at least one digit
    if i < len && bytes[i].is_ascii_digit() {
      // Collect all digits
      while i < len && bytes[i].is_ascii_digit() {
        i += 1;
      }

      // Check word boundary at end
      if i >= len || bytes[i] == b'\n' || !bytes[i].is_ascii_alphanumeric() {
        return Some(text[start..i].to_string());
      }
    }
  }

  None
}

/// Manually find issue number pattern (e.g., JIRA-123) without regex
/// Returns the first issue number found, or None
/// Only searches up to the first newline (subject line only)
pub fn find_issue_number(text: &str) -> Option<String> {
  let bytes = text.as_bytes();

  // Early exit for strings too short to contain a valid pattern
  if bytes.len() < 3 {
    return None; // Minimum: "A-1"
  }

  let mut i = 0;

  // Skip [category] prefix if present
  i = skip_bracket_prefix(bytes, i)?;

  // Skip semantic commit prefix if present
  i = skip_semantic_prefix(bytes, i)?;

  // Try to extract issue pattern at current position
  extract_issue_at(text, bytes, i)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_find_issue_number() {
    // Test finding issue numbers at start
    assert_eq!(find_issue_number("ABC-123 Fix the bug"), Some("ABC-123".to_string()));
    assert_eq!(find_issue_number("ISSUE-1"), Some("ISSUE-1".to_string()));
    assert_eq!(find_issue_number("A-1 minimal"), Some("A-1".to_string()));

    // Test that arbitrary text before issue is NOT found
    assert_eq!(find_issue_number("Fix JIRA-456 in code"), None);
    assert_eq!(find_issue_number("Update (ABC-789) docs"), None);

    // But [tag] prefix IS recognized
    assert_eq!(find_issue_number("[tag] XYZ-999: title"), Some("XYZ-999".to_string()));

    // Test cases that should NOT match
    assert_eq!(find_issue_number("Fix the bug"), None);
    assert_eq!(find_issue_number("abc-123 lowercase"), None);
    assert_eq!(find_issue_number("ABC- missing number"), None);
    assert_eq!(find_issue_number("-123 missing prefix"), None);
    assert_eq!(find_issue_number("ABC-"), None);
    assert_eq!(find_issue_number("ABC-abc not digits"), None);
    assert_eq!(find_issue_number(""), None);

    // Edge cases
    assert_eq!(find_issue_number("ABC-123-456"), Some("ABC-123".to_string())); // Stops at first valid pattern
    assert_eq!(find_issue_number("ABC-123ABC-456"), None); // No word boundary after first pattern
    assert_eq!(find_issue_number("prefixABC-123"), None); // Not at word boundary
    assert_eq!(find_issue_number("ABC--123"), None); // Double hyphen
    assert_eq!(find_issue_number("ABC123-456"), None); // No hyphen after letters

    // Multiple patterns - only finds if at start or after recognized prefix
    assert_eq!(find_issue_number("Fix ABC-123 and DEF-456"), None); // "Fix" is not a recognized prefix
    assert_eq!(find_issue_number("ABC-123ABC-456 should find DEF-789"), None); // No valid issue at start

    // Test that it stops at newline (only searches subject line)
    assert_eq!(find_issue_number("No issue here\nBUT-123 in the body"), None);
    assert_eq!(find_issue_number("ABC-123 in subject\nDEF-456 in body"), Some("ABC-123".to_string()));
    assert_eq!(find_issue_number("Subject line\n\nXYZ-789 in body"), None);

    // Test [category] prefix handling
    assert_eq!(find_issue_number("[threading] IJPL-163558: Fix observability"), Some("IJPL-163558".to_string()));
    assert_eq!(find_issue_number("[subsystem] ABC-456: Update documentation"), Some("ABC-456".to_string()));
    assert_eq!(find_issue_number("[auth] No issue here"), None);

    // Test semantic commit prefix handling
    assert_eq!(find_issue_number("fix: ABC-123 resolve bug"), Some("ABC-123".to_string()));
    assert_eq!(find_issue_number("feat(auth): DEF-456 add login"), Some("DEF-456".to_string()));
    assert_eq!(find_issue_number("chore: no issue here"), None);

    // Test combination of prefixes
    assert_eq!(find_issue_number("[category] fix: GHI-789 combined"), Some("GHI-789".to_string()));
    assert_eq!(find_issue_number("[test] feat(api): JKL-012 all prefixes"), Some("JKL-012".to_string()));
  }
}

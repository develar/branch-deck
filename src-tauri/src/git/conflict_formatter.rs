use crate::git::fast_cherry_pick::ConflictInfo;
use similar::TextDiff;

/// Format conflicts in a user-friendly way
pub fn format_conflicts_for_user(conflicts: &[ConflictInfo]) -> String {
  // Pre-allocate with estimated capacity to reduce reallocations
  let estimated_capacity = conflicts.len() * 512 + 1024; // ~512 chars per conflict + headers
  let mut output = String::with_capacity(estimated_capacity);

  output.push_str("\n🔥 **MERGE CONFLICTS DETECTED** 🔥\n");
  output.push_str(&format!("Found conflicts in {} file(s). Please review and resolve manually:\n\n", conflicts.len()));

  for (i, conflict) in conflicts.iter().enumerate() {
    output.push_str(&format!("{}. 📄 **{}**\n", i + 1, conflict.path.display()));

    // Add a preview of the conflict using unified diff
    let diff = TextDiff::from_lines(&conflict.our_content, &conflict.their_content);

    output.push_str("   ┌─ Conflict Preview:\n");

    // Use unified diff format which is easier to handle
    let unified_diff = diff
      .unified_diff()
      .context_radius(3)
      .header("Our version (target)", "Their version (cherry-pick)")
      .to_string();

    if unified_diff.is_empty() {
      output.push_str("   │ (Files are identical or binary)\n");
    } else {
      for (line_count, line) in unified_diff.lines().enumerate() {
        if line_count >= 15 {
          // Limit preview
          output.push_str("   │ ... (truncated, more conflicts below)\n");
          break;
        }

        if line.starts_with("@@") {
          output.push_str(&format!("   │ 📍 {line}\n"));
        } else if line.starts_with("+") && !line.starts_with("+++") {
          output.push_str(&format!("   │ 🔵 {line}\n"));
        } else if line.starts_with("-") && !line.starts_with("---") {
          output.push_str(&format!("   │ 🔴 {line}\n"));
        } else if line.starts_with(" ") {
          output.push_str(&format!("   │     {line}\n"));
        } else if !line.starts_with("---") && !line.starts_with("+++") {
          output.push_str(&format!("   │ {line}\n"));
        }
      }
    }

    output.push_str("   └─\n\n");
  }

  output.push_str("⚠️  **What to do next:**\n");
  output.push_str("   1. Review the conflicting files manually\n");
  output.push_str("   2. Choose which changes to keep\n");
  output.push_str("   3. Make sure the merge makes sense in context\n");
  output.push_str("   4. Test your changes\n\n");

  output
}

/// Render a conflict as a user-friendly diff using the similar crate
pub fn render_conflict_diff(conflict: &ConflictInfo) -> String {
  let diff = TextDiff::from_lines(&conflict.our_content, &conflict.their_content);

  let mut output = String::new();

  // Add a brief explanation
  output.push_str("┌─ Conflict Details:\n");
  output.push_str("│  🔴 Our version (target branch)\n");
  output.push_str("│  🔵 Their version (cherry-picked commit)\n");
  output.push_str("└─\n\n");

  // Generate unified diff with context
  let unified_diff = diff.unified_diff().context_radius(3).header("🔴 Our version", "🔵 Their version").to_string();

  if unified_diff.is_empty() {
    output.push_str("(Binary file or no textual differences detected)\n");
  } else {
    // Format the diff nicely
    for line in unified_diff.lines() {
      if line.starts_with("@@") {
        output.push_str(&format!("\n📍 {line}\n"));
      } else if line.starts_with('+') && !line.starts_with("+++") {
        output.push_str(&format!("🔵 {line}\n"));
      } else if line.starts_with('-') && !line.starts_with("---") {
        output.push_str(&format!("🔴 {line}\n"));
      } else if line.starts_with(' ') {
        output.push_str(&format!("   {line}\n"));
      } else if !line.starts_with("---") && !line.starts_with("+++") {
        output.push_str(&format!("{line}\n"));
      }
    }
  }

  output.push('\n');
  output
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::PathBuf;

  #[test]
  fn test_format_conflicts_for_user_basic() {
    let conflicts = vec![ConflictInfo {
      path: PathBuf::from("test.txt"),
      our_content: "line1\nour_line2\nline3".to_string(),
      their_content: "line1\ntheir_line2\nline3".to_string(),
      ancestor_content: Some("line1\noriginal_line2\nline3".to_string()),
    }];

    let formatted = format_conflicts_for_user(&conflicts);

    // Check main structure
    assert!(formatted.contains("🔥 **MERGE CONFLICTS DETECTED** 🔥"));
    assert!(formatted.contains("Found conflicts in 1 file(s)"));
    assert!(formatted.contains("📄 **test.txt**"));
    assert!(formatted.contains("Conflict Preview:"));
    assert!(formatted.contains("⚠️  **What to do next:**"));

    // Check diff content
    assert!(formatted.contains("🔴 -")); // Deletions
    assert!(formatted.contains("🔵 +")); // Additions
  }

  #[test]
  fn test_format_conflicts_for_user_multiple_files() {
    let conflicts = vec![
      ConflictInfo {
        path: PathBuf::from("file1.txt"),
        our_content: "content1".to_string(),
        their_content: "different1".to_string(),
        ancestor_content: None,
      },
      ConflictInfo {
        path: PathBuf::from("file2.txt"),
        our_content: "content2".to_string(),
        their_content: "different2".to_string(),
        ancestor_content: None,
      },
    ];

    let formatted = format_conflicts_for_user(&conflicts);

    assert!(formatted.contains("Found conflicts in 2 file(s)"));
    assert!(formatted.contains("1. 📄 **file1.txt**"));
    assert!(formatted.contains("2. 📄 **file2.txt**"));
  }

  #[test]
  fn test_format_conflicts_for_user_empty_diff() {
    let conflicts = vec![ConflictInfo {
      path: PathBuf::from("binary.dat"),
      our_content: "same content".to_string(),
      their_content: "same content".to_string(),
      ancestor_content: None,
    }];

    let formatted = format_conflicts_for_user(&conflicts);

    assert!(formatted.contains("(Files are identical or binary)"));
  }

  #[test]
  fn test_format_conflicts_for_user_long_diff() {
    // Create a conflict with many lines to test truncation
    let our_lines: Vec<String> = (1..=20).map(|i| format!("our_line_{i}")).collect();
    let their_lines: Vec<String> = (1..=20).map(|i| format!("their_line_{i}")).collect();

    let conflicts = vec![ConflictInfo {
      path: PathBuf::from("long.txt"),
      our_content: our_lines.join("\n"),
      their_content: their_lines.join("\n"),
      ancestor_content: None,
    }];

    let formatted = format_conflicts_for_user(&conflicts);

    // Should contain truncation message
    assert!(formatted.contains("... (truncated, more conflicts below)"));
  }

  #[test]
  fn test_format_conflicts_for_user_realistic_code() {
    let conflicts = vec![ConflictInfo {
      path: PathBuf::from("calculator.js"),
      our_content:
        "function calculateTotal(items) {\n  let total = 0;\n  for (let item of items) {\n    total += item.price * item.quantity;\n  }\n  return Math.round(total * 100) / 100;\n}"
          .to_string(),
      their_content: "function calculateTotal(items) {\n  let total = 0;\n  for (let item of items) {\n    total += item.price + item.tax;\n  }\n  return total.toFixed(2);\n}"
        .to_string(),
      ancestor_content: Some("function calculateTotal(items) {\n  let total = 0;\n  for (let item of items) {\n    total += item.price;\n  }\n  return total;\n}".to_string()),
    }];

    let formatted = format_conflicts_for_user(&conflicts);

    // Should show meaningful code differences
    assert!(formatted.contains("calculator.js"));
    assert!(formatted.contains("quantity")); // Our version
    assert!(formatted.contains("tax")); // Their version
    assert!(formatted.contains("Math.round")); // Our version
    assert!(formatted.contains("toFixed")); // Their version
  }

  #[test]
  fn test_render_conflict_diff_basic() {
    let conflict = ConflictInfo {
      path: PathBuf::from("test.txt"),
      our_content: "line1\nour_line2\nline3".to_string(),
      their_content: "line1\ntheir_line2\nline3".to_string(),
      ancestor_content: Some("line1\noriginal_line2\nline3".to_string()),
    };

    let rendered = render_conflict_diff(&conflict);

    // Check basic structure
    assert!(rendered.contains("┌─ Conflict Details:"));
    assert!(rendered.contains("🔴 Our version (target branch)"));
    assert!(rendered.contains("🔵 Their version (cherry-picked commit)"));

    // Check diff content
    assert!(rendered.contains("🔴 -")); // Deletions
    assert!(rendered.contains("🔵 +")); // Additions
    assert!(rendered.contains("our_line2"));
    assert!(rendered.contains("their_line2"));
  }

  #[test]
  fn test_render_conflict_diff_identical_content() {
    let conflict = ConflictInfo {
      path: PathBuf::from("same.txt"),
      our_content: "same content".to_string(),
      their_content: "same content".to_string(),
      ancestor_content: None,
    };

    let rendered = render_conflict_diff(&conflict);

    // Should indicate no differences
    assert!(rendered.contains("(Binary file or no textual differences detected)"));
  }
}

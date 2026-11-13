use crate::collapse::{MAX_OPEN_CHANGED_LINES, collapse_of};
use crate::model::{Collapse, DiffContent};
use generated_files::names::GeneratedName;
use pretty_assertions::assert_eq;
use text_diff::model::TextDiff;

fn text(additions: u32, deletions: u32) -> DiffContent {
  DiffContent::Text(TextDiff {
    additions,
    deletions,
    ..TextDiff::default()
  })
}

#[test]
fn a_generated_name_no_text_and_many_changed_lines_start_a_file_collapsed() {
  assert_eq!(collapse_of("src/a.rs", &text(3, 1)), None);
  assert_eq!(collapse_of("src/a.rs", &text(MAX_OPEN_CHANGED_LINES, 0)), None);
  assert_eq!(collapse_of("src/a.rs", &text(900, 200)), Some(Collapse::Large { changed: 1100 }));
  assert_eq!(collapse_of("img.png", &DiffContent::Binary), Some(Collapse::NoText));
  assert_eq!(collapse_of("big.txt", &DiffContent::TooLarge { old_bytes: 0, new_bytes: 1 }), Some(Collapse::NoText));
  assert_eq!(collapse_of("x", &DiffContent::Missing), Some(Collapse::NoText));
  // the name says the most: a lock file is a lock file when it is large or binary too
  let lock = Some(Collapse::Generated { name: GeneratedName::LockFile });
  assert_eq!(collapse_of("pnpm-lock.yaml", &text(2, 2)), lock);
  assert_eq!(collapse_of("Cargo.lock", &text(3000, 0)), lock);
  assert_eq!(collapse_of("bun.lockb", &DiffContent::Binary), lock);
  assert_eq!(collapse_of("dist/app.min.js", &text(1, 1)), Some(Collapse::Generated { name: GeneratedName::Minified }));
}

use crate::attributes::{PathAttribute, read_attributes};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn attribute(path: &str, attribute: &str, value: &str) -> PathAttribute {
  PathAttribute {
    path: path.to_string(),
    attribute: attribute.to_string(),
    value: value.to_string(),
  }
}

/// The attributes of HEAD, not of the working tree, for paths with a space, a tab and a non-ASCII letter
#[test]
fn reads_the_attributes_of_the_source_tree() {
  let repo = TestRepo::new();
  repo.create_commit(".gitattributes", ".gitattributes", "gen/** linguist-generated\n*.bin -diff\n*.txt eol=lf\n");
  // not committed: HEAD's attributes decide
  std::fs::write(repo.path().join(".gitattributes"), "*.md linguist-generated\n").unwrap();
  let (git, path) = (repo.git_executor(), repo.path_str());
  let paths = ["gen/a b.rs", "x\ty.bin", "ü.txt", "README.md"];
  let attributes = read_attributes(git, path, "HEAD", &["linguist-generated", "diff", "eol"], paths).unwrap();
  assert_eq!(
    attributes,
    [
      attribute("gen/a b.rs", "linguist-generated", "set"),
      attribute("gen/a b.rs", "diff", "unspecified"),
      attribute("gen/a b.rs", "eol", "unspecified"),
      attribute("x\ty.bin", "linguist-generated", "unspecified"),
      attribute("x\ty.bin", "diff", "unset"),
      attribute("x\ty.bin", "eol", "unspecified"),
      attribute("ü.txt", "linguist-generated", "unspecified"),
      attribute("ü.txt", "diff", "unspecified"),
      attribute("ü.txt", "eol", "lf"),
      attribute("README.md", "linguist-generated", "unspecified"),
      attribute("README.md", "diff", "unspecified"),
      attribute("README.md", "eol", "unspecified"),
    ]
  );
  assert_eq!(read_attributes(git, path, "HEAD", &["diff"], []).unwrap(), []);
}

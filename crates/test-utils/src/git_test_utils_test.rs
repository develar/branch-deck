use crate::git_test_utils::{TestRepo, git_canonical_path, windows_git_form};

/// The expected directory of a test is the one git names, on every OS: the temporary directory of macOS is a symlink,
/// and Windows gives the verbatim form with backslashes
#[test]
fn the_canonical_path_is_the_top_level_git_prints() {
  let repo = TestRepo::new();
  assert_eq!(git_canonical_path(repo.path()), repo.git(&["rev-parse", "--show-toplevel"]).trim());
}

#[test]
fn a_windows_path_loses_the_verbatim_prefix_and_its_backslashes() {
  assert_eq!(
    windows_git_form(r"\\?\C:\Users\runneradmin\AppData\Local\Temp\.tmpJWiJDv"),
    "C:/Users/runneradmin/AppData/Local/Temp/.tmpJWiJDv"
  );
  assert_eq!(windows_git_form(r"\\?\UNC\server\share\repo"), "//server/share/repo");
  assert_eq!(windows_git_form(r"C:\Users\me\repo"), "C:/Users/me/repo");
}

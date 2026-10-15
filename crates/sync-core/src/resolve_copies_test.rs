use crate::resolve_copies::windows_display;
use pretty_assertions::assert_eq;

#[test]
fn a_windows_path_has_backslashes_and_the_verbatim_form_only_from_260_characters() {
  assert_eq!(
    windows_display("C:/Users/me/repo/.git/branch-deck/resolve/r1/a.txt"),
    r"C:\Users\me\repo\.git\branch-deck\resolve\r1\a.txt"
  );
  // `canonicalize` gives the verbatim form; a short path loses it
  assert_eq!(windows_display(r"\\?\C:\Users\me\copies"), r"C:\Users\me\copies");
  assert_eq!(windows_display(r"\\?\UNC\server\share\copies"), r"\\server\share\copies");

  let name = "d".repeat(260);
  let long = format!(r"\\?\C:\{name}\a.txt");
  assert_eq!(windows_display(&format!("C:/{name}/a.txt")), long);
  assert_eq!(windows_display(&long), long);
  let long_share = format!(r"\\?\UNC\server\share\{name}");
  assert_eq!(windows_display(&format!("//server/share/{name}")), long_share);
  assert_eq!(windows_display(&long_share), long_share);
}

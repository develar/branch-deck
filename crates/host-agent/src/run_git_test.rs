use super::*;
use pretty_assertions::assert_eq;

#[test]
fn a_login_shell_of_bash_or_sh_on_macos_gets_a_warning() {
  let warning = |argv: &[&str], macos: bool| {
    let argv: Vec<String> = argv.iter().map(|argument| (*argument).to_owned()).collect();
    login_shell_warning(&argv, "git 2.56.0", macos)
  };
  let said = warning(&["bash", "-lc", "make"], true).expect("a warning");
  assert_eq!(
    said,
    "`bash -l` runs /etc/profile, whose path_helper puts /usr/bin first: the command can get the Apple Git of /usr/bin, not git 2.56.0. Use `zsh -lc` or no login shell"
  );
  for login in [
    &["/bin/bash", "-l", "-c", "x"][..],
    &["sh", "--login", "-c", "x"],
    &["bash", "-cl", "x"],
    &["/bin/sh", "-ilc", "x"],
  ] {
    assert!(warning(login, true).is_some(), "{login:?}");
  }
  for other in [
    &["bash", "-lc", "x"][..],
    &["zsh", "-lc", "x"],
    &["bash", "-c", "ls -l"],
    &["bash", "script.sh", "-l"],
    &["bash", "--norc", "-c", "x"],
    &["/usr/bin/env", "bash", "-lc", "x"],
    &[],
  ] {
    let macos = other != ["bash", "-lc", "x"];
    assert_eq!(warning(other, macos), None, "{other:?}");
  }
}

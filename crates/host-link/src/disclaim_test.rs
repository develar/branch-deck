use super::*;
use pretty_assertions::{assert_eq, assert_ne};
use std::process::{Child, Stdio};

/// `pid_t responsibility_get_pid_responsible_for_pid(pid_t)` of libSystem.
fn responsible_for(pid: u32) -> libc::pid_t {
  type Responsible = unsafe extern "C" fn(libc::pid_t) -> libc::pid_t;
  // SAFETY: the name is a C string, and `RTLD_DEFAULT` searches every image of the process.
  let symbol = unsafe { libc::dlsym(libc::RTLD_DEFAULT, c"responsibility_get_pid_responsible_for_pid".as_ptr()) };
  assert!(!symbol.is_null(), "libSystem has no responsibility_get_pid_responsible_for_pid");
  // SAFETY: libSystem exports the symbol as the function of `Responsible`.
  let responsible = unsafe { std::mem::transmute::<*mut c_void, Responsible>(symbol) };
  let pid = libc::pid_t::try_from(pid).expect("a pid");
  // SAFETY: the call only reads the state of the process.
  unsafe { responsible(pid) }
}

/// A `sleep 30` that ends with the test, also when an assertion fails, so nextest sees no leaked process
struct Sleeper(Child);

impl Sleeper {
  fn start(disclaimed: bool) -> Self {
    let mut command = Command::new("/bin/sleep");
    command.arg("30");
    if disclaimed {
      disclaim(&mut command).expect("disclaimed");
    }
    Self(command.spawn().expect("a sleep"))
  }

  fn pid(&self) -> libc::pid_t {
    libc::pid_t::try_from(self.0.id()).expect("a pid")
  }
}

impl Drop for Sleeper {
  fn drop(&mut self) {
    let _ = self.0.kill();
    let _ = self.0.wait();
  }
}

/// The responsible process of a process that a terminal app starts is that app. A session without a terminal app, such
/// as a background agent session, starts the test with another responsible process, and the assertions do not hold
/// there; yonder had the same failure. Run it from a terminal with `--run-ignored only`
#[test]
#[ignore = "needs a terminal app as the responsible process of the test: in a background agent session the responsible process differs, as in yonder"]
fn a_disclaimed_child_is_responsible_for_itself_and_a_plain_one_is_not() {
  let plain = Sleeper::start(false);
  let disclaimed = Sleeper::start(true);
  assert_eq!(responsible_for(disclaimed.0.id()), disclaimed.pid());
  assert_eq!(responsible_for(plain.0.id()), responsible_for(std::process::id()));
  assert_ne!(responsible_for(plain.0.id()), plain.pid());
}

#[test]
fn a_disclaimed_child_gets_its_arguments_and_its_pipes() {
  let mut command = Command::new("/bin/echo");
  command.args(["one", "two words"]).stdout(Stdio::piped());
  disclaim(&mut command).expect("disclaimed");
  let output = command.output().expect("an echo");
  assert_eq!(String::from_utf8_lossy(&output.stdout), "one two words\n");
  assert!(output.status.success());
}

#[test]
fn a_missing_program_fails_the_spawn() {
  let mut command = Command::new("/nonexistent/program");
  disclaim(&mut command).expect("disclaimed");
  let error = command.spawn().expect_err("missing");
  assert_eq!(error.kind(), io::ErrorKind::NotFound, "{error}");
}

#[test]
fn a_command_that_changes_the_environment_is_refused() {
  let mut command = Command::new("/bin/echo");
  command.env("BD_HOST_TEST", "1");
  assert_eq!(disclaim(&mut command), Err("the command changes the environment".to_owned()));
}

#[test]
fn a_program_without_a_path_is_refused() {
  assert_eq!(disclaim(&mut Command::new("nc")), Err("nc has no path".to_owned()));
}

#[test]
fn libsystem_has_the_disclaim() {
  assert_eq!(unavailable(), None);
}

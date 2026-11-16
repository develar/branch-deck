//! A child process of a test that ends with the test however the test ends

/// A child process killed and waited for when dropped, also when an assertion fails before the end of the test: a
/// child that outlived its test ran on for minutes and held its directories (`brd lease return` refused the lease:
/// `slot_busy`, a `sleep 120` of a failed test)
pub struct Killed(pub std::process::Child);

impl std::ops::Deref for Killed {
  type Target = std::process::Child;

  fn deref(&self) -> &std::process::Child {
    &self.0
  }
}

impl std::ops::DerefMut for Killed {
  fn deref_mut(&mut self) -> &mut std::process::Child {
    &mut self.0
  }
}

impl Drop for Killed {
  fn drop(&mut self) {
    let _ = self.0.kill();
    let _ = self.0.wait();
  }
}

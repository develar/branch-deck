use crate::environment::ToolEnvironment;
use test_utils::fake_cli_tools::FakeCliTools;

/// The fakes of the CLIs in a temp dir and the environment that finds only them
pub(crate) fn fakes() -> (tempfile::TempDir, FakeCliTools, ToolEnvironment) {
  let root = tempfile::tempdir().unwrap();
  let fakes = FakeCliTools::new(root.path());
  let environment = ToolEnvironment::isolated(&fakes.bin(), &fakes.home());
  (root, fakes, environment)
}

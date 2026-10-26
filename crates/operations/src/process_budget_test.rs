use crate::witness::read_refs;
use test_utils::git_test_utils::TestRepo;
use test_utils::process_budget::{assert_processes, counting};

/// The witness of a plan reads the tips of every ref it names with one `for-each-ref`, under the lock too
#[test]
fn a_witness_reads_its_refs_with_one_process() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test\n");
  let git = counting();
  let refs: Vec<String> = ["auth", "login", "ui"]
    .iter()
    .flat_map(|name| [format!("refs/heads/user/virtual/{name}"), format!("refs/remotes/origin/user/virtual/{name}")])
    .collect();
  read_refs(&git, repo.path_str(), &refs).unwrap();
  assert_processes(&git, &[("for-each-ref", 1)]);
}

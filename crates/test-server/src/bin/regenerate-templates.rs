use test_server::get_test_repos_dir;
use test_utils::repo_template::templates;

fn main() -> anyhow::Result<()> {
  println!("Regenerating test repository templates...");

  let test_repos_dir = get_test_repos_dir();

  // Remove existing templates
  if test_repos_dir.exists() {
    std::fs::remove_dir_all(&test_repos_dir)?;
  }

  // Create directory
  std::fs::create_dir_all(&test_repos_dir)?;

  // Create templates
  let templates_to_create = vec![
    ("simple", templates::simple()),
    ("unassigned", templates::unassigned()),
    ("conflict_unassigned", templates::conflict_unassigned()),
    ("conflict_branches", templates::conflict_branches()),
    ("issue_links", templates::issue_links()),
    ("long_subjects", templates::long_subjects()),
  ];

  for (name, template) in templates_to_create {
    let repo_path = test_repos_dir.join(name);
    println!("Creating template: {name}");
    template.build(&repo_path)?;
  }
  {
    let name = "archived_branches";
    let repo_path = test_repos_dir.join(name);
    println!("Creating template: {name}");
    templates::archived_branches().build(&repo_path)?;
  }

  println!("Test repository templates regenerated successfully!");
  Ok(())
}

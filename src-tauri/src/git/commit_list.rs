use git2::Error;

pub(crate) fn get_commit_list<'a>(repo: &'a git2::Repository, main_branch_name: &'a str) -> Result<Vec<git2::Commit<'a>>, Error> {
  let mut rev_walk = repo.revwalk()?;

  // push HEAD to start walking from
  let head = repo.head()?;
  if let Some(head_oid) = head.target() {
    rev_walk.push(head_oid)?;
  }

  // try to find the remote branch first, then fall back to local
  let main_branch_oid = repo
    .revparse_single(&format!("origin/{main_branch_name}"))
    .or_else(|_| repo.revparse_single(main_branch_name))?;

  // hide commits that are reachable from the main branch
  rev_walk.hide(main_branch_oid.id())?;

  let mut commits = Vec::new();
  for oid in rev_walk {
    commits.push(repo.find_commit(oid?)?);
  }
  commits.reverse();

  Ok(commits)
}

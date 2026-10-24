use crate::branch_prefixes::BranchPrefixes;
use crate::resolve_test::{git, resolved};
use domain_types::settings::{AppSettings, GitBranchPrefix, PrefixSource};
use pretty_assertions::assert_eq;
use std::cell::Cell;

fn settings(app_prefix: Option<&str>) -> AppSettings {
  AppSettings {
    global_user_branch_prefix: app_prefix.map(str::to_owned),
    ..AppSettings::default()
  }
}

#[test]
fn git_is_read_once_per_repository_and_the_app_prefix_of_each_command_applies() {
  let prefixes = BranchPrefixes::default();
  let reads = Cell::new(0);
  let read = |prefix: &'static str| {
    let reads = &reads;
    move || {
      reads.set(reads.get() + 1);
      Ok::<_, ()>(git(prefix, false))
    }
  };

  assert_eq!(prefixes.resolve(&settings(None), "/a", read("global")), Ok(resolved("global", PrefixSource::GlobalConfig)));
  // the memo answers: the read is not called again, and a changed app prefix applies at once
  assert_eq!(
    prefixes.resolve(&settings(Some("jane/")), "/a", read("other")),
    Ok(resolved("jane", PrefixSource::AppSetting))
  );
  assert_eq!(reads.get(), 1);
  // another repository is read for itself
  assert_eq!(prefixes.resolve(&settings(None), "/b", read("b")), Ok(resolved("b", PrefixSource::GlobalConfig)));
  assert_eq!(reads.get(), 2);
}

#[test]
fn a_failed_read_keeps_nothing() {
  let prefixes = BranchPrefixes::default();
  assert_eq!(
    prefixes.resolve(&settings(Some("jane")), "/gone", || Err("Repository not accessible")),
    Err("Repository not accessible")
  );
  assert_eq!(
    prefixes.resolve(&settings(Some("jane")), "/gone", || Ok::<_, &str>(git("own", true))),
    Ok(resolved("own", PrefixSource::RepositoryConfig))
  );
}

#[test]
fn a_refresh_replaces_the_git_prefix_and_a_clear_forgets_every_repository() {
  let prefixes = BranchPrefixes::default();
  let unread = || -> Result<GitBranchPrefix, ()> { panic!("the memo answers") };
  assert_eq!(
    prefixes.refresh(&settings(Some("jane")), "/a", git("own", true)),
    resolved("own", PrefixSource::RepositoryConfig)
  );
  assert_eq!(prefixes.resolve(&settings(Some("jane")), "/a", unread), Ok(resolved("own", PrefixSource::RepositoryConfig)));
  // the repository's own prefix is unset: the app's applies
  prefixes.refresh(&settings(None), "/a", git("", false));
  assert_eq!(prefixes.resolve(&settings(Some("jane")), "/a", unread), Ok(resolved("jane", PrefixSource::AppSetting)));

  prefixes.clear();
  assert_eq!(
    prefixes.resolve(&settings(None), "/a", || Ok::<_, ()>(git("global", false))),
    Ok(resolved("global", PrefixSource::GlobalConfig))
  );
}

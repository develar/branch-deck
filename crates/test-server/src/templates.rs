//! The repository templates a test copies (`create_repository`). The E2E server keeps them in a cache beside its
//! executable, keyed by the executable and the version of git that built them: a run of the same build copies them at
//! once, and a rebuilt server builds them anew. It serves before they are built: the missing ones build in the
//! background, all at once, and a repository waits only for its own template. A route test builds a template into its
//! temporary directory when it first makes a repository of it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use sync_utils::fs_retry;
use sync_utils::span::in_current_span;
use test_utils::repo_template::templates;
use tokio::sync::OnceCell;

/// How a template is built
type TemplateBuild = fn(&Path) -> anyhow::Result<()>;

/// Every template, by its name (`RepositoryTemplate::as_str`)
const TEMPLATES: &[(&str, TemplateBuild)] = &[
  ("simple", |path| templates::simple().build(path)),
  ("simple_no_prefix", |path| templates::simple_no_prefix().build(path)),
  ("unassigned", |path| templates::unassigned().build(path)),
  ("conflict_unassigned", |path| templates::conflict_unassigned().build(path)),
  ("conflict_branches", |path| templates::conflict_branches().build(path)),
  ("single_unassigned", |path| templates::single_unassigned().build(path)),
  ("issue_links", |path| templates::issue_links().build(path)),
  ("issue_split", |path| templates::issue_split().build(path)),
  ("issue_groups", |path| templates::issue_groups().build(path)),
  ("issue_combine", |path| templates::issue_combine().build(path)),
  ("fixups", |path| templates::fixups().build(path)),
  ("long_subjects", |path| templates::long_subjects().build(path)),
  ("commit_tree", |path| templates::commit_tree().build(path)),
  ("commit_deleted", |path| templates::commit_deleted().build(path)),
  ("commit_many_files", |path| templates::commit_many_files().build(path)),
  ("commit_whitespace", |path| templates::commit_whitespace().build(path)),
  ("symbol_navigation", |path| templates::symbol_navigation().build(path)),
  ("script_colors", |path| templates::script_colors().build(path)),
  ("stacked_branches", |path| templates::stacked_branches().build(path)),
  ("stack_island", |path| templates::stack_island().build(path)),
  ("readme_showcase", |path| templates::readme_showcase().build(path)),
  ("branch_table_large", |path| templates::branch_table_large().build(path)),
  ("stack_side_conflict", |path| templates::stack_side_conflict().build(path)),
  ("stacked_issues", |path| templates::stacked_issues().build(path)),
  ("stacked_conflict", |path| templates::stacked_conflict().build(path)),
  ("conflict_fix", |path| templates::conflict_fix().build(path)),
  ("conflict_words", |path| templates::conflict_words().build(path)),
  ("resolved_words", |path| templates::resolved_words().build(path)),
  ("stacked_ownership", |path| templates::stacked_ownership().build(path)),
  ("stack_push_alone", |path| templates::stack_push_alone().build(path)),
  ("stack_uses_parent", |path| templates::stack_uses_parent().build(path)),
  ("landing_needs", |path| templates::landing_needs().build(path)),
  ("stack_problems", |path| templates::stack_problems().build(path)),
  ("branch_filter", |path| templates::branch_filter().build(path)),
  ("empty-non-git", |path| templates::empty_non_git().build(path)),
  ("empty_git", |path| templates::empty_git().build(path)),
  ("bare", |path| templates::bare().build(path)),
  ("archived_branches", |path| templates::archived_branches().build(path)),
  ("behind_upstream", |path| templates::behind_upstream().build(path)),
  ("update_conflict", |path| templates::update_conflict().build(path)),
  ("update_words", |path| templates::update_words().build(path)),
  ("update_conflict_wide", |path| templates::update_conflict_wide().build(path)),
  ("update_blocked", |path| templates::update_blocked().build(path)),
  ("landed_upstream", |path| templates::landed_upstream().build(path)),
  ("amend_changes", |path| templates::amend_changes().build(path)),
];

/// The directory beside the executable that holds the caches of the templates, one per build
const CACHE_DIR: &str = "e2e-templates";

/// The caches of other builds that stay beside the current one: a server of the build before may still run
const KEPT_CACHES: usize = 2;

/// A half-built template older than this is left by a server that stopped while it built
const ABANDONED_BUILD: Duration = Duration::from_mins(10);

/// The templates of a server and the builds of the ones that are missing
pub struct Templates {
  dir: PathBuf,
  /// How each template is built and its path once built; a failed build leaves its cell empty, so the next ask tries
  /// again
  built: HashMap<&'static str, (TemplateBuild, OnceCell<PathBuf>)>,
}

impl Templates {
  /// The templates of a route test: built in `dir`, its temporary directory, when first asked for
  pub fn on_demand(dir: PathBuf) -> Self {
    Self {
      dir,
      built: TEMPLATES.iter().map(|&(name, build)| (name, (build, OnceCell::new()))).collect(),
    }
  }

  /// The templates of the E2E server: the cache of this executable built by git `git_version`, in `e2e-templates/`
  /// beside the executable; the caches of older builds but the newest `KEPT_CACHES` are removed. In `fallback`, a
  /// temporary directory, when the executable or its directory can't hold them
  pub fn cached(git_version: &str, fallback: PathBuf) -> Self {
    match cache_dir(git_version) {
      Ok(dir) => {
        tracing::info!(dir = %dir.display(), "the templates of this build");
        Self::on_demand(dir)
      }
      Err(e) => {
        tracing::warn!(error = %e, "no cache of the templates beside the executable: built in the temporary directory");
        Self::on_demand(fallback)
      }
    }
  }

  /// Builds every template that is not there yet, all at once, in the background: the server serves meanwhile, and a
  /// repository waits only for its own template
  pub fn build_all_in_background(self: &Arc<Self>) {
    for name in self.names() {
      let templates = Arc::clone(self);
      tokio::spawn(async move {
        if let Err(e) = templates.path(name).await {
          tracing::error!("Failed to create the template {name}: {e:#}");
        }
      });
    }
  }

  /// The names of every template
  pub fn names(&self) -> impl Iterator<Item = &'static str> {
    TEMPLATES.iter().map(|&(name, _)| name)
  }

  /// The template `name`, built now when it is missing, or once the build in progress ends
  pub async fn path(&self, name: &str) -> anyhow::Result<PathBuf> {
    let (&name, &(build, ref cell)) = self.built.get_key_value(name).ok_or_else(|| anyhow::anyhow!("no template {name}"))?;
    let dir = self.dir.clone();
    let path = cell
      .get_or_try_init(|| async move { tokio::task::spawn_blocking(in_current_span(move || build_template(&dir, name, build))).await? })
      .await?;
    Ok(path.clone())
  }
}

/// The template `name` in `dir`, built by `build` when it is not there: into a directory of this process first, renamed
/// into place once whole, so a server never copies a half-built template, its own or another one's
fn build_template(dir: &Path, name: &str, build: TemplateBuild) -> anyhow::Result<PathBuf> {
  let target = dir.join(name);
  if target.exists() {
    return Ok(target);
  }
  let building = dir.join(format!(".{name}.{}.building", std::process::id()));
  let _ = fs_retry::remove_dir_all(&building);
  let started = Instant::now();
  let built = build(&building).and_then(|()| match fs_retry::rename(&building, &target) {
    Ok(()) => Ok(()),
    // another server of this build put its copy in place first
    Err(_) if target.exists() => Ok(fs_retry::remove_dir_all(&building)?),
    Err(e) => Err(e.into()),
  });
  if built.is_err() {
    let _ = fs_retry::remove_dir_all(&building);
  }
  built?;
  tracing::info!(elapsed_ms = started.elapsed().as_millis(), "built the template {name}");
  Ok(target)
}

/// The cache of the templates of this executable and `git_version`, created if missing; the caches of older builds but
/// the newest `KEPT_CACHES`, and half-built templates of a stopped server, are removed
fn cache_dir(git_version: &str) -> std::io::Result<PathBuf> {
  let exe = std::env::current_exe()?;
  let root = exe.parent().ok_or_else(|| std::io::Error::other("the executable has no directory"))?.join(CACHE_DIR);
  let dir = root.join(cache_key(&exe, git_version)?);
  std::fs::create_dir_all(&dir)?;
  remove_old_caches(&root, &dir);
  remove_abandoned_builds(&dir);
  Ok(dir)
}

/// The size and the modification time of the executable, which change with every build of it (a template is built by
/// code of the executable), and the version of git
fn cache_key(exe: &Path, git_version: &str) -> std::io::Result<String> {
  let metadata = exe.metadata()?;
  let modified = metadata.modified()?.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos();
  let git: String = git_version.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '.').collect();
  Ok(format!("{:x}-{modified:x}-git-{git}", metadata.len()))
}

fn remove_old_caches(root: &Path, current: &Path) {
  let Ok(entries) = std::fs::read_dir(root) else {
    return;
  };
  let mut others: Vec<(SystemTime, PathBuf)> = entries
    .flatten()
    .map(|entry| entry.path())
    .filter(|path| path != current && path.is_dir())
    .map(|path| (path.metadata().and_then(|it| it.modified()).unwrap_or(SystemTime::UNIX_EPOCH), path))
    .collect();
  others.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
  for (_, path) in others.into_iter().skip(KEPT_CACHES) {
    if let Err(e) = fs_retry::remove_dir_all(&path) {
      tracing::warn!(error = %e, path = %path.display(), "cannot remove the templates of an older build");
    }
  }
}

fn remove_abandoned_builds(dir: &Path) {
  let Ok(entries) = std::fs::read_dir(dir) else {
    return;
  };
  for path in entries.flatten().map(|entry| entry.path()) {
    let building = path.file_name().is_some_and(|name| name.to_string_lossy().ends_with(".building"));
    let age = path.metadata().and_then(|it| it.modified()).ok().and_then(|modified| modified.elapsed().ok());
    if building && age.is_some_and(|age| age > ABANDONED_BUILD) {
      let _ = fs_retry::remove_dir_all(&path);
    }
  }
}

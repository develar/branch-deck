//! The commits of a pull request to review, without the working tree, the index or the repository lock: nothing when
//! both commits are local, else one fetch of its head (and of its base when that is missing) into
//! `refs/branch-deck/review/<n>/{head,base}`; then the merge base and the commits of the range. A stack of pull requests
//! is the same fetch of the head of its top with the base of its bottom, its range cut into the layers at their heads;
//! only a layer whose head is not on it (restacked) is a fetch of its own, into
//! `refs/branch-deck/review/<top>/layers/<n>/{head,base}`. Pruning deletes those refs in one CAS transaction. See
//! "Review fetch" in `docs/design/git-operations.md`.

use crate::github_repository::{GitHubRepository, GitHubRepositorySource, parse_github_repository, resolve_github_repository_source};
use crate::remote_command::execute_remote_command_with_progress_until;
use crate::transfer_progress::TransferProgress;
use anyhow::{Result, anyhow, bail};
use domain_types::review_request::{FetchReviewHeadParams, ReviewCommit, ReviewHeadState, ReviewRange, ReviewSource};
use git_executor::git_command_executor::{GitCommandExecutor, ProgressStop, StoppedCommand};
use git_ops::cat_file_batch::object_types;
use git_ops::ref_transaction::RefTransaction;
use git_ops::revs::{is_ancestor, is_object_id, merge_base};
use std::collections::HashMap;
use std::time::Duration;
use sync_utils::commit_body::has_body;
use tracing::{instrument, warn};

/// The namespace of the review refs; only the review fetch writes it and only pruning deletes from it
pub const REVIEW_REF_PREFIX: &str = "refs/branch-deck/review/";

/// A fetch that prints nothing for this long is killed: an HTTP transfer aborts by itself (`http.lowSpeedLimit`), SSH
/// does not
pub const REVIEW_FETCH_IDLE_TIMEOUT: Duration = Duration::from_secs(120);

/// The most commits listed for a range; GitHub lists 250
const MAX_COMMITS: &str = "--max-count=1000";

/// The head of the pull request and its base branch have no merge base
#[derive(Debug)]
pub struct NoCommonHistory {
  pub base: String,
}

impl std::fmt::Display for NoCommonHistory {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "The pull request has no common history with {}", self.base)
  }
}

impl std::error::Error for NoCommonHistory {}

/// GitHub refused the fetch for missing credentials: the user's git has no credential helper for github.com
#[derive(Debug)]
pub struct GitHubCredentials;

impl std::fmt::Display for GitHubCredentials {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("GitHub asked for credentials: store HTTPS credentials of github.com in git (gh auth setup-git does)")
  }
}

impl std::error::Error for GitHubCredentials {}

/// The review ref of pull request `number`: `head` or `base`
pub fn review_ref(number: u32, side: &str) -> String {
  format!("{REVIEW_REF_PREFIX}{number}/{side}")
}

/// The review ref of the restacked layer `number` of the stack of `top`: `head` or `base`, kept and pruned with the
/// refs of `top`
pub fn stack_layer_ref(top: u32, number: u32, side: &str) -> String {
  format!("{REVIEW_REF_PREFIX}{top}/layers/{number}/{side}")
}

/// Where a review fetch writes what it fetched: the head and the base branch
struct ReviewRefs {
  head: String,
  base: String,
}

impl ReviewRefs {
  fn of(number: u32) -> Self {
    Self {
      head: review_ref(number, "head"),
      base: review_ref(number, "base"),
    }
  }
}

/// The head (and, if missing, the base) of a pull request, fetched unless both commits are local, and the range to
/// review. `is_cancelled` kills the fetch, and a fetch that reports nothing for `REVIEW_FETCH_IDLE_TIMEOUT` is killed;
/// the error is then a `StoppedCommand`.
#[instrument(skip(git, params, on_progress, is_cancelled), fields(repository_path = %params.repository_path, number = params.number))]
pub fn fetch_review_head(
  git: &GitCommandExecutor,
  params: &FetchReviewHeadParams,
  on_progress: &mut dyn FnMut(TransferProgress),
  is_cancelled: &dyn Fn() -> bool,
) -> Result<ReviewRange> {
  fetch_review_head_until(
    git,
    params,
    on_progress,
    &ProgressStop {
      idle_timeout: Some(REVIEW_FETCH_IDLE_TIMEOUT),
      is_cancelled,
    },
  )
}

/// `fetch_review_head` with the stop conditions given
pub fn fetch_review_head_until(
  git: &GitCommandExecutor,
  params: &FetchReviewHeadParams,
  on_progress: &mut dyn FnMut(TransferProgress),
  stop: &ProgressStop<'_>,
) -> Result<ReviewRange> {
  fetch_into(git, params, &ReviewRefs::of(params.number), on_progress, stop)
}

/// `fetch_review_head_until` into `refs`
fn fetch_into(
  git: &GitCommandExecutor,
  params: &FetchReviewHeadParams,
  refs: &ReviewRefs,
  on_progress: &mut dyn FnMut(TransferProgress),
  stop: &ProgressStop<'_>,
) -> Result<ReviewRange> {
  // they come from the frontend: an id must never read as an option or a revision expression, a branch name never
  // as another refspec
  for id in [&params.head_ref_oid, &params.base_ref_oid] {
    if !is_object_id(id) {
      bail!("Not a commit id: {id}");
    }
  }
  for name in [&params.head_ref_name, &params.base_ref_name] {
    if !is_safe_branch_name(name) {
      bail!("Not a branch name: {name}");
    }
  }
  let repo = params.repository_path.as_str();
  let base_tracking_ref = format!("refs/remotes/origin/{}", params.base_ref_name);
  let [head_present, base_present, tracking_present, head_exists] = present_commits(git, repo, [&params.head_ref_oid, &params.base_ref_oid, &base_tracking_ref, "HEAD"])?;
  if head_present && base_present {
    return range_of(git, params, params.head_ref_oid.clone(), &params.base_ref_oid, ReviewHeadState::Same, ReviewSource::Local);
  }

  let negotiation_tip = if tracking_present {
    Some(base_tracking_ref)
  } else {
    head_exists.then(|| "HEAD".to_string())
  };
  let mut fetch = ReviewFetch {
    git,
    params,
    refs,
    on_progress,
    stop,
    negotiation_tip,
    base_tip: None,
    base_present,
  };

  let (repository, origin_kind) = resolve_github_repository_source(git, repo)?.ok_or_else(|| anyhow!("The pull requests of the branches are not on GitHub"))?;
  let remotes = remote_urls(git, repo)?;
  let origin_url = remotes.iter().find(|(name, _)| name == "origin").map(|(_, url)| url.as_str());
  let origin_is_github = origin_url.and_then(parse_github_repository).is_some_and(|it| same_repository(&it, &repository));
  let github = || FetchSource::github(&remotes, &repository);
  let pull_ref = format!("refs/pull/{}/head", params.number);

  let (tip, source) = if origin_is_github {
    (fetch.run(&FetchSource::origin(), &pull_ref)?, ReviewSource::Origin)
  } else if origin_url.is_some() && !params.is_cross_repository && origin_kind != GitHubRepositorySource::Origin {
    // the mirror case: origin has the branch, GitHub has it after the mirror pushed it
    let branch_ref = format!("refs/heads/{}", params.head_ref_name);
    match fetch.run(&FetchSource::origin(), &branch_ref) {
      Ok(tip) if tip == params.head_ref_oid || present_commits(git, repo, [&params.head_ref_oid])? == [true] => (tip, ReviewSource::Origin),
      // the mirror is behind or force-pushed since: GitHub has the commit; without it, the review shows origin's
      Ok(tip) => match fetch.run(&github(), &pull_ref) {
        Ok(github_tip) => (github_tip, ReviewSource::GitHub),
        Err(e) if e.is::<StoppedCommand>() => return Err(e),
        Err(e) => {
          warn!(error = %format!("{e:#}"), "cannot fetch the pull request from GitHub, the review shows origin's branch");
          (tip, ReviewSource::Origin)
        }
      },
      Err(e) if e.is::<StoppedCommand>() => return Err(e),
      Err(e) => {
        warn!(error = %format!("{e:#}"), "cannot fetch the branch of the pull request from origin, trying GitHub");
        (fetch.run(&github(), &pull_ref)?, ReviewSource::GitHub)
      }
    }
  } else {
    (fetch.run(&github(), &pull_ref)?, ReviewSource::GitHub)
  };

  let base = if fetch.base_present || present_commits(git, repo, [&params.base_ref_oid])? == [true] {
    params.base_ref_oid.clone()
  } else {
    fetch.base_tip.clone().ok_or_else(|| anyhow!("Cannot fetch the base branch {}", params.base_ref_name))?
  };
  let (head, head_state) = classify_head(git, repo, &params.head_ref_oid, tip)?;
  range_of(git, params, head, &base, head_state, source)
}

/// The ranges of a stack of pull requests: of the whole stack and of each layer, in the order of the layers
#[derive(Debug)]
pub struct StackRanges {
  pub range: ReviewRange,
  pub layers: Vec<LayerRange>,
}

/// The range of a layer of a stack, and whether it was a fetch of its own
#[derive(Debug)]
pub struct LayerRange {
  pub range: ReviewRange,
  pub restacked: bool,
}

/// The commits of a stack of pull requests, `layers` the bottom first, each into the head branch of the one below: one
/// `fetch_review_head` of the head of the top with the base of the bottom (nothing when both are local), whose range
/// holds every layer that is built on the one below, cut at their heads; a layer that is not (its head is not on the
/// range, or the head below is not under it: restacked) is a `fetch_review_head` of its own, from its own base, into
/// `refs/branch-deck/review/<top>/layers/<n>/`. Stopped as `fetch_review_head`.
///
/// # Errors
/// No layers, a fetch that failed or was stopped
#[instrument(skip_all, fields(layers = layers.len()))]
pub fn fetch_review_stack(
  git: &GitCommandExecutor,
  layers: &[FetchReviewHeadParams],
  on_progress: &mut dyn FnMut(TransferProgress),
  is_cancelled: &dyn Fn() -> bool,
) -> Result<StackRanges> {
  let stop = ProgressStop {
    idle_timeout: Some(REVIEW_FETCH_IDLE_TIMEOUT),
    is_cancelled,
  };
  let (Some(bottom), Some(top)) = (layers.first(), layers.last()) else {
    bail!("A stack has no layers");
  };
  let whole = FetchReviewHeadParams {
    base_ref_name: bottom.base_ref_name.clone(),
    base_ref_oid: bottom.base_ref_oid.clone(),
    ..top.clone()
  };
  let range = fetch_into(git, &whole, &ReviewRefs::of(top.number), on_progress, &stop)?;
  // the end of each layer in the commits of the stack, oldest first: after its head
  let ends: HashMap<&str, usize> = range.commits.iter().enumerate().map(|(index, commit)| (commit.id.as_str(), index + 1)).collect();
  let mut cut = Vec::with_capacity(layers.len());
  // where the layer above starts: after the head of this one, while it is on the range
  let mut start = Some(0);
  for (index, layer) in layers.iter().enumerate() {
    let is_top = index + 1 == layers.len();
    let end = if is_top {
      Some(range.commits.len())
    } else {
      ends.get(layer.head_ref_oid.as_str()).copied()
    };
    let own = match (start, end) {
      (Some(from), Some(to)) if from <= to => Some(ReviewRange {
        base: if from == 0 { range.base.clone() } else { range.commits[from - 1].id.clone() },
        head: if is_top { range.head.clone() } else { layer.head_ref_oid.clone() },
        head_state: if is_top { range.head_state } else { ReviewHeadState::Same },
        source: range.source,
        commits: range.commits[from..to].to_vec(),
      }),
      _ => None,
    };
    cut.push(if let Some(range) = own {
      LayerRange { range, restacked: false }
    } else {
      let refs = ReviewRefs {
        head: stack_layer_ref(top.number, layer.number, "head"),
        base: stack_layer_ref(top.number, layer.number, "base"),
      };
      LayerRange {
        range: fetch_into(git, layer, &refs, on_progress, &stop)?,
        restacked: true,
      }
    });
    start = end;
  }
  Ok(StackRanges { range, layers: cut })
}

/// A remote and whether it is GitHub, for the error of a refused fetch
struct FetchSource {
  /// A remote name or a URL
  remote: String,
  is_github: bool,
}

impl FetchSource {
  fn origin() -> Self {
    Self {
      remote: "origin".to_string(),
      is_github: false,
    }
  }

  /// A configured remote whose URL is `repository` (its credentials and `insteadOf` apply), else its HTTPS URL
  fn github(remotes: &[(String, String)], repository: &GitHubRepository) -> Self {
    let remote = remotes
      .iter()
      .find(|(_, url)| parse_github_repository(url).is_some_and(|it| same_repository(&it, repository)))
      .map(|(name, _)| name.clone());
    Self {
      remote: remote.unwrap_or_else(|| format!("https://github.com/{}/{}.git", repository.owner, repository.name)),
      is_github: true,
    }
  }
}

/// The remotes with their URLs as configured, from one `git config`: `insteadOf` is not applied, so a URL rewritten
/// to a proxy or a local mirror still names the repository it stands for
fn remote_urls(git: &GitCommandExecutor, repo: &str) -> Result<Vec<(String, String)>> {
  let (urls, exit_code) = git.execute_command_with_status(&["config", "--get-regexp", r"^remote\..*\.url$"], repo)?;
  // 1: no remote has a URL
  if exit_code != 0 {
    return Ok(Vec::new());
  }
  Ok(
    urls
      .lines()
      .filter_map(|line| {
        let (key, url) = line.split_once(' ')?;
        let name = key.strip_prefix("remote.")?.strip_suffix(".url")?;
        Some((name.to_string(), url.to_string()))
      })
      .collect(),
  )
}

/// The state of one review fetch: what the next `run` needs
struct ReviewFetch<'a> {
  git: &'a GitCommandExecutor,
  params: &'a FetchReviewHeadParams,
  refs: &'a ReviewRefs,
  on_progress: &'a mut dyn FnMut(TransferProgress),
  stop: &'a ProgressStop<'a>,
  negotiation_tip: Option<String>,
  /// The fetched tip of the base branch
  base_tip: Option<String>,
  base_present: bool,
}

impl ReviewFetch<'_> {
  /// Fetch `source_ref` of `source` into the head review ref, and the base branch into the base review ref while the
  /// base commit is missing; returns the fetched head tip
  fn run(&mut self, source: &FetchSource, source_ref: &str) -> Result<String> {
    let ReviewRefs { head: head_ref, base: base_ref } = self.refs;
    let negotiation_tip = self.negotiation_tip.as_ref().map(|tip| format!("--negotiation-tip={tip}"));
    let head_refspec = format!("+{source_ref}:{head_ref}");
    let base_refspec = format!("+refs/heads/{}:{base_ref}", self.params.base_ref_name);
    let mut args = vec![
      "fetch",
      "--porcelain",
      // `=` lines for refs that are up to date: the tip of an earlier review fetch
      "--verbose",
      "--no-tags",
      "--no-write-fetch-head",
      "--no-recurse-submodules",
      "--no-auto-maintenance",
      // no opportunistic update of `refs/remotes/origin/<branch>`: the push lease reads those
      "--refmap=",
      // `fetch.prune` would prune the remote-tracking refs of origin
      "--no-prune",
      "--no-show-forced-updates",
      "--no-write-commit-graph",
    ];
    args.extend(negotiation_tip.as_deref());
    args.extend(["--end-of-options", &source.remote, &head_refspec]);
    if !self.base_present {
      args.push(&base_refspec);
    }
    let output = execute_remote_command_with_progress_until(self.git, &args, &self.params.repository_path, &[], self.on_progress, self.stop)
      .map_err(|e| {
        if e.is::<StoppedCommand>() {
          return e;
        }
        let message = format!("{e:#}");
        if source.is_github && asks_for_credentials(&message) {
          return anyhow::Error::new(GitHubCredentials);
        }
        // the last line of git's stderr says why, the command line before it does not
        anyhow!("{}: {}", source.remote, message.lines().last().unwrap_or_default().trim_start_matches("Error: "))
      })?
      .into_text();
    if !self.base_present {
      self.base_tip = fetched_tip(&output, base_ref);
      self.base_present = self.base_tip.as_ref().is_some_and(|tip| *tip == self.params.base_ref_oid);
    }
    fetched_tip(&output, head_ref).ok_or_else(|| anyhow!("git fetch did not report {head_ref}"))
  }
}

/// The new id of `local_ref` in `fetch --porcelain --verbose` output: `<flag> <old> <new> <ref>`
fn fetched_tip(output: &str, local_ref: &str) -> Option<String> {
  output.lines().find_map(|line| {
    let mut fields = line.get(2..)?.split(' ');
    let (_old, new, name) = (fields.next()?, fields.next()?, fields.next()?);
    (name == local_ref && is_object_id(new)).then(|| new.to_string())
  })
}

/// git's words for a refused authentication in the C locale (`execute_remote_command` sets it): HTTPS without a
/// credential helper, or SSH without a key
fn asks_for_credentials(message: &str) -> bool {
  [
    "could not read Username",
    "could not read Password",
    "Authentication failed",
    "terminal prompts disabled",
    "Permission denied (publickey",
  ]
  .iter()
  .any(|words| message.contains(words))
}

fn same_repository(a: &GitHubRepository, b: &GitHubRepository) -> bool {
  a.owner.eq_ignore_ascii_case(&b.owner) && a.name.eq_ignore_ascii_case(&b.name)
}

/// A branch name that is a valid ref name and can't change a refspec or read as an option
fn is_safe_branch_name(name: &str) -> bool {
  !name.is_empty()
    && !name.starts_with(['-', '/', '.'])
    && !name.ends_with(['/', '.'])
    && !std::path::Path::new(name).extension().is_some_and(|extension| extension.eq_ignore_ascii_case("lock"))
    && !name.contains("..")
    && !name.contains("//")
    && !name.contains("@{")
    && !name.contains("/.")
    && name.bytes().all(|b| b > b' ' && b != 0x7f && !b"~^:?*[\\".contains(&b))
}

/// Whether each of `names` (object ids or ref names) is a local commit, from one `cat-file --batch-check`
pub fn present_commits<const N: usize>(git: &GitCommandExecutor, repo: &str, names: [&str; N]) -> Result<[bool; N]> {
  let types = object_types(git, repo, &names)?;
  Ok(std::array::from_fn(|index| types[index] == Some("commit")))
}

/// The commit the review shows and how the fetched `tip` relates to the one the pull request shows: that one when it is
/// local, else the tip
fn classify_head(git: &GitCommandExecutor, repo: &str, head_ref_oid: &str, tip: String) -> Result<(String, ReviewHeadState)> {
  if tip == head_ref_oid {
    return Ok((tip, ReviewHeadState::Same));
  }
  if present_commits(git, repo, [head_ref_oid])? != [true] {
    return Ok((tip, ReviewHeadState::Missing));
  }
  let state = if is_ancestor(git, repo, head_ref_oid, &tip)? {
    ReviewHeadState::SourceAhead
  } else {
    ReviewHeadState::Diverged
  };
  Ok((head_ref_oid.to_string(), state))
}

/// The merge base of `head` and `base_commit` and the commits between, oldest first
fn range_of(git: &GitCommandExecutor, params: &FetchReviewHeadParams, head: String, base_commit: &str, head_state: ReviewHeadState, source: ReviewSource) -> Result<ReviewRange> {
  let repo = params.repository_path.as_str();
  let Some(base) = merge_base(git, repo, &head, base_commit)? else {
    return Err(anyhow::Error::new(NoCommonHistory {
      base: params.base_ref_name.clone(),
    }));
  };
  let range = format!("{base}..{head}");
  // `-z`: a NUL ends each commit, as NULs separate its fields; the message may hold any other byte
  let log = git
    .cmd(
      &[
        "log",
        "-z",
        "--format=%H%x00%s%x00%an%x00%at%x00%ct%x00%B",
        "--reverse",
        MAX_COMMITS,
        "--end-of-options",
        &range,
      ],
      repo,
    )
    .bytes()?;
  let log = String::from_utf8_lossy(&log);
  let mut fields = log.split('\0');
  let mut commits = Vec::new();
  while let (Some(id), Some(subject), Some(author), Some(author_time), Some(committer_time), Some(message)) =
    (fields.next(), fields.next(), fields.next(), fields.next(), fields.next(), fields.next())
  {
    commits.push(ReviewCommit::from_log_fields(
      id.trim_start_matches('\n'),
      subject,
      author,
      author_time,
      committer_time,
      message,
      has_body(message),
    ));
  }
  Ok(ReviewRange {
    base,
    head,
    head_state,
    source,
    commits,
  })
}

/// The review refs of the pull requests not in `keep`, as CAS deletes: one `for-each-ref`
pub fn review_refs_to_prune(git: &GitCommandExecutor, repo: &str, keep: &[u32]) -> Result<RefTransaction> {
  let refs = git.execute_command_lines(&["for-each-ref", "--format=%(refname) %(objectname)", REVIEW_REF_PREFIX], repo)?;
  let mut transaction = RefTransaction::new();
  for line in &refs {
    let Some((name, id)) = line.split_once(' ') else { continue };
    let number = name
      .strip_prefix(REVIEW_REF_PREFIX)
      .and_then(|rest| rest.split('/').next())
      .and_then(|n| n.parse::<u32>().ok());
    if number.is_none_or(|number| !keep.contains(&number)) {
      transaction.delete(name, Some(id));
    }
  }
  Ok(transaction)
}

/// Delete the review refs of the pull requests not in `keep` with one `for-each-ref` and at most one `update-ref`. A
/// ref that moved meanwhile (a review fetch of another window) fails the transaction: that is only logged, the next
/// prune tries again
#[instrument(skip(git))]
pub fn prune_review_refs(git: &GitCommandExecutor, repo: &str, keep: &[u32]) -> Result<()> {
  let transaction = review_refs_to_prune(git, repo, keep)?;
  if let Err(e) = transaction.commit(git, repo, "prune review refs") {
    warn!(error = %format!("{e:#}"), "failed to prune the review refs");
  }
  Ok(())
}

#[cfg(test)]
#[path = "review_fetch_test.rs"]
pub(crate) mod review_fetch_test;

//! The text of a new pull request of a virtual branch, as the app's form, `bd push` and `bd safe-push
//! --create-pull-request` start it (spec/branch/pull-requests.spec.md): the `bd describe` description, else the
//! commits the pull request lands, written as a reviewer reads them on GitHub. The title keeps the issue key and takes
//! the issue's summary from YouTrack when it answers; the body links the issues, then has an item per commit with its
//! body: hard-wrapped lines joined into paragraphs (GitHub shows a newline of a pull request as a line break), the
//! trailers left out, a mention or a `<T>` kept literal. A stacked branch is titled by its own commits and names the
//! branches it is built on, whose commits the pull request holds too. No git: the callers read the commits.

use crate::stack_pull_request::{MAX_BODY_CHARS, cut_chars, cut_title, push_fitting_sections};
use domain_types::commit_body::CommitBodyBlock;
use domain_types::issue_navigation::IssueNavigationConfig;
use domain_types::youtrack::YouTrackIssueAnswer;
use git_ops::branch_description::Description;
use std::borrow::Cow;
use std::fmt::Write as _;
use std::time::Duration;
use sync_core::issue_navigation::issue_link;
use sync_core::sync::branch_summary;
use sync_utils::commit_body::body_blocks;
use sync_utils::issue_pattern::find_issue_number;
use tracing::debug;
use youtrack::service::YouTrackService;

/// The lookups of the issues of a text that take longer leave their summaries out: the text is not worth the wait
const ISSUE_LOOKUP_TIMEOUT: Duration = Duration::from_secs(3);

/// A commit of the pull request
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitText {
  /// Without the `(prefix)` of the grouping
  pub subject: String,
  /// The message after the subject
  pub body: String,
}

impl CommitText {
  /// The subject `stripped_subject` and the body of `message`, whose first line is the subject
  pub fn of_message(stripped_subject: &str, message: &str) -> Self {
    Self {
      subject: stripped_subject.trim().to_string(),
      body: message.split_once('\n').map_or("", |(_, body)| body).trim().to_string(),
    }
  }
}

/// A branch the pull request's branch is built on, whose commits it holds too
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AncestorText {
  pub name: String,
  /// Oldest first
  pub subjects: Vec<String>,
}

/// What the text is written from
#[derive(Debug, Clone)]
pub struct PullRequestTextInput<'a> {
  /// The virtual branch name: `IJAI-4390`, `auth`
  pub branch_name: &'a str,
  pub description: Option<&'a Description>,
  /// The branch's own commits, oldest first
  pub commits: Vec<CommitText>,
  /// The branches it is built on, the root first
  pub ancestors: Vec<AncestorText>,
}

/// What is known of an issue the pull request names
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueText {
  pub key: String,
  /// The summary YouTrack answered
  pub summary: Option<String>,
  /// The issue page
  pub url: Option<String>,
}

impl PullRequestTextInput<'_> {
  /// The commits the body has an item for: without `fixup!`, `squash!` and `amend!` commits, which a fold puts into
  /// others (all of them when there is nothing else)
  fn items(&self) -> Vec<&CommitText> {
    let items: Vec<&CommitText> = self.commits.iter().filter(|commit| !is_autosquash(&commit.subject)).collect();
    if items.is_empty() { self.commits.iter().collect() } else { items }
  }

  /// The issues of the pull request, each once: the branch's name when it is a key, then the keys of the subjects in
  /// their order
  pub fn issue_keys(&self) -> Vec<String> {
    let named = (find_issue_number(self.branch_name) == Some(self.branch_name)).then_some(self.branch_name);
    let mut keys: Vec<String> = Vec::new();
    for key in named.into_iter().chain(self.items().into_iter().filter_map(|commit| find_issue_number(&commit.subject))) {
      if !keys.iter().any(|known| known == key) {
        keys.push(key.to_string());
      }
    }
    keys
  }
}

fn is_autosquash(subject: &str) -> bool {
  ["fixup! ", "squash! ", "amend! "].iter().any(|marker| subject.starts_with(marker))
}

/// The issue texts of `keys`: the link of the repository's issue navigation (`.idea/vcs.xml`), and when that link is
/// an issue page of the YouTrack `youtrack` is signed in to, the summary and the page YouTrack answers. A YouTrack that
/// doesn't answer within `ISSUE_LOOKUP_TIMEOUT` for all the keys, or not at all, leaves the summary out.
pub async fn issue_texts(keys: &[String], links: Option<&IssueNavigationConfig>, youtrack: Option<&YouTrackService>) -> Vec<IssueText> {
  let deadline = tokio::time::Instant::now() + ISSUE_LOOKUP_TIMEOUT;
  let youtrack_url = match youtrack {
    Some(service) => service.status().url,
    None => None,
  };
  let mut issues = Vec::with_capacity(keys.len());
  for key in keys {
    let url = links.and_then(|links| issue_link(links, key));
    let in_youtrack = youtrack_url
      .as_deref()
      .zip(url.as_deref())
      .is_some_and(|(base, url)| url.strip_prefix(&format!("{}/issue/", base.trim_end_matches('/'))) == Some(key.as_str()));
    let found = match youtrack.filter(|_| in_youtrack) {
      Some(service) => match tokio::time::timeout_at(deadline, service.issue(key)).await {
        Ok(YouTrackIssueAnswer::Found { issue }) => Some(issue),
        Ok(answer) => {
          debug!(key, ?answer, "no issue summary for the pull request");
          None
        }
        Err(_) => {
          debug!(key, "YouTrack did not answer in time: no issue summary for the pull request");
          None
        }
      },
      None => None,
    };
    issues.push(match found {
      Some(issue) => IssueText {
        key: key.clone(),
        summary: Some(issue.summary).filter(|summary| !summary.trim().is_empty()),
        url: Some(issue.url),
      },
      None => IssueText {
        key: key.clone(),
        summary: None,
        url,
      },
    });
  }
  issues
}

/// The title and the body of the pull request of `input`; `issues` are the texts of `input.issue_keys()`
pub fn pull_request_text(input: &PullRequestTextInput<'_>, issues: &[IssueText]) -> (String, String) {
  if let Some(description) = input.description {
    return (cut_title(&description.title), cut_chars(&description.body, MAX_BODY_CHARS));
  }
  let items = input.items();
  let keys = input.issue_keys();
  // the one issue of the branch, which the title carries
  let title_key = match keys.as_slice() {
    [key] => Some(key.as_str()),
    _ => None,
  };
  let title = match items.as_slice() {
    [] => title_of_branch(input.branch_name),
    [only] => only.subject.clone(),
    _ => {
      let summary = branch_summary(items.iter().map(|commit| commit.subject.as_str()));
      match title_key {
        Some(key) => {
          let issue_summary = issues.iter().find(|issue| issue.key == key).and_then(|issue| issue.summary.as_deref());
          let summary = issue_summary.map_or(summary.as_str(), str::trim);
          if summary.is_empty() { key.to_string() } else { format!("{key} {summary}") }
        }
        None if summary.is_empty() => title_of_branch(input.branch_name),
        None => summary,
      }
    }
  };

  let mut body = issue_line(&keys, issues);
  match items.as_slice() {
    [] => {}
    [only] => {
      let text = commit_body_markdown(&only.body);
      if !text.is_empty() {
        let _ = write!(body, "\n\n{text}");
      }
    }
    many => {
      // the subjects as `push_fitting_sections` names the items left out
      let subjects: Vec<String> = many.iter().map(|commit| item_subject(&commit.subject, title_key)).collect();
      let sections = many.iter().zip(&subjects).enumerate().map(|(index, (commit, subject))| {
        let item = format!("\n\n{}", commit_item(index + 1, subject, &commit.body));
        (subject.as_str(), item)
      });
      push_fitting_sections(&mut body, sections, ("commit", "commits"));
    }
  }
  if let Some(section) = ancestors_section(&input.ancestors) {
    let section = format!("\n\n{section}");
    if body.chars().count() + section.chars().count() <= MAX_BODY_CHARS {
      body.push_str(&section);
    }
  }
  (cut_title(&title), body.trim_start_matches('\n').to_string())
}

/// `feature-auth` -> `feature auth`, as `gh pr create --fill` names a pull request; an issue key stays as it is
fn title_of_branch(branch_name: &str) -> String {
  if find_issue_number(branch_name) == Some(branch_name) {
    return branch_name.to_string();
  }
  branch_name.replace(['-', '_'], " ")
}

/// "[IJAI-4390](https://…/issue/IJAI-4390) · [IJAI-1](…)": each issue linked when its page is known
fn issue_line(keys: &[String], issues: &[IssueText]) -> String {
  keys
    .iter()
    .map(|key| match issues.iter().find(|issue| &issue.key == key).and_then(|issue| issue.url.as_deref()) {
      Some(url) => format!("[{key}]({url})"),
      None => key.clone(),
    })
    .collect::<Vec<_>>()
    .join(" · ")
}

/// The subject of an item, Markdown: without the issue key the title carries when the subject starts with it
fn item_subject(subject: &str, title_key: Option<&str>) -> String {
  let without_key = title_key
    .and_then(|key| subject.strip_prefix(key))
    .filter(|rest| rest.is_empty() || rest.starts_with([' ', ':']))
    .map(|rest| rest.trim_start_matches([' ', ':']))
    .filter(|rest| !rest.is_empty())
    .unwrap_or(subject);
  literal_markdown(without_key)
}

/// "3. **subject**" and the body under it, indented by the width of the marker, so that it stays in the item
fn commit_item(number: usize, subject: &str, body: &str) -> String {
  let marker = format!("{number}. ");
  let indent = " ".repeat(marker.len());
  let mut item = format!("{marker}**{subject}**");
  let text = commit_body_markdown(body);
  for line in text.lines() {
    item.push('\n');
    if !line.is_empty() {
      item.push_str(&indent);
      item.push_str(line);
    }
  }
  item
}

/// "Built on auth: its commits are in this pull request too." and their subjects; per branch for several
fn ancestors_section(ancestors: &[AncestorText]) -> Option<String> {
  let ancestors: Vec<&AncestorText> = ancestors.iter().filter(|ancestor| !ancestor.subjects.is_empty()).collect();
  let list = |subjects: &[String]| subjects.iter().map(|subject| format!("- {}", literal_markdown(subject))).collect::<Vec<_>>().join("\n");
  match ancestors.as_slice() {
    [] => None,
    [only] => Some(format!("Built on {}: its commits are in this pull request too.\n\n{}", only.name, list(&only.subjects))),
    many => {
      let names: Vec<&str> = many.iter().map(|ancestor| ancestor.name.as_str()).collect();
      let mut section = format!("Built on {}: their commits are in this pull request too.", names.join(", then "));
      for ancestor in many {
        let _ = write!(section, "\n\n**{}**\n{}", ancestor.name, list(&ancestor.subjects));
      }
      Some(section)
    }
  }
}

/// The body of a commit as Markdown that GitHub shows as the commit reads, from the blocks of the commit preview
/// (`sync_utils::commit_body::body_blocks`): a line per line of a paragraph (GitHub shows a newline of a pull request as
/// a line break), a list with `-` or the numbers from its first, code fenced, the blocks apart by a blank line; the
/// trailers (`Co-Authored-By:`…) left out, a mention or a `<T>` in code spans
fn commit_body_markdown(body: &str) -> String {
  let blocks: Vec<String> = body_blocks(body)
    .into_iter()
    .filter_map(|block| match block {
      CommitBodyBlock::Paragraph { lines } => Some(lines.iter().map(|line| literal_markdown(line)).collect::<Vec<_>>().join("\n")),
      CommitBodyBlock::List { ordered, start, items } => Some(
        items
          .iter()
          .zip(start..)
          .map(|(item, number)| {
            if ordered {
              format!("{number}. {}", literal_markdown(item))
            } else {
              format!("- {}", literal_markdown(item))
            }
          })
          .collect::<Vec<_>>()
          .join("\n"),
      ),
      CommitBodyBlock::Preformatted { text } => {
        // a fence the code itself has no line of
        let fence = if text.lines().any(|line| line.trim_start().starts_with("```")) { "~~~" } else { "```" };
        Some(format!("{fence}\n{text}\n{fence}"))
      }
      CommitBodyBlock::Trailers { .. } => None,
    })
    .collect();
  blocks.join("\n\n")
}

/// `text` with each word that GitHub would read as a mention (`@Nullable`) or as an HTML tag (`<T>`, `List<String>`) in a
/// code span; code spans stay as they are
fn literal_markdown(text: &str) -> String {
  let mut literal = String::with_capacity(text.len());
  let mut rest = text;
  while let Some(next) = rest.chars().next() {
    if next == '`' {
      let run = rest.len() - rest.trim_start_matches('`').len();
      let span = rest[run..].find(&rest[..run]).map_or(run, |end| run + end + run);
      literal.push_str(&rest[..span]);
      rest = &rest[span..];
    } else if next.is_whitespace() {
      literal.push(next);
      rest = &rest[next.len_utf8()..];
    } else {
      let end = rest.find(|c: char| c.is_whitespace() || c == '`').unwrap_or(rest.len());
      literal.push_str(&literal_word(&rest[..end]));
      rest = &rest[end..];
    }
  }
  literal
}

fn literal_word(word: &str) -> Cow<'_, str> {
  let start = word.len() - word.trim_start_matches(['(', '[', '"', '\'']).len();
  let end = word.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '"', '\'']).len().max(start);
  let core = &word[start..end];
  let mention = core.len() > 1 && core.starts_with('@');
  let tag = core.find('<').is_some_and(|open| core[open..].contains('>')) && !core.starts_with("<http");
  if mention || tag {
    Cow::Owned(format!("{}`{core}`{}", &word[..start], &word[end..]))
  } else {
    Cow::Borrowed(word)
  }
}

#[cfg(test)]
#[path = "pull_request_text_test.rs"]
mod pull_request_text_test;

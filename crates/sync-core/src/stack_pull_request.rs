//! The text of the pull request of the whole stack: the pull request of the stack branch `{prefix}/stack/<root>` into
//! the target, whose approvals count for the one robot that lands the whole stack (docs/design/landing.md,
//! spec/branch/whole-stack.spec.md). It is written from what the sync read: the description of the stack branch when it
//! has one, else the descriptions of its members (`bd describe`, `ConfigSnapshot::descriptions`), else the subjects of
//! their commits; no git process. GitHub refuses a body over 65,536 characters and a title over 256: both are cut.

use anyhow::Result;
use domain_types::branch_names::{to_final_branch_name, to_stack_branch_name};
use domain_types::pull_request::NewPullRequest;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::branch_description::{Description, read_descriptions};
use serde::Deserialize;
use std::fmt::Write as _;
use tracing::instrument;

/// The most characters GitHub takes in the body of a pull request
pub const MAX_BODY_CHARS: usize = 65_536;
/// The most characters GitHub takes in the title of a pull request
pub const MAX_TITLE_CHARS: usize = 256;
/// What the line about the members left out may take of the body
const OMITTED_LINE_CHARS: usize = 1_000;

/// A branch of the stack, as the text names it
#[derive(Debug, Clone)]
pub struct StackMemberText<'a> {
  pub name: &'a str,
  /// The branch it is built on; `None` for the root
  pub parent: Option<&'a str>,
  /// Its `bd describe` title and body
  pub description: Option<&'a Description>,
  /// The subjects of its commits without their prefix, oldest first
  pub subjects: Vec<&'a str>,
}

/// The title and the body of the pull request of the whole stack of `members` (the root first, each after its parent).
/// `own` is the description of the stack branch itself, which wins. Else the title is the root's ("Add auth, with the 2
/// branches built on it"), the body says what the one Safe Push lands, then a section per member: its description, or
/// the subjects of its commits. Sections that don't fit into `MAX_BODY_CHARS` are named in a last line instead.
pub fn stack_pull_request_text(own: Option<&Description>, members: &[StackMemberText<'_>], target: &str) -> (String, String) {
  if let Some(own) = own {
    return (cut_title(&own.title), cut_chars(&own.body, MAX_BODY_CHARS));
  }
  let Some(root) = members.first() else {
    return (String::new(), String::new());
  };
  let built_on = members.len().saturating_sub(1);
  let title = match built_on {
    0 => member_title(root),
    1 => format!("{}, with the branch built on it", member_title(root)),
    count => format!("{}, with the {count} branches built on it", member_title(root)),
  };

  let commits: usize = members.iter().map(|member| member.subjects.len()).sum();
  let intro = format!(
    "The whole {} stack in one Safe Push into {target}: {} · {}.",
    root.name,
    count_of(members.len(), "branch", "branches"),
    count_of(commits, "commit", "commits")
  );
  let mut body = intro;
  let sections = members.iter().map(|member| (member.name, format!("\n\n{}", member_section(member))));
  push_fitting_sections(&mut body, sections, ("branch", "branches"));
  (cut_title(&title), body)
}

/// Appends each section (its text starts with its separator) while the body fits into `MAX_BODY_CHARS`; from the first
/// that doesn't, the sections are named in a last line instead ("the text of 2 branches is left out (auth, login)").
/// `kind` names a section: one and many.
pub(crate) fn push_fitting_sections<'a>(body: &mut String, sections: impl IntoIterator<Item = (&'a str, String)>, kind: (&str, &str)) {
  let mut used = body.chars().count();
  let mut omitted: Vec<&str> = Vec::new();
  for (name, section) in sections {
    let length = section.chars().count();
    if omitted.is_empty() && used + length <= MAX_BODY_CHARS - OMITTED_LINE_CHARS {
      body.push_str(&section);
      used += length;
    } else {
      omitted.push(name);
    }
  }
  if !omitted.is_empty() {
    let line = format!(
      "\n\n_Too long for a pull request: the text of {} is left out ({})._",
      count_of(omitted.len(), kind.0, kind.1),
      omitted.join(", ")
    );
    body.push_str(&cut_chars(&line, OMITTED_LINE_CHARS));
  }
}

/// The description title, else the subject of its one commit, else the branch name, as `gh pr create --fill` names it
fn member_title(member: &StackMemberText<'_>) -> String {
  match (member.description, member.subjects.as_slice()) {
    (Some(description), _) => description.title.clone(),
    (None, [only]) => (*only).to_string(),
    (None, _) => member.name.to_string(),
  }
}

/// "## auth: Add auth", "built on auth", then the description body, else the subjects as a list
fn member_section(member: &StackMemberText<'_>) -> String {
  let heading = match member.description {
    Some(description) => format!("## {}: {}", member.name, description.title),
    None => format!("## {}", member.name),
  };
  let mut section = heading;
  if let Some(parent) = member.parent {
    let _ = write!(section, "\n\nBuilt on {parent}.");
  }
  match member.description.map(|description| description.body.as_str()).filter(|body| !body.trim().is_empty()) {
    Some(body) => {
      let _ = write!(section, "\n\n{}", body.trim_end());
    }
    None if !member.subjects.is_empty() => {
      section.push('\n');
      for subject in &member.subjects {
        let _ = write!(section, "\n- {subject}");
      }
    }
    None => {}
  }
  section
}

/// "1 branch", "7 branches"
pub(crate) fn count_of(count: usize, one: &str, many: &str) -> String {
  format!("{count} {}", if count == 1 { one } else { many })
}

/// A branch of the stack, as the app's form of the stack's pull request sends it: what its row shows
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StackMemberSubjects {
  pub name: String,
  /// The branch it is built on; null for the root
  pub parent: Option<String>,
  /// The subjects of its commits without their prefix, oldest first
  pub subjects: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetNewStackPullRequestParams {
  pub repository_path: String,
  pub branch_prefix: String,
  /// The root of the stack, which names its stack branch `{prefix}/stack/<root>`
  pub root: String,
  /// The branches of the stack, the root first, each after its parent (`StackBranchUpdate::members`)
  pub members: Vec<StackMemberSubjects>,
  /// The branch it merges into, as the remote names it: `master`
  pub target: String,
}

/// The text of the pull request of the whole stack for the app's form (`stack_pull_request_text`): the descriptions of
/// the stack branch and of its members from one `git config`, the subjects as the rows show them. The viewer is left to
/// the caller, who asks GitHub.
#[instrument(skip(git, params), fields(root = %params.root))]
pub fn read_new_stack_pull_request(git: &GitCommandExecutor, params: &GetNewStackPullRequestParams) -> Result<NewPullRequest> {
  let descriptions = read_descriptions(git, &params.repository_path)?;
  let description_of = |full_name: Result<String>| full_name.ok().and_then(|name| descriptions.get(&format!("refs/heads/{name}")));
  let own = description_of(to_stack_branch_name(&params.branch_prefix, &params.root));
  let members: Vec<StackMemberText<'_>> = params
    .members
    .iter()
    .map(|member| StackMemberText {
      name: &member.name,
      parent: member.parent.as_deref(),
      description: description_of(to_final_branch_name(&params.branch_prefix, &member.name)),
      subjects: member.subjects.iter().map(String::as_str).collect(),
    })
    .collect();
  let (title, body) = stack_pull_request_text(own, &members, &params.target);
  Ok(NewPullRequest { title, body, viewer_login: None })
}

/// One line of at most `MAX_TITLE_CHARS`
pub(crate) fn cut_title(title: &str) -> String {
  let line = title.lines().next().unwrap_or_default().trim();
  cut_chars(line, MAX_TITLE_CHARS)
}

/// `text`, or its first characters and "…" to `max` characters
pub(crate) fn cut_chars(text: &str, max: usize) -> String {
  if text.chars().count() <= max {
    return text.to_string();
  }
  let mut cut: String = text.chars().take(max.saturating_sub(1)).collect();
  cut.push('…');
  cut
}

#[cfg(test)]
#[path = "stack_pull_request_test.rs"]
mod stack_pull_request_test;

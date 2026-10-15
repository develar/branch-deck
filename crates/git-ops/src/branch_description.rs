//! The pull request text of a virtual branch, kept in git's own `branch.<name>.description` as
//! `git branch --edit-description` writes it (the title, a blank line, the body). `bd describe` writes it; `bd` and
//! the sync read it

use anyhow::{Result, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Also a value of `descriptions` in a `bd group` plan
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Description {
  pub title: String,
  /// Omitted when empty
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub body: String,
}

impl Description {
  /// The first line is the title, the text after the blank line the body
  pub fn parse(text: &str) -> Self {
    let text = text.trim();
    let (title, body) = text.split_once('\n').unwrap_or((text, ""));
    Self {
      title: title.trim().to_string(),
      body: body.trim().to_string(),
    }
  }

  /// The title and the body without the white space around them
  #[must_use]
  pub fn trimmed(&self) -> Self {
    Self {
      title: self.title.trim().to_string(),
      body: self.body.trim().to_string(),
    }
  }

  /// A pull request needs a title of one line
  pub fn has_valid_title(&self) -> bool {
    !self.title.is_empty() && !self.title.contains(['\n', '\r'])
  }

  /// The value of `branch.<name>.description`, the inverse of `parse`
  pub fn to_text(&self) -> String {
    if self.body.is_empty() {
      format!("{}\n", self.title)
    } else {
      format!("{}\n\n{}\n", self.title, self.body)
    }
  }
}

/// Full ref -> description of every branch that has one, from one `git config --get-regexp`
pub fn read_descriptions(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<HashMap<String, Description>> {
  let (output, exit_code) = git_executor.execute_command_with_status(&["config", "-z", "--get-regexp", r"^branch\..*\.description$"], repository_path)?;
  // 1: no description at all
  if exit_code == 1 {
    return Ok(HashMap::new());
  }
  if exit_code != 0 {
    bail!("Cannot read the branch descriptions: {output}");
  }
  // `branch.<name>.description` LF `<value>` NUL
  Ok(
    output
      .split('\0')
      .filter_map(|entry| {
        let (key, value) = entry.split_once('\n')?;
        let name = key.trim().strip_prefix("branch.")?.strip_suffix(".description")?;
        Some((format!("refs/heads/{name}"), Description::parse(value)))
      })
      .collect(),
  )
}

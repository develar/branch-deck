use crate::stack_pull_request::{MAX_BODY_CHARS, MAX_TITLE_CHARS, StackMemberText, stack_pull_request_text};
use git_ops::branch_description::Description;
use pretty_assertions::assert_eq;

fn description(title: &str, body: &str) -> Description {
  Description {
    title: title.to_string(),
    body: body.to_string(),
  }
}

fn member<'a>(name: &'a str, parent: Option<&'a str>, description: Option<&'a Description>, subjects: &[&'a str]) -> StackMemberText<'a> {
  StackMemberText {
    name,
    parent,
    description,
    subjects: subjects.to_vec(),
  }
}

/// The text of the pull request of the whole stack: the root's title, what the one Safe Push lands, then each member by
/// its description, else by the subjects of its commits
#[test]
fn the_pull_request_of_the_whole_stack_names_each_member_by_its_description_else_its_commits() {
  let auth = description("IJPL-1 Add auth", "Adds the auth service.\n");
  let session = description("IJPL-3 Keep the session", "");
  let members = [
    member("auth", None, Some(&auth), &["Add auth", "Fix auth"]),
    member("login", Some("auth"), None, &["Add login"]),
    member("session", Some("auth"), Some(&session), &["Add session", "Expire session"]),
  ];
  let (title, body) = stack_pull_request_text(None, &members, "master");
  assert_eq!(title, "IJPL-1 Add auth, with the 2 branches built on it");
  assert_eq!(
    body,
    "The whole auth stack in one Safe Push into master: 3 branches · 5 commits.

## auth: IJPL-1 Add auth

Adds the auth service.

## login

Built on auth.

- Add login

## session: IJPL-3 Keep the session

Built on auth.

- Add session
- Expire session"
  );

  // without a description, the title of a root of one commit is its subject, of several the branch name
  let (title, _) = stack_pull_request_text(None, &members[1..2], "master");
  assert_eq!(title, "Add login");
  let plain = [member("auth", None, None, &["Add auth", "Fix auth"]), members[1].clone()];
  assert_eq!(stack_pull_request_text(None, &plain, "master").0, "auth, with the branch built on it");

  // the description of the stack branch itself wins
  let own = description("The whole auth rework", "Everything at once.");
  assert_eq!(
    stack_pull_request_text(Some(&own), &members, "master"),
    ("The whole auth rework".to_string(), "Everything at once.".to_string())
  );
}

/// GitHub refuses a body over 65,536 characters and a title over 256: the sections that don't fit are named instead
#[test]
fn a_text_too_long_for_github_is_cut_and_names_the_members_left_out() {
  let long = "x".repeat(40_000);
  let big = description(&"t".repeat(300), &long);
  let members = [
    member("auth", None, Some(&big), &["Add auth"]),
    member("login", Some("auth"), Some(&big), &["Add login"]),
    member("session", Some("auth"), None, &["Add session"]),
  ];
  let (title, body) = stack_pull_request_text(None, &members, "master");
  assert_eq!(title.chars().count(), MAX_TITLE_CHARS);
  assert!(title.ends_with('…'), "{title}");
  assert!(body.chars().count() <= MAX_BODY_CHARS, "{}", body.chars().count());
  assert!(body.contains("## auth: "), "the root is in");
  assert!(
    body.ends_with("_Too long for a pull request: the text of 2 branches is left out (login, session)._"),
    "{}",
    &body[body.len() - 200..]
  );
}

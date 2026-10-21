use crate::pull_request_text::{AncestorText, CommitText, IssueText, PullRequestTextInput, issue_texts, pull_request_text};
use git_ops::branch_description::Description;
use github::fake_github::{FakeAnswer, start_fake_server};
use pretty_assertions::assert_eq;
use serde_json::json;
use sync_types::issue_navigation::{IssueNavigationConfig, IssueNavigationLink};
use sync_utils::secret_store::write_secret_json;
use youtrack::client::YouTrackClient;
use youtrack::service::YouTrackService;
use youtrack::token_store::{StoredToken, token_file};

fn commit(message: &str) -> CommitText {
  CommitText::of_message(message.lines().next().unwrap_or_default(), message)
}

fn input<'a>(branch_name: &'a str, messages: &[&str]) -> PullRequestTextInput<'a> {
  PullRequestTextInput {
    branch_name,
    description: None,
    commits: messages.iter().map(|message| commit(message)).collect(),
    ancestors: Vec::new(),
  }
}

fn issue(key: &str, summary: Option<&str>) -> IssueText {
  IssueText {
    key: key.to_string(),
    summary: summary.map(str::to_string),
    url: Some(format!("https://youtrack.example.com/issue/{key}")),
  }
}

/// The text of `input` with the issue texts of its keys from `known`, as `issue_texts` would give them
fn text_of(input: &PullRequestTextInput<'_>, known: &[IssueText]) -> (String, String) {
  let issues: Vec<IssueText> = input
    .issue_keys()
    .iter()
    .map(|key| {
      known.iter().find(|issue| &issue.key == key).cloned().unwrap_or_else(|| IssueText {
        key: key.clone(),
        summary: None,
        url: None,
      })
    })
    .collect();
  pull_request_text(input, &issues)
}

const TERMINAL: &str = "IJAI-4390 a terminal start can remove named variables from the parent environment

ShellStartupOptions gets removedEnvVariables. LocalOptionsConfigurer removes these names after it copies
the parent environment, and before it adds the variables of the options.

Air needs it for an agent CLI.";

const BANNER: &str = "IJAI-4390 a terminal tab shows no banner

The tab observer showed the banner on every tab.";

#[test]
fn an_issue_branch_keeps_its_key_and_takes_the_issue_summary() {
  let branch = input("IJAI-4390", &[TERMINAL, BANNER]);
  let (title, body) = text_of(&branch, &[issue("IJAI-4390", Some("Agent CLIs render without colors"))]);
  assert_eq!(title, "IJAI-4390 Agent CLIs render without colors");
  assert_eq!(
    body,
    "[IJAI-4390](https://youtrack.example.com/issue/IJAI-4390)

1. **a terminal start can remove named variables from the parent environment**
   ShellStartupOptions gets removedEnvVariables. LocalOptionsConfigurer removes these names after it copies the parent environment, and before it adds the variables of the options.

   Air needs it for an agent CLI.

2. **a terminal tab shows no banner**
   The tab observer showed the banner on every tab."
  );
}

#[test]
fn the_branch_summary_keeps_what_comes_before_the_key() {
  // `(IJPL-1) test: add the UI test` is pushed as `test: IJPL-1 add the UI test`: the summary keeps its type
  let branch = input("IJPL-1", &["[plugins] IJPL-1 refactor the descriptor", "test: IJPL-1 add the UI test"]);
  assert_eq!(text_of(&branch, &[]).0, "IJPL-1 test: add the UI test");
  // a refactoring after a category is still a refactoring
  assert_eq!(
    text_of(&input("IJPL-1", &["[plugins] IJPL-1 refactor the descriptor"]), &[]).0,
    "[plugins] IJPL-1 refactor the descriptor"
  );
  assert_eq!(
    text_of(&input("IJPL-1", &["[plugins] IJPL-1 refactor the descriptor", "[plugins] IJPL-1 format"]), &[]).0,
    "IJPL-1 [plugins] refactor the descriptor"
  );
}

#[test]
fn without_an_issue_summary_the_title_is_the_key_and_the_branch_summary() {
  let cleanup_first = "IJAI-4390 cleanup the terminal options";
  let branch = input("IJAI-4390", &[cleanup_first, TERMINAL]);
  let (title, body) = text_of(&branch, &[]);
  // the summary of the row: the first commit that is no cleanup, without its key
  assert_eq!(title, "IJAI-4390 a terminal start can remove named variables from the parent environment");
  assert!(body.starts_with("IJAI-4390\n\n1. **cleanup the terminal options**\n\n2. **a terminal start"), "{body}");

  // one commit: its subject, and its body without a list
  let (title, body) = text_of(&input("IJAI-4390", &[BANNER]), &[issue("IJAI-4390", Some("ignored for one commit"))]);
  assert_eq!(title, "IJAI-4390 a terminal tab shows no banner");
  assert_eq!(
    body,
    "[IJAI-4390](https://youtrack.example.com/issue/IJAI-4390)\n\nThe tab observer showed the banner on every tab."
  );

  // a branch name that is no key: as `gh pr create --fill` names it, and the summary when there is one
  assert_eq!(text_of(&input("feature-auth", &[]), &[]).0, "feature auth");
  assert_eq!(text_of(&input("IJAI-4390", &[]), &[]).0, "IJAI-4390");
  assert_eq!(
    text_of(&input("auth", &["Add login", "Add logout"]), &[]),
    ("Add login".to_string(), "1. **Add login**\n\n2. **Add logout**".to_string())
  );
}

#[test]
fn a_branch_of_several_keys_names_each_key_and_is_titled_by_its_summary() {
  let branch = input("pair", &["IJPL-1 Add the pair view", "IJPL-2 Fix the pair order", "Polish the pair view"]);
  let (title, body) = text_of(&branch, &[issue("IJPL-1", Some("Pair view"))]);
  assert_eq!(title, "Add the pair view");
  // each subject keeps its key: the title carries none
  assert_eq!(
    body,
    "[IJPL-1](https://youtrack.example.com/issue/IJPL-1) · IJPL-2

1. **IJPL-1 Add the pair view**

2. **IJPL-2 Fix the pair order**

3. **Polish the pair view**"
  );
}

#[test]
fn each_commit_is_an_item_with_its_body_reflowed() {
  // hard-wrapped: the lines of a paragraph and of a list item are joined
  let wrapped = "The job state decides first. For the newest job whose state names no
finished job, it decides at once:
- the saved --settings names an existing file, or the planned resume has no flags: one bare
  `--bg --resume <id>`. The session keeps its options;";
  let joined = "The job state decides first. For the newest job whose state names no finished job, it decides at once:
- the saved --settings names an existing file, or the planned resume has no flags: one bare `--bg --resume <id>`. The session keeps its options;";
  // each of these lines stays as it is
  let kept = "- else: `claude rm` first.
1) a list of another marker
2) continued
## A heading
| a | table |
| - | ----- |
> a quote
> of two lines
---
A line with a hard break\x20\x20
after it, and another\\
after it.

    indented code
    stays as it is

```
fenced code
  stays too
```
The end.";
  let mut commits: Vec<String> = (1..=9).map(|n| format!("Commit {n}")).collect();
  commits.push(format!("Commit 10\n\n{wrapped}\n{kept}"));
  let messages: Vec<&str> = commits.iter().map(String::as_str).collect();
  let (_, text) = text_of(&input("work", &messages), &[]);
  let tenth = text.split_once("10. ").unwrap().1;
  // the item of 10 indents its body by 4, the width of its marker
  let indented: Vec<String> = format!("{joined}\n{kept}")
    .lines()
    .map(|line| if line.is_empty() { String::new() } else { format!("    {line}") })
    .collect();
  assert_eq!(tenth, format!("**Commit 10**\n{}", indented.join("\n")));
}

#[test]
fn trailers_and_autosquash_commits_are_left_out() {
  let trailed = "Add login

The form.

Co-Authored-By: Claude <noreply@example.com>
Signed-off-by: Jane <jane@example.com>
(cherry picked from commit 0123456789abcdef)";
  let prose = "Add logout

Note: the session ends.";
  let branch = input("auth", &[trailed, "fixup! Add login", "squash! Add login\n\nMore.", "amend! Add login", prose]);
  let (title, body) = text_of(&branch, &[]);
  assert_eq!(title, "Add login");
  // a one-word token is prose, not a trailer
  assert_eq!(body, "1. **Add login**\n   The form.\n\n2. **Add logout**\n   Note: the session ends.");

  // only fixups: their items are all there is
  let (_, only_fixups) = text_of(&input("auth", &["fixup! Add login", "fixup! Add logout"]), &[]);
  assert_eq!(only_fixups, "1. **fixup! Add login**\n\n2. **fixup! Add logout**");
}

#[test]
fn mentions_and_angle_brackets_stay_literal() {
  let message = "Annotate @Nullable fields

A List<String> of (@Foo), <T>, and `@Bar <U>` in a span; mail me@example.com, see <https://example.com>.";
  let (_, body) = text_of(&input("nullability", &[message]), &[]);
  assert_eq!(
    body,
    "A `List<String>` of (`@Foo`), `<T>`, and `@Bar <U>` in a span; mail me@example.com, see <https://example.com>."
  );
  let (_, items) = text_of(&input("nullability", &[message, "Check @Nullable <T>"]), &[]);
  assert!(items.contains("2. **Check `@Nullable` `<T>`**"), "{items}");
}

#[test]
fn a_stacked_branch_is_titled_by_its_own_commits_and_names_its_ancestors() {
  let mut branch = input("oauth", &["Add OAuth", "Test OAuth"]);
  branch.ancestors = vec![
    AncestorText {
      name: "auth".to_string(),
      subjects: vec!["Add auth".to_string()],
    },
    AncestorText {
      name: "login".to_string(),
      subjects: vec!["Add login".to_string(), "Add <form>".to_string()],
    },
  ];
  let (title, body) = text_of(&branch, &[]);
  assert_eq!(title, "Add OAuth");
  assert_eq!(
    body,
    "1. **Add OAuth**

2. **Test OAuth**

Built on auth, then login: their commits are in this pull request too.

**auth**
- Add auth

**login**
- Add login
- Add `<form>`"
  );

  branch.ancestors.truncate(1);
  let (_, body) = text_of(&branch, &[]);
  assert!(body.ends_with("Built on auth: its commits are in this pull request too.\n\n- Add auth"), "{body}");
}

#[test]
fn a_body_too_long_for_github_is_cut_and_names_the_commits_left_out() {
  let long = format!("Add the data\n\n{}", "word ".repeat(9_000));
  let branch = input("data", &[&long, &long, "Add the rest"]);
  let (title, body) = text_of(&branch, &[]);
  assert_eq!(title, "Add the data");
  assert!(body.chars().count() <= 65_536);
  assert!(body.contains("1. **Add the data**"));
  assert!(
    body.ends_with("_Too long for a pull request: the text of 2 commits is left out (Add the data, Add the rest)._"),
    "{}",
    &body[body.len() - 200..]
  );
}

#[test]
fn the_description_wins() {
  let description = Description {
    title: "Auth\nsecond line".to_string(),
    body: "Login and logout.".to_string(),
  };
  let mut branch = input("IJAI-1", &["IJAI-1 Add login", "IJAI-1 Add logout"]);
  branch.description = Some(&description);
  assert_eq!(text_of(&branch, &[issue("IJAI-1", Some("Auth"))]), ("Auth".to_string(), "Login and logout.".to_string()));
}

#[tokio::test]
async fn an_issue_is_asked_from_youtrack_only_when_the_repository_links_it_there() {
  let (fake, url) = start_fake_server().await;
  fake.answer(
    "GET",
    "/api/issues/IJAI-4390",
    FakeAnswer::ok(json!({"idReadable": "IJAI-4390", "summary": "Agent CLIs render without colors"})),
  );
  let dir = tempfile::tempdir().unwrap();
  let file = token_file(dir.path());
  let token = StoredToken {
    url: url.clone(),
    login: "jane".to_string(),
    full_name: "Jane Doe".to_string(),
    access_token: "access".to_string(),
    access_expires_at: None,
    refresh: None,
    created_at: 0,
  };
  write_secret_json(&file, &token).unwrap();
  let youtrack = YouTrackService::without_renewal(YouTrackClient::new("Branch Deck test"), None, Some(file));
  let links = IssueNavigationConfig {
    links: vec![
      IssueNavigationLink {
        issue_regexp: r"IJAI-\d+".to_string(),
        link_regexp: format!("{url}/issue/$0"),
      },
      IssueNavigationLink {
        issue_regexp: r"EA-\d+".to_string(),
        link_regexp: "https://ea.example.com/issue/$0".to_string(),
      },
    ],
  };
  let keys = ["IJAI-4390".to_string(), "EA-1".to_string(), "OTHER-1".to_string()];
  let issues = issue_texts(&keys, Some(&links), Some(&youtrack)).await;
  assert_eq!(
    issues,
    [
      IssueText {
        key: "IJAI-4390".to_string(),
        summary: Some("Agent CLIs render without colors".to_string()),
        url: Some(format!("{url}/issue/IJAI-4390")),
      },
      // linked elsewhere: YouTrack is not asked
      IssueText {
        key: "EA-1".to_string(),
        summary: None,
        url: Some("https://ea.example.com/issue/EA-1".to_string()),
      },
      IssueText {
        key: "OTHER-1".to_string(),
        summary: None,
        url: None,
      },
    ]
  );
  assert_eq!(fake.requests().len(), 1);
  // without YouTrack: the links only
  assert_eq!(issue_texts(&keys[..1], Some(&links), None).await[0].summary, None);
}

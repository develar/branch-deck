use crate::commit_grouper::GroupingInputs;
use crate::pick::{FollowToken, GroupedBy, PickUndoToken, picked_group};
use pretty_assertions::assert_eq;

const A: &str = "1111111111111111111111111111111111111111";
const B: &str = "2222222222222222222222222222222222222222";
const C: &str = "3333333333333333333333333333333333333333";

#[test]
fn an_undo_token_prints_and_parses_the_target_and_the_worktree_that_followed() {
  let both = PickUndoToken {
    head: Some((A.to_string(), B.to_string())),
    follow: Some(FollowToken {
      worktree_id: "lane+1".to_string(),
      previous_head: C.to_string(),
      new_head: B.to_string(),
    }),
  };
  assert_eq!(both.to_string(), format!("pick:{A}..{B}+follow:lane+1:{C}..{B}"));
  assert_eq!(PickUndoToken::parse(&both.to_string()), Some(both));
  let only_the_follow = format!("pick:+follow:main:{C}..{B}");
  assert_eq!(PickUndoToken::parse(&only_the_follow).map(|token| token.to_string()), Some(only_the_follow));

  for invalid in [
    "pick:".to_string(),
    format!("pick:{A}..{A}"),
    format!("pick:{}..{B}", &A[..8]),
    format!("pick:{A}..{B}+follow:{C}..{B}"),
    format!("pick:{A}..{B}+follow:../x:{C}..{B}"),
    format!("update:{A}..{B}"),
  ] {
    assert_eq!(PickUndoToken::parse(&invalid), None, "{invalid}");
  }
  assert!(PickUndoToken::is_pick_token("pick:anything"));
}

#[test]
fn a_picked_commit_names_what_groups_it() {
  let mut grouping = GroupingInputs::default();
  grouping.issue_groups.entries.insert("ABC-2".to_string(), "parser".to_string());
  let cases = [
    ("(lane) Add x", Some("lane"), GroupedBy::Prefix),
    ("ABC-2 Parse y", Some("parser"), GroupedBy::IssueGroup),
    ("ABC-1 Fix z", Some("ABC-1"), GroupedBy::Issue),
    ("setup vm: one more script", None, GroupedBy::None),
  ];
  for (subject, group, grouped_by) in cases {
    let picked = picked_group(&grouping, subject);
    assert_eq!((picked.group.as_deref(), picked.grouped_by), (group, grouped_by), "{subject}");
  }
}

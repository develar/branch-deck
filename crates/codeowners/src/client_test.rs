use crate::client::CodeOwnersError;
use crate::fake_codeowners::{FAKE_TOKEN, group_json, member_json, start_fake};
use domain_types::code_owners::{GroupMember, OwnerGroup};
use pretty_assertions::assert_eq;

#[tokio::test]
async fn a_lookup_names_the_reviewers_and_the_members_with_their_logins_and_availability() {
  let (fake, client) = start_fake().await;
  fake.set_group(
    "Air Plugin",
    group_json(
      "Air Plugin",
      &[member_json("rita", Some("rita-gh"), "AVAILABLE", false)],
      &[member_json("mike", Some("mike-gh"), "VACATION", false), member_json("nolan", None, "AVAILABLE", true)],
    ),
  );
  let group = client.lookup_group(FAKE_TOKEN, "Air Plugin").await.unwrap();
  let member = |username: &str, github: Option<&str>, available: bool, rotated: bool| GroupMember {
    username: username.to_owned(),
    github_login: github.map(str::to_owned),
    available,
    rotated,
  };
  assert_eq!(
    group,
    OwnerGroup {
      name: "Air Plugin".to_owned(),
      reviewers: vec![member("rita", Some("rita-gh"), true, false)],
      members: vec![member("mike", Some("mike-gh"), false, false), member("nolan", None, true, true)],
    }
  );
  // the name with its space is one segment of the path
  assert_eq!(fake.lookups(), ["Air Plugin"]);
}

#[tokio::test]
async fn a_group_the_service_lacks_has_no_members_and_a_rejected_token_is_an_error() {
  let (_fake, client) = start_fake().await;
  assert_eq!(
    client.lookup_group(FAKE_TOKEN, "Unknown").await.unwrap(),
    OwnerGroup {
      name: "Unknown".to_owned(),
      ..OwnerGroup::default()
    }
  );
  assert_eq!(client.lookup_group("expired", "Unknown").await, Err(CodeOwnersError::Unauthorized));
}

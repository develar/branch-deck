use crate::parse::parse_youtrack_instances;
use crate::test_support::fakes;
use crate::youtrack_login::sign_in_youtrack_with_token;
use pretty_assertions::assert_eq;

#[test]
fn hands_the_token_on_stdin_and_keeps_the_instance_the_user_had_active() {
  let (root, fakes, environment) = fakes();
  fakes.install("youtrack", "0.7.0");
  // the user's own instance, active
  fakes.set("youtrack.auth", r#"{"alias":"work","url":"https://work.example","login":"me","status":"authenticated"}"#);
  fakes.set(
    "youtrack.others",
    r#"{"alias":"work","url":"https://work.example","login":"me","status":"authenticated","active":false},"#,
  );
  let login = sign_in_youtrack_with_token(&environment, "https://yt.example/", "perm-secret").unwrap();
  assert_eq!(login, "jane");
  assert_eq!(std::fs::read_to_string(root.path().join("state/youtrack.token")).unwrap(), "perm-secret");
  let log = fakes.log();
  // the token is no argument; the user's instance active again
  assert!(log.iter().all(|line| !line.contains("perm-secret")), "{log:?}");
  assert!(
    log.contains(&"youtrack auth login --base-url https://yt.example --no-browser --json --quiet".to_string()),
    "{log:?}"
  );
  assert!(log.contains(&"youtrack instance use work --quiet".to_string()), "{log:?}");
}

#[test]
fn a_cli_without_instances_needs_no_instance_back() {
  let (_root, fakes, environment) = fakes();
  fakes.install("youtrack", "0.7.0");
  assert_eq!(sign_in_youtrack_with_token(&environment, "https://yt.example", "perm-x").unwrap(), "jane");
  assert!(!fakes.log().iter().any(|line| line.contains("instance use")), "{:?}", fakes.log());
}

#[test]
fn reads_the_alias_and_the_active_instance() {
  let one = parse_youtrack_instances(r#"{"alias":"work","url":"https://a.example/","login":"me","status":"authenticated"}"#).unwrap();
  assert_eq!((one[0].alias.as_deref(), one[0].active, one[0].authenticated), (Some("work"), true, true));
  let many = parse_youtrack_instances(
    r#"[{"alias":"a","url":"https://a.example","status":"authenticated","active":false},{"alias":"b","url":"https://b.example","status":"invalid token","active":true}]"#,
  )
  .unwrap();
  assert_eq!(many.iter().map(|it| (it.active, it.authenticated)).collect::<Vec<_>>(), [(false, true), (true, false)]);
}

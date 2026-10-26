use crate::facts::fill;
use crate::http_budget_test::{TIP, counted, failed_robot, session, table};
use crate::land::{LandHost, one_shot_plan, prepare, prepare_one_shot};
use crate::needs::needs;
use domain_types::fixtures::land_intent;
use domain_types::landing::{Land, LandFact, PlannedEffect, ProfileChoice, StartRoute, Subject};
use domain_types::operation::{Intent, Plan};
use domain_types::patronus::{LandingAllowances, PatronusPushMode};
use domain_types::session::LandedSection;
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use repository_model::branch_table::BranchTable;
use repository_model::session::SessionKey;
use std::collections::BTreeMap;

fn safe_push(subject: &str) -> Land {
  Land {
    subject: Subject::branch(subject),
    mode: PatronusPushMode::RebaseAutosquash,
    profile: ProfileChoice::Default,
    without_approvals: false,
    create_pull_request: false,
    allow: LandingAllowances::default(),
  }
}

fn host<'a>(table: &'a BranchTable, baseline: &'a LandedSection) -> LandHost<'a> {
  LandHost {
    table,
    baseline,
    target: "master",
    github: true,
    gate: &[],
    target_conflict: None,
  }
}

/// The plan of `bd`'s start: decided over the facts of the first round the session holds, then over the review its first
/// decision waits for, read once; what it does, the profiles it offers and the intent it is of
#[tokio::test(flavor = "multi_thread")]
async fn a_start_is_planned_over_what_the_session_holds_and_reads_the_review_its_first_decision_waits_for() {
  let (counted, sources) = counted(failed_robot());
  let git = GitCommandExecutor::new();
  let mut session = session();
  let land = safe_push("auth");
  fill(&mut session, &sources, &git, &needs(&Intent::Land(land.clone()))).await;
  let first = BTreeMap::from([("details", 1), ("profiles", 1), ("pull_requests", 1), ("robots", 1)]);
  assert_eq!(counted.calls(), first);
  let (table, baseline) = (table(), LandedSection::default());
  let host = host(&table, &baseline);
  let waiting = one_shot_plan(&session, &land, &host).unwrap();
  assert!(waiting.waits.iter().any(|wait| wait.fact == LandFact::Readiness), "{:?}", waiting.waits);

  let plan = prepare_one_shot(&mut session, &sources, &git, &land, &host).await.unwrap().plan;
  let mut read = first;
  read.insert("readiness", 1);
  assert_eq!(counted.calls(), read);
  let Plan {
    intent,
    waits,
    blockers,
    effects,
    lands,
    alternatives,
    ..
  } = &plan;
  assert_eq!((intent, waits, blockers), (&Intent::Land(land.clone()), &Vec::new(), &Vec::new()));
  assert_eq!(lands, &["auth"]);
  assert_eq!(
    effects,
    &[
      PlannedEffect::Push {
        branches: vec!["auth".to_owned()],
        force: false,
      },
      PlannedEffect::StartRobot {
        route: StartRoute::Api { profile: None },
      },
    ]
  );
  assert!(alternatives.is_some(), "a start offers its profiles");
  // the facts answer nothing it waits for: nothing more is read
  assert_eq!(prepare_one_shot(&mut session, &sources, &git, &land, &host).await.map(|planned| planned.plan), Some(plan));
  assert_eq!(counted.calls()["readiness"], 1);
}

/// The fingerprint of a plan is the same while the intent and what it was decided on are, and another for another intent
/// or when the virtual branch it pushes or its remote branch moved
#[tokio::test(flavor = "multi_thread")]
async fn the_fingerprint_of_a_plan_changes_with_its_intent_and_the_tips_it_was_decided_on() {
  let (_, sources) = counted(failed_robot());
  let git = GitCommandExecutor::new();
  let mut session = session();
  fill(&mut session, &sources, &git, &needs(&land_intent("auth", PatronusPushMode::RebaseAutosquash))).await;
  let baseline = LandedSection::default();
  let table = table();
  let land = safe_push("auth");
  let planned = one_shot_plan(&session, &land, &host(&table, &baseline)).unwrap().fingerprint;
  assert_eq!(one_shot_plan(&session, &land, &host(&table, &baseline)).unwrap().fingerprint, planned);

  let dry_run = Land {
    mode: PatronusPushMode::DryRun,
    ..land.clone()
  };
  assert_ne!(one_shot_plan(&session, &dry_run, &host(&table, &baseline)).unwrap().fingerprint, planned);
  let mut moved = table.clone();
  moved.tips.insert("user/virtual/auth".to_owned(), TIP.replace('1', "2"));
  assert_ne!(one_shot_plan(&session, &land, &host(&moved, &baseline)).unwrap().fingerprint, planned);
  let mut pushed = table.clone();
  pushed.remote.branches.get_mut("auth").unwrap().remote_tip = Some(TIP.to_owned());
  assert_ne!(one_shot_plan(&session, &land, &host(&pushed, &baseline)).unwrap().fingerprint, planned);
  // a subject the table lacks has no plan
  assert_eq!(one_shot_plan(&session, &safe_push("gone"), &host(&table, &baseline)), None);
}

/// The app's plan is read on the actor of the session: none for a session that is not open, nor before its Patronus
/// view
#[tokio::test(flavor = "multi_thread")]
async fn the_plan_of_a_session_waits_for_its_patronus_view() {
  let sessions = crate::registry::registry_test::sessions();
  let key = SessionKey {
    worktree: "/repository".to_owned(),
    prefix: "user".to_owned(),
  };
  assert_eq!(prepare(&sessions, &key, safe_push("auth")).await, None);
  let _open = sessions.hold(&key);
  assert!(sessions.is_open(&key));
  assert_eq!(prepare(&sessions, &key, safe_push("auth")).await, None);
}

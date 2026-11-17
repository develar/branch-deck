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
use repository_model::facts::Needs;
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
    anyway: None,
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

/// A start reads only fresh landing needs (`land_session::SessionStart::plan`): planned while the session computes the
/// needs of the tips a sync moved on the same baseline, whose steps keep the needs before meanwhile (the part `Stale`)
/// and whose decision waits for them, it waits for them too and is decided on them, never on the needs before; here
/// the new needs refuse it, which the needs before did not
#[tokio::test(flavor = "multi_thread")]
async fn a_start_planned_while_the_needs_of_new_tips_are_computed_waits_for_them_and_is_decided_on_them() {
  use crate::land_execute::StartHost;
  use crate::push::push_test::{OFFLINE, TestNeeds, host_with_needs, registry_on};
  use domain_types::landing::{FactWait, LandingFix, LandingRefusal};
  use domain_types::reason::Reason;
  use domain_types::session::{FactState, SyncReason};
  use std::sync::Arc;
  let needs = Arc::new(TestNeeds::default());
  let repo = test_utils::git_test_utils::stack_ahead_of_bare_origin();
  let (registry, key) = registry_on(&repo, host_with_needs(Arc::clone(&needs)), true).await;
  let sessions = registry.sessions();
  let Intent::Land(land) = land_intent("alpha", PatronusPushMode::RebaseAutosquash) else {
    unreachable!()
  };
  // the other facts the start reads (the profiles, the review) stay unanswered offline: only the needs are of the test
  let needs_wait = FactWait {
    fact: LandFact::LandingNeeds,
    failed: None,
  };
  let decided = soon("the plan on the first needs", async {
    loop {
      if let Some(plan) = prepare(sessions, &key, land.clone()).await
        && !plan.waits.contains(&needs_wait)
      {
        break plan;
      }
      tokio::task::yield_now().await;
    }
  })
  .await;
  assert!(!decided.blockers.iter().any(|reason| matches!(reason, Reason::LandingRefused { .. })), "{decided:?}");
  assert_eq!(sessions.get_snapshot(&key).await.unwrap().patronus.landing_needs, FactState::Fresh);

  // a commit of alpha: its new tip on the same baseline, whose needs are computed while the test holds them
  let (started, release) = needs.held.hold();
  repo.create_commit("(alpha) Alpha again", "alpha-again.txt", "again\n");
  sessions.sync(&key, SyncReason::Manual).wait().await.unwrap();
  soon("the computation of the new needs", started).await.unwrap();
  assert_eq!(sessions.get_snapshot(&key).await.unwrap().patronus.landing_needs, FactState::Stale { reason: None });
  let waiting = prepare(sessions, &key, land.clone()).await.unwrap();
  assert!(waiting.waits.contains(&needs_wait), "the decision of the start waits for the fresh needs: {waiting:?}");

  // the new needs refuse it: alpha uses a name gamma adds
  let refusal = LandingRefusal::MissingDeclaration {
    branch: "alpha".to_string(),
    identifier: "Gamma".to_string(),
    file: "alpha.txt".to_string(),
    line: 1,
    used_by: domain_types::fixtures::stack_commit("a2", "Alpha again", "alpha"),
    declared_by: domain_types::fixtures::stack_commit("g1", "Gamma", "gamma"),
    fix: LandingFix::Restack {
      branch: "alpha".to_string(),
      onto: "gamma".to_string(),
    },
    landing_in: None,
  };
  *needs.answer.lock().unwrap() = Needs::from([("alpha".to_string(), vec![refusal.clone()])]);
  let (host, _) = crate::land_session::pull_request_start(&OFFLINE, sessions, &key).await.unwrap();
  // the plan waits for the fresh needs: it subscribes to the session, which only that wait does, and is still pending
  // then; the release comes only after it, so a plan on the needs before would have answered first
  let subscribers = || sessions.open_sessions().into_iter().find(|open| open.key == key).map_or(0, |open| open.subscribers);
  let before = subscribers();
  let mut plan = host.plan(&land);
  soon("the plan waits for the fresh needs", async {
    while subscribers() == before {
      let polled = std::future::poll_fn(|cx| std::task::Poll::Ready(plan.as_mut().poll(cx).is_ready())).await;
      assert!(!polled, "the plan answered while the needs of the new tip were computed");
      tokio::task::yield_now().await;
    }
  })
  .await;
  let polled = std::future::poll_fn(|cx| std::task::Poll::Ready(plan.as_mut().poll(cx).is_ready())).await;
  assert!(!polled, "the plan still waits once it subscribed");
  release.send(()).unwrap();
  let planned = soon("the plan on the new needs", plan).await;
  let planned = planned.unwrap().expect("a plan of alpha");
  assert!(!planned.plan.waits.contains(&needs_wait), "{:?}", planned.plan);
  assert!(
    planned.plan.blockers.contains(&Reason::LandingRefused { refusal: Box::new(refusal) }),
    "decided on the new needs: {:?}",
    planned.plan
  );
}

/// `body` within a minute
async fn soon<T>(what: &str, body: impl Future<Output = T>) -> T {
  tokio::time::timeout(std::time::Duration::from_mins(1), body)
    .await
    .unwrap_or_else(|_elapsed| panic!("{what} within a minute"))
}

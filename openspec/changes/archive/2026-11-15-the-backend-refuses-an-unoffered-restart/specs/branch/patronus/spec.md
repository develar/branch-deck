## ADDED Requirements

### Requirement: Restart and cancel: The backend holds to the decision of the menus

The plan and the run of a restart MUST refuse it where the session offers it not as the menus decide it
(`domain_policy::actions::robot_actions` of the robot, without the operations of the session), before any push or
restart, with the code of its blocker: whoever asks (a window of a state before, another client) restarts nothing the
menus would not.

#### Scenario: A restart the session offers not, asked anyway
- **WHEN** a restart is asked while Patronus has not told why the Safe Push failed, once it failed to tell it, while the Safe Push waits for approvals, or of a robot the session offers no restart of
- **THEN** its plan and its run refuse it: `plan_changed` "Waits for Patronus to tell why the Safe Push failed", `patronus_error` "Patronus didn't tell why the Safe Push failed (…)", `approvals_missing`, `invalid_arguments`, each with "nothing was pushed or restarted"; Patronus gets no restart
  [@test] crates/operations/src/robot_test.rs (`a_restart_runs_only_where_the_session_offers_it_and_each_blocker_refuses_it_by_its_code`)
  [@test] tests/e2e/next-step/facts-without-answer.spec.ts (`should refuse in the backend a restart the session offers not, before any push or restart: while Patronus tells why the Safe Push failed, and once it failed to tell it`)

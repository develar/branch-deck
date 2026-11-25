## MODIFIED Requirements

### Requirement: Merged: A running robot the session sees successful is news of its merge

A robot known as running or canceling that an answer of the session (a poll, a list, the robot of a cancel) shows
successful and not a dry run merged its branch (`domain_policy::news::robot_finished`): it MUST be news of the session
(`News`, `robotMerged`, with the pull requests its landing closes, [below](#merged)). A robot first seen finished MUST
NOT be news.

#### Scenario: A robot known running finished
- **GIVEN** the session knows a robot as running or canceling
- **WHEN** an answer of the session shows it successful and not a dry run
- **THEN** the session has the news `robotMerged` of it, and of a failed one its failure
  [@test] crates/domain-policy/src/news_test.rs (`a_robot_known_running_that_finished_merged_or_failed`)

#### Scenario: The news waits for the view of its robots
- **WHEN** a robot merged
- **THEN** the news is told once the Patronus view of the robots it merged in stands (at once while the session has no table to derive it of)
- **AND** it names the pull requests its landing closes
  [@test] crates/repository-model/src/facts_test.rs (`a_merge_is_news_at_once_and_a_failure_once_its_problems_say_why`, `a_merge_waits_for_the_view_of_its_robots_and_names_the_pull_requests_its_landing_closes`)

#### Scenario: The main window tells the news once
- **WHEN** the news of a merge comes, also while the window reads the view of the session anew (after a patch it lacks,
  or the session opened anew in place, which numbers its news on under its line, `newsLine`)
- **THEN** the main window tells it once (`tellRobotNews`; none of those its first view of a numbering had: its first
  view, the session of another prefix)
  [@test] src/model/store.test.ts (`tells the news of a session opened anew in place that came while it read its view: the session numbers them on under its line, so none is lost or told twice`, `tells none of the news of a view read anew that it told before, and none of a view of another numbering: the session of another prefix`)

#### Scenario: A successful dry run or a robot first seen merged
- **WHEN** a dry run succeeds, or the session first sees a robot merged
- **THEN** nothing is told
  [@test] src/stores/patronus.test.ts (`does not tell a successful dry run or a robot first seen merged`)

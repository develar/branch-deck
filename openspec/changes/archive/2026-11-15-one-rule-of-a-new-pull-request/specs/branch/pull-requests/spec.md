## MODIFIED Requirements

### Requirement: Creating a pull request: A pull request a refresh brings disables the creation

One a refresh brings while the dialog is open that refuses a new one ([below](#requirement-creating-a-pull-request-one-rule-says-when-a-pull-request-refuses-a-new-one))
MUST turn it into "feature-auth already has pull request #7." with "Open Pull Request"; Create and "Continue on GitHub"
MUST be disabled, their hover saying why: GitHub's form would open nothing.

#### Scenario: A refresh brings an open pull request
- **WHEN** a refresh brings an open pull request of the branch while the dialog is open
- **THEN** the dialog says "feature-auth already has pull request #7." with "Open Pull Request"
- **AND** Create and "Continue on GitHub" are disabled, their hover "feature-auth already has pull request #7"
  [@test] src/presenters/pull-request-url.test.ts (`says why GitHub's form and Create can't make a new pull request: a live one; a branch not on the remote can, it is pushed first`)
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should say why it can't create, and offer only Open Pull Request once the branch has one`)

#### Scenario: A merged pull request
- **WHEN** the pull request of the branch is merged and the branch landed by it
- **THEN** Create and "Continue on GitHub" are disabled, their hover "Pull request #7 of feature-auth is merged"
- **AND** one merged that did not land the branch lets a new pull request repeat it
  [@test] src/lib/pull-request-dialog.test.ts (`lets a new pull request repeat a merged one only while Rust offers its creation, and says the one that refuses it`)

### Requirement: Creating a pull request: One rule says when a pull request refuses a new one

A pull request of a subject MUST refuse a new one only while it is open, or merged so that it landed the subject (all
of it landed, as Rust's next step says); any other MUST let a new one repeat it. One rule of Rust decides it
(`domain_policy::pull_request::creation_blocker`), which the menus, the dialogs and the engine read, never a copy.

#### Scenario: Who reads the rule
- **WHEN** a pull request refuses a new one
- **THEN** the menus offer Open Pull Request in the place of Create Pull Request…, the dialogs say it from the actions Rust offers, and the plan and the run of a creation refuse it before any push (`pullRequestOpen`, `pullRequestMerged`), for whoever asks
  [@test] crates/domain-policy/src/actions_test.rs (`the_review_the_pull_request_actions_and_the_rarer_starts_follow_their_facts`)
  [@test] src/lib/pull-request-dialog.test.ts (`lets a new pull request repeat a merged one only while Rust offers its creation, and says the one that refuses it`)

#### Scenario: The five cases
- **WHEN** a subject has an open pull request, one merged that landed it, one merged into the target with commits after it, one merged into a parent's branch, a closed one
- **THEN** a new one is refused, refused, offered, offered, offered
  [@test] crates/domain-policy/src/pull_request_test.rs (`a_new_pull_request_is_refused_only_beside_an_open_one_or_a_merged_one_that_landed_the_subject`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_review_the_pull_request_actions_and_the_rarer_starts_follow_their_facts`)
  [@test] crates/operations/src/pull_request_test.rs (`a_creation_pushes_first_what_the_remote_lacks_then_opens_the_pull_request`)

#### Scenario: Alike in the menu, the dialog and the backend
- **GIVEN** #763 of feature-db merged a commit the branch is no longer at, and #760 of JIRA-456 is open
- **WHEN** the user opens their menus, and another client asks for a creation of each
- **THEN** feature-db offers Create Pull Request… beside Open Pull Request #763, its dialog says nothing against it, its plan has no blocker
- **AND** JIRA-456 offers Open Pull Request #760 in the place of Create, and its creation is refused before a push: "Nothing was pushed or created: JIRA-456 has the open pull request #760: Open Pull Request shows it" (`plan_changed`), GitHub gets no creation
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should offer a new pull request beside a merged one that did not land the branch, and refuse one beside an open one, alike in the menu, the dialog and the backend`)

---
name: Reviews
description: Requirements for the Reviews view of the main window: every open pull request of the selected repository the user takes part in on GitHub, beyond the requests by name of Your Turn (the requests of the user's teams, the pull requests the user reviewed, the user's own), each once, and one field that filters every section by the words typed and adds a pull request by its link when its author forgot to ask the user; read in the request of the review requests only while the view shows, its sections and the actions of each pull request decided in Rust.
style: plain-1
targets:
  - crates/domain-types/src/review_request.rs
  - crates/domain-types/src/session.rs
  - crates/domain-types/src/actions.rs
  - crates/domain-policy/src/reviews.rs
  - crates/domain-policy/src/actions.rs
  - crates/domain-policy/src/refresh.rs
  - crates/remote-facts/src/reviews.rs
  - crates/remote-facts/src/review_requests.rs
  - crates/remote-facts/src/scheduler_review_requests.rs
  - crates/remote-facts/src/service_sources.rs
  - crates/repository-model/src/review_request_facts.rs
  - crates/repository-model/src/review_view.rs
  - crates/repository-model/src/runtime.rs
  - crates/repository-model/src/session.rs
  - crates/app-settings/src/linked_pull_requests.rs
  - crates/operations/src/review.rs
  - crates/github/src/fake_repository.rs
  - crates/test-server/src/review_fixtures.rs
  - crates/test-server/src/session_routes.rs
  - crates/app-api/src/session/reviews.rs
  - crates/app-api/src/session/view.rs
  - src/components/reviews/ReviewsView.tsx
  - src/components/reviews/FilteredOnGitHub.tsx
  - src/components/reviews/ReviewEntryRow.tsx
  - crates/domain-policy/src/landing_node.rs
  - src/components/shared/SearchField.tsx
  - src/hooks/useSlashFocus.ts
  - src/lib/text-search.ts
  - src/lib/review-requests.ts
  - src/components/repository/ReviewRequestRow.tsx
  - src/components/repository/YourTurnCard.tsx
  - src/hooks/useReturnedToYou.ts
  - src/presenters/reviews.ts
  - src/presenters/actions.ts
  - src/stores/reviews.ts
  - src/model/demand.ts
  - src/model/patch.ts
  - src/lib/review-request-actions.ts
  - src/lib/pull-request-review.ts
  - src/lib/review-window.ts
  - src/components/review/ReviewHeader.tsx
  - src/components/review/PullRequestReview.tsx
  - src/pages/PullRequestReviewWindow.tsx
  - src/lib/command-palette.ts
---

# Reviews

Status: Implemented
Date: 2026-10-27

## Purpose

Your Turn ([Review Requests](../review-requests/spec.md)) tells what waits for the user by name. The rest of what the
user takes part in on GitHub was told only by github.com/pulls: the requests of the user's teams (CODEOWNERS), the
pull requests the user reviewed, the user's own pull requests and their reviewers. And sometimes the author forgets to
ask the user: the user has only a link. Reviews is the second view of the main window (Branches · Reviews · Landing,
[Main Views](../main-views/spec.md)): every open pull request of the selected repository the user takes part in, Your
Turn's rows first, then the sections, each pull request once, and one field that filters them by the words typed and
adds any pull request of the repository by its link, opening it in the review window. Your Turn stays above the branches as it is.

## Goals

- See in one place what the user takes part in on GitHub, beyond what waits for the user by name.
- Find a pull request among them by a few words of it.
- Review a pull request nobody asked the user for, from its link, in the app.
- Ask GitHub for it only while the user looks.

## Non-goals

- Other repositories than the selected one, and pull requests the user only commented on or is mentioned in.
- News, toasts, OS notifications and the Dock badge of the sections: they stay Your Turn's.
- Knowing which requested team the user is in: Reviews names the teams GitHub requests.
- Following a link across the worktrees of other GitHub repositories: a link names a pull request of this one.
- A `brd` command for Reviews yet: the rule is pure and `brd` can call it.

## Requirements

### Requirement: Reading: The rest of Reviews is read only while a window shows the view

While a window shows the Reviews view (`Demand::reviews`, held while it shows, focused or not), every list of the
review requests of the session MUST read the rest of Reviews in the same GraphQL request. While no window shows it, the
request MUST be the one of Your Turn, and nothing more is read.

#### Scenario: A window shows the view
- **GIVEN** a window shows the Reviews view, focused or not (`Demand::reviews`, held while it shows)
- **WHEN** the session lists the review requests
- **THEN** the same GraphQL request reads the rest of Reviews
  [@test] crates/remote-facts/src/reviews_test.rs (`reads_the_rest_of_reviews_in_the_request_of_the_review_requests`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`)

#### Scenario: No window shows the view
- **GIVEN** no window shows the Reviews view
- **WHEN** the session lists the review requests
- **THEN** the request is the one of Your Turn, and nothing more is read
  [@test] crates/remote-facts/src/reviews_test.rs (`without_the_reviews_view_the_request_is_the_one_of_your_turn`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`)

### Requirement: Reading: The rest is three searches and the links by their numbers

The rest of Reviews MUST be the searches `team`, `reviewed` and `authored`, each open and not archived, drafts only of
the user's own, 50 each, and the pull requests the user added by link, by their numbers (`pullRequest(number:)`
aliases).

#### Scenario: The searches and the links of the request
- **WHEN** a list of the review requests reads the rest of Reviews
- **THEN** it searches `team` (`review-requested:@me`), `reviewed` (`reviewed-by:@me -author:@me`) and `authored`
  (`author:@me`), each open and not archived, drafts only of the user's own, 50 each
- **AND** it reads the pull requests the user added by link by their numbers (`pullRequest(number:)` aliases)
  [@test] crates/remote-facts/src/reviews_test.rs (`reads_the_rest_of_reviews_in_the_request_of_the_review_requests`)

### Requirement: Reading: A token without read:org is asked again without the teams

A token without `read:org` MUST be asked again once without the team slugs, as the pull requests are.

#### Scenario: The token lacks read:org
- **GIVEN** a token without `read:org`
- **WHEN** the request with the rest of Reviews is refused for it
- **THEN** it is asked again once without the team slugs, as the pull requests are
  [@test] crates/remote-facts/src/reviews_test.rs (`a_token_without_read_org_is_asked_again_without_the_teams`)

### Requirement: Reading: The view that shows reads the rest when it is old or never read

The view that shows MUST read the rest at once when it was never read or is older than two minutes (at least a minute
after it was asked); off GitHub and under a rate limit it MUST NOT. A list in flight that doesn't read it MUST ask once
more after it.

#### Scenario: Never read, or older than two minutes
- **WHEN** the view shows and the rest was never read, or is older than two minutes (at least a minute after it was
  asked)
- **THEN** the rest is read at once
  [@test] crates/domain-policy/src/refresh_test.rs (`the_reviews_view_lists_the_rest_of_reviews_when_it_is_older_than_two_minutes_or_never_read`)
  [@test] crates/remote-facts/src/scheduler_test.rs (`the_reviews_view_reads_the_rest_of_reviews_with_the_review_requests_while_it_shows`)

#### Scenario: Off GitHub or under a rate limit
- **WHEN** the view shows off GitHub or under a rate limit
- **THEN** the rest is not read at once
  [@test] crates/domain-policy/src/refresh_test.rs (`the_reviews_view_lists_the_rest_of_reviews_when_it_is_older_than_two_minutes_or_never_read`)

#### Scenario: A list in flight that doesn't read the rest
- **GIVEN** a list of the review requests in flight that doesn't read the rest
- **WHEN** the view shows
- **THEN** the session asks once more after that list

### Requirement: Reading: While the view shows, every read of the requests reads the rest

While the view shows, the poll, a focus, a sync, Refresh and a review MUST read the rest with the requests.

#### Scenario: The poll while the view shows
- **GIVEN** the view shows
- **WHEN** the poll, a focus, a sync, Refresh or a review reads the requests
- **THEN** it reads the rest with them
  [@test] crates/remote-facts/src/scheduler_test.rs (`the_reviews_view_reads_the_rest_of_reviews_with_the_review_requests_while_it_shows`)

### Requirement: Reading: The rest is a part of the review requests fact

The rest MUST be a part of the review requests fact of the session: `Unknown` until a window shows the view, `Loading`
until a list brings it, then as the list stands (`Stale` with the reason through a failure, gone with no token or a
rejected one); a list without it MUST keep it.

#### Scenario: The rest stands by its listing
- **WHEN** a window shows the view for the first time and a list brings the rest
- **THEN** the rest is `Unknown` until the window shows the view, `Loading` until the list brings it, then as the list
  stands: `Stale` with the reason through a failure, gone with no token or a rejected one
  [@test] crates/repository-model/src/review_request_facts_test.rs (`the_rest_of_reviews_stands_by_its_listing_and_stays_through_one_without_it`)

#### Scenario: A list without the rest
- **WHEN** a list of the review requests comes without the rest
- **THEN** the rest stays as it was
  [@test] crates/repository-model/src/review_request_facts_test.rs (`the_rest_of_reviews_stands_by_its_listing_and_stays_through_one_without_it`)

### Requirement: Reading: The requests the user answered with a comment come with each list

The requests by name the user answered with a comment MUST come with each list; Your Turn holds them back.

#### Scenario: A request answered with a comment
- **GIVEN** a request by name the user answered with a comment
- **WHEN** a list of the review requests comes
- **THEN** it brings that request among those answered
- **AND** Your Turn holds it back
  [@test] crates/remote-facts/src/review_requests_test.rs (`a_request_the_user_answered_with_a_comment_is_not_their_turn`)

### Requirement: Sections: The view derives the sections in their order

The view MUST derive the sections (`domain_policy::reviews::sections`, `RepositoryView.reviews`) in their order: Added
by Link, Requested from Your Teams, Reviewed by You, Your Pull Requests. Your Turn's rows (the requests by name and the
returned own pull requests) MUST lead it as "Waiting for You".

#### Scenario: Your Turn's rows lead, then the sections in their order
- **WHEN** the view shows
- **THEN** Your Turn's rows (the requests by name and the returned own pull requests) lead it as "Waiting for You"
- **AND** Added by Link, Requested from Your Teams, Reviewed by You and Your Pull Requests follow in that order
  [@test] crates/repository-model/src/review_view_test.rs (`the_view_has_the_sections_of_reviews_and_the_actions_of_each_pull_request_to_review_and_what_its_approval_counts_for`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`)

#### Scenario: Added by Link and Requested from Your Teams
- **WHEN** the view derives Added by Link and Requested from Your Teams
- **THEN** Added by Link shows the newest link first
- **AND** Requested from Your Teams shows the one waiting the longest first, since the pull request was opened
  [@test] crates/domain-policy/src/reviews_test.rs (`each_pull_request_shows_once_in_the_first_section_after_your_turn_the_empty_ones_left_out`)

#### Scenario: Reviewed by You
- **WHEN** the view derives Reviewed by You
- **THEN** the user's moves come first, then those with commits since the user's review, then by the latest update
- **AND** a request the user answered with a comment is among them, "you replied"
  [@test] crates/domain-policy/src/reviews_test.rs (`reviewed_by_the_user_shows_new_commits_first_and_the_requests_answered_with_a_comment`)

#### Scenario: Your Pull Requests
- **WHEN** the view derives Your Pull Requests
- **THEN** the user's moves come first, then by the latest update
- **AND** the returned own pull requests are left to Your Turn, which leads the view as "Waiting for You"
  [@test] crates/domain-policy/src/reviews_test.rs (`the_own_pull_requests_name_their_branch_and_leave_the_returned_ones_to_your_turn`)

### Requirement: Sections: A pull request shows once and an empty section is left out

A pull request MUST show once, in the first section that has it; an empty section MUST be left out.

#### Scenario: A pull request in several sections
- **GIVEN** a pull request that is a request by name, a link and a team's request
- **WHEN** the view derives the sections
- **THEN** it shows once, in the first section that has it
  [@test] crates/domain-policy/src/reviews_test.rs (`each_pull_request_shows_once_in_the_first_section_after_your_turn_the_empty_ones_left_out`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`)

#### Scenario: Nothing beyond Your Turn
- **WHEN** a section has no pull request
- **THEN** it is left out
  [@test] crates/domain-policy/src/reviews_test.rs (`each_pull_request_shows_once_in_the_first_section_after_your_turn_the_empty_ones_left_out`)

### Requirement: Sections: A section's count is GitHub's less the earlier sections'

A section's count MUST be GitHub's less the pull requests an earlier section has; while the field filters, it MUST be
the count of the matches of it ([Search](#search)).

#### Scenario: The count of a section
- **WHEN** an earlier section has some of the pull requests GitHub counts for a section
- **THEN** the section's count is GitHub's less them
  [@test] crates/domain-policy/src/reviews_test.rs (`each_pull_request_shows_once_in_the_first_section_after_your_turn_the_empty_ones_left_out`)

#### Scenario: The count while the field filters
- **WHEN** the field filters
- **THEN** a section's count is the count of the matches of it ([Search](#search))
  [@test] tests/e2e/reviews/reviews.spec.ts (`should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches`, `should search GitHub with the words typed where it returned only a part of a section`)

### Requirement: Sections: Each section names the pull requests whose move is the user's

Each section MUST name the pull requests whose move is the user's (`ReviewSection.moves`,
`domain_policy::reviews::is_users_move`): a link the user hasn't reviewed, or not since its head moved; a pull request
the user reviewed whose head moved since, unless GitHub still counts the user's approval; an own pull request whose
branch of the table is the user's move, or, of no branch, with changes requested. A team's request is nobody's move in
particular.

#### Scenario: A link
- **WHEN** a link is one the user hasn't reviewed, or not since its head moved
- **THEN** it is the user's move
  [@test] crates/domain-policy/src/reviews_test.rs (`the_users_moves_are_a_link_to_review_a_review_its_commits_outdated_and_an_own_pull_request_to_land_or_fix`)

#### Scenario: A pull request the user reviewed
- **WHEN** the head of a pull request the user reviewed moved since
- **THEN** it is the user's move, unless GitHub still counts the user's approval (`reviewDecision` approved)
  [@test] crates/domain-policy/src/reviews_test.rs (`the_users_moves_are_a_link_to_review_a_review_its_commits_outdated_and_an_own_pull_request_to_land_or_fix`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should mark the user's moves alone, keep the other rows behind Show N More, and give the title the room before the meta`)

#### Scenario: An own pull request
- **WHEN** the branch of the table of an own pull request is the user's move (its node `ready`, `yourStep` or
  `needsFix`, `landing_node::is_users_move`)
- **THEN** the pull request is the user's move
- **AND** an own pull request of no branch is the user's move with changes requested
  [@test] crates/domain-policy/src/reviews_test.rs (`the_users_moves_are_a_link_to_review_a_review_its_commits_outdated_and_an_own_pull_request_to_land_or_fix`)
  [@test] crates/domain-policy/src/landing_node_test.rs (`a_filled_node_is_the_users_move`)

#### Scenario: A team's request
- **WHEN** a pull request is a team's request
- **THEN** it is nobody's move in particular
  [@test] crates/domain-policy/src/reviews_test.rs (`the_users_moves_are_a_link_to_review_a_review_its_commits_outdated_and_an_own_pull_request_to_land_or_fix`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`)

### Requirement: Sections: A section shows its moves and three more rows

A section MUST show its moves and three more rows, then "Show N More" (Show Less), as Your Turn shows its rows
(`pickVisibleEntries`); filtered, every match.

#### Scenario: More rows than the moves and three more
- **WHEN** a section has more rows than its moves and three more
- **THEN** it shows its moves and three more rows, then "Show N More"
- **AND** "Show N More" shows every row, then reads Show Less
  [@test] src/presenters/reviews.test.ts (`shows the user's moves and three more before Show N More`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should mark the user's moves alone, keep the other rows behind Show N More, and give the title the room before the meta`)

#### Scenario: Filtered
- **WHEN** the field filters
- **THEN** a section shows every match

### Requirement: Sections: A row's mark makes only the user's moves stand out

A row MUST be the row of a request ([Review Requests](../review-requests/spec.md#card)) by how the user takes part, so
that only the user's moves stand out. Its mark MUST be the filled `primary` dot of the user's move; else the user's
review, muted; nothing for a team request; of an own pull request of a branch of the table the node of the branch, its
hover the branch row's.

#### Scenario: The user's move
- **WHEN** a row is the user's move
- **THEN** its mark is the filled `primary` dot ("Your move: new commits since your review")
  [@test] src/presenters/reviews.test.ts (`marks the user's move alone, and the user's review muted`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`)

#### Scenario: The user's review, muted
- **WHEN** a row is no move of the user's and the user reviewed it
- **THEN** its mark is the user's review muted: `CheckIcon` approved (of an own pull request, GitHub's approval),
  `MessageSquareXIcon` changes requested, `MessageSquareIcon` commented or replied
  [@test] src/presenters/reviews.test.ts (`marks the user's move alone, and the user's review muted`)

#### Scenario: A team request
- **WHEN** a row is a team request
- **THEN** it has no mark
  [@test] src/presenters/reviews.test.ts (`marks the user's move alone, and the user's review muted`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`)

#### Scenario: An own pull request of a branch of the table
- **WHEN** a row is an own pull request of a branch of the table
- **THEN** its mark is the node of the branch
- **AND** its hover is the branch row's
  [@test] tests/e2e/reviews/reviews.spec.ts (`should show the node of the branch of an own pull request of the table, where the branch stands, and the branch on a click`)

### Requirement: Sections: A row's meta says in short what the mark doesn't

A row's meta MUST say in short what the mark doesn't; of an own pull request where it stands, of one of a branch of the
table the first line of its node.

#### Scenario: The meta of a team request and of a reviewed one
- **WHEN** a row is a team request, or a pull request the user reviewed or replied to
- **THEN** its meta reads "#201 · carol · requested from ide-platform", "#202 · carol", "#202 · carol · you replied"
  [@test] src/presenters/reviews.test.ts (`says in its meta only what its mark doesn't, the rest in the hint`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`)

#### Scenario: The meta of an own pull request
- **WHEN** a row is an own pull request of no branch of the table
- **THEN** its meta says where it stands ("changes requested by alice", "waiting for bob", "review required")
  [@test] src/presenters/reviews.test.ts (`says in its meta only what its mark doesn't, the rest in the hint`)

#### Scenario: The meta of an own pull request of a branch of the table
- **WHEN** a row is an own pull request of a branch of the table
- **THEN** its meta is the first line of its node ("#1239 · Lands through #1239")
  [@test] tests/e2e/reviews/reviews.spec.ts (`should show the node of the branch of an own pull request of the table, where the branch stands, and the branch on a click`)

### Requirement: Sections: A row shows its size only of a move

A row MUST show the size ("2 files · +12 −3") only of a move; of the rest it MUST be in the tooltip of the meta, with
what the meta leaves out.

#### Scenario: The size of a move
- **WHEN** a row is the user's move
- **THEN** it shows the size ("2 files · +12 −3")
  [@test] tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`)

#### Scenario: The size of another row
- **WHEN** a row is no move of the user's
- **THEN** its size is in the tooltip of the meta, with what the meta leaves out ("you approved 11111111",
  "approved by alice · waiting for bob")
  [@test] src/presenters/reviews.test.ts (`says in its meta only what its mark doesn't, the rest in the hint`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`)

### Requirement: Sections: A row shows its age, muted but of a move

A row MUST show its age, muted; of a move `text-foreground`.

#### Scenario: The age of a row
- **WHEN** a row shows
- **THEN** its age reads "waiting 2 days", "new commits since your review" or "updated 3 hours ago", muted
- **AND** of a move it is `text-foreground`
  [@test] tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`)

### Requirement: Sections: A click and Enter review the pull request in the review window

A click and Enter MUST review the pull request in the review window, D its changes, ⌘-click and O MUST open it on
GitHub, ↑/↓ move between the rows of a section; the menu MUST be the actions of the pull request.

#### Scenario: A click on a row
- **WHEN** the user clicks a row or presses Enter on it
- **THEN** the review window reviews its pull request
  [@test] tests/e2e/reviews/reviews.spec.ts (`should review a pull request of a team in the window of Reviews and approve it there`)

#### Scenario: The other keys and the menu
- **WHEN** the user presses D, ⌘-clicks or presses O, presses ↑/↓, or opens the menu on a row
- **THEN** D reviews its changes, ⌘-click and O open it on GitHub, ↑/↓ move between the rows of a section
- **AND** the menu is the actions of the pull request

### Requirement: Sections: An own pull request of a branch of the table shows the branch row

An own pull request of a branch of the table MUST show the branch row on a click and Enter, as Your Turn's returned
rows do.

#### Scenario: A click on an own pull request of the table
- **WHEN** the user clicks an own pull request of a branch of the table or presses Enter on it
- **THEN** the branch row shows, as Your Turn's returned rows do
  [@test] tests/e2e/reviews/reviews.spec.ts (`should show the node of the branch of an own pull request of the table, where the branch stands, and the branch on a click`)

### Requirement: Sections: A section beyond what GitHub returns links its search

With more than GitHub returns, a section MUST say "50 of 63" with "Open All on GitHub", its search.

#### Scenario: More than GitHub returns
- **WHEN** a section has more pull requests than GitHub returns
- **THEN** it says "50 of 63" with "Open All on GitHub", its search
  [@test] tests/e2e/reviews/reviews.spec.ts (`should search GitHub with the words typed where it returned only a part of a section`)

### Requirement: The actions of a pull request: Rust decides the actions of every pull request to review

Rust MUST decide the actions of every pull request to review, Your Turn's and Reviews' and those a review window shows
(`domain_policy::actions::review_actions`, `RepositoryView.actions` of `pull:<number>`). The page MUST word them
(`src/presenters/actions.ts`) and lay its window's own state over them (the commit it shows, a write that runs).

#### Scenario: The actions of the rows and of a review window
- **WHEN** the view or a review window shows a pull request to review
- **THEN** its actions are Rust's (`domain_policy::actions::review_actions`, `RepositoryView.actions` of `pull:<number>`)
- **AND** the page words them (`src/presenters/actions.ts`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_pull_request_to_review_offers_the_review_the_decisions_and_its_safe_push_into_the_target`)
  [@test] src/lib/review-request-actions.test.ts (`offers Approve and Safe Push where Rust offers it, disabled with its reason, the Safe Push alone once approved`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`)

#### Scenario: The window's own state over Rust's actions
- **WHEN** a review window shows a pull request
- **THEN** it lays its own state over Rust's actions: the commit it shows, a write that runs
  [@test] src/lib/review-request-actions.test.ts (`disables the writes of the review until the commit the user saw is there, an approval of the same commit, and an own pull request`)

### Requirement: The actions of a pull request: The actions offered to review a pull request

The actions MUST be Review, Approve, Approve and Safe Push where Patronus merges, Request Changes, Open Pull Request,
Show Branch of an own one of the table, Remove from Reviews of a link.

#### Scenario: A pull request to review
- **WHEN** Rust decides the actions of a pull request to review
- **THEN** they are Review, Approve, Approve and Safe Push where Patronus merges, Request Changes and Open Pull Request
  [@test] crates/domain-policy/src/actions_test.rs (`a_pull_request_to_review_offers_the_review_the_decisions_and_its_safe_push_into_the_target`)

#### Scenario: An own pull request of the table and a link
- **WHEN** the pull request is an own one of the table, or a link
- **THEN** the own one offers Show Branch, the link Remove from Reviews
  [@test] crates/domain-policy/src/actions_test.rs (`a_pull_request_to_review_offers_the_review_the_decisions_and_its_safe_push_into_the_target`)
  [@test] src/lib/review-request-actions.test.ts (`offers Show Branch of an own pull request and Remove from Reviews of a link where Rust offers them`)

### Requirement: The actions of a pull request: Approve is done once the user approved its head

Approve MUST be done once the user approved its head: the review the app submitted, else GitHub's latest review of the
user.

#### Scenario: The user approved the head
- **WHEN** the review the app submitted, else GitHub's latest review of the user, approved the head
- **THEN** Approve is done
- **AND** an approval of an older commit approves nothing of the head
  [@test] crates/domain-policy/src/actions_test.rs (`an_approval_of_the_head_is_done_and_the_safe_push_waits_for_the_target_patronus_pushes_into`)
  [@test] crates/repository-model/src/review_request_facts_test.rs (`a_submitted_review_and_a_shown_pull_request_outlive_the_listing`)

### Requirement: The actions of a pull request: Approve and Safe Push waits for the target

Approve and Safe Push MUST be disabled while the target is not known, for a baseline that is no branch of origin, and
for another base than the target. The target MUST be what the sync read of the baseline (`TableHeader.targetBranch`),
no git of its own.

#### Scenario: The target is not known or no branch of origin
- **WHEN** the target is not known, or the baseline is no branch of origin
- **THEN** Approve and Safe Push is disabled
  [@test] crates/domain-policy/src/actions_test.rs (`an_approval_of_the_head_is_done_and_the_safe_push_waits_for_the_target_patronus_pushes_into`)

#### Scenario: Another base than the target
- **GIVEN** the target is what the sync read of the baseline (`TableHeader.targetBranch`)
- **WHEN** the pull request has another base than the target
- **THEN** Approve and Safe Push is disabled
  [@test] crates/domain-policy/src/actions_test.rs (`an_approval_of_the_head_is_done_and_the_safe_push_waits_for_the_target_patronus_pushes_into`)
  [@test] crates/repository-model/src/review_view_test.rs (`the_view_has_the_sections_of_reviews_and_the_actions_of_each_pull_request_to_review_and_what_its_approval_counts_for`)

### Requirement: The actions of a pull request: The decisions of an own pull request are disabled

GitHub takes no review of the author: the decisions of an own pull request MUST be disabled with "Your own pull
request".

#### Scenario: An own pull request
- **WHEN** Rust decides the actions of an own pull request
- **THEN** its decisions are disabled with "Your own pull request"
  [@test] crates/domain-policy/src/actions_test.rs (`a_pull_request_to_review_offers_the_review_the_decisions_and_its_safe_push_into_the_target`)
  [@test] src/lib/review-request-actions.test.ts (`disables the writes of the review until the commit the user saw is there, an approval of the same commit, and an own pull request`)
  [@test] crates/repository-model/src/review_view_test.rs (`the_view_has_the_sections_of_reviews_and_the_actions_of_each_pull_request_to_review_and_what_its_approval_counts_for`)

### Requirement: The actions of a pull request: The review window opened from Reviews walks its rows

The review window opened from Reviews MUST walk its rows ("Reviews › 2 of 4"), say how the user takes part
("ide-platform asked for a review of carol's pull request", "Added by link: …") and, of a pull request the user
reviewed, offer "Since Your Review". A write MUST be the `SubmitReview` of any pull request.

#### Scenario: A team's request opened from Reviews
- **WHEN** the user opens a team's request from Reviews
- **THEN** the review window walks the rows of Reviews ("Reviews › 2 of 4")
- **AND** it says how the user takes part ("ide-platform asked for a review of carol's pull request", "Added by link: …")
- **AND** an approval there is the `SubmitReview` of the pull request
  [@test] tests/e2e/reviews/reviews.spec.ts (`should review a pull request of a team in the window of Reviews and approve it there`)

#### Scenario: The same pull request opened from Reviews and from Your Turn
- **WHEN** a pull request the window shows from Your Turn is opened from Reviews
- **THEN** the window starts over, walking the other list
  [@test] src/lib/review-reuse.test.ts (`keeps its review for a plain open of the request it shows, and starts over for another request or an intent`)

#### Scenario: A pull request the user reviewed
- **WHEN** the review window shows a pull request the user reviewed, opened from Reviews
- **THEN** it offers "Since Your Review"

### Requirement: Links: The field of the view takes a link of a pull request

The field of the view ([Search](#search)) MUST take a link too: a URL of a pull request (`/files` and an anchor after
it too), `owner/name#123`, `#123` or `123`.

#### Scenario: The forms of a link
- **WHEN** the field holds a URL of a pull request (`/files` and an anchor after it too), `owner/name#123`, `#123` or
  `123`
- **THEN** it names that pull request of the repository
  [@test] crates/remote-facts/src/reviews_test.rs (`a_link_names_a_pull_request_of_the_repository`)

### Requirement: Links: Enter on a link that no row is adds it through Rust

Enter on a link that no row of the view is MUST ask Rust (`link_pull_request`), which reads it from GitHub, keeps an
open one, shows it in the section of the links at once and reads Reviews again, which the answer does not wait for;
the field MUST empty and the review window open on it.

#### Scenario: Enter on a link of an open pull request
- **WHEN** the user presses Enter on a link that no row of the view is
- **THEN** Rust (`link_pull_request`) reads it from GitHub, keeps an open one and shows it in the section of the links
  at once
- **AND** it reads Reviews again, which the answer does not wait for
- **AND** the field empties and the review window opens on it
  [@test] crates/remote-facts/src/reviews_test.rs (`reads_a_link_the_user_adds`)
  [@test] crates/test-server/src/review_requests_test.rs (`a_link_joins_reviews_from_the_fake_github`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should add a pull request by its link from the field the palette focuses, keep it, and remove it from its menu`)

### Requirement: Links: A link of another repository or a missing number is refused

A link of another repository and a number the repository lacks MUST be refused under the header (`invalid_arguments`,
`no_pull_request`), no toast, the focus kept in the field, read-only while Rust reads it; a merged or closed one MUST
open but is not kept.

#### Scenario: Another repository, or a number the repository lacks
- **WHEN** the user presses Enter on a link of another repository, or on a number the repository lacks
- **THEN** it is refused under the header (`invalid_arguments`, `no_pull_request`), no toast
- **AND** the focus stays in the field, read-only while Rust reads it
  [@test] crates/remote-facts/src/reviews_test.rs (`a_link_names_a_pull_request_of_the_repository`)
  [@test] crates/test-server/src/review_requests_test.rs (`a_link_joins_reviews_from_the_fake_github`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should add a pull request by its link from the field the palette focuses, keep it, and remove it from its menu`)

#### Scenario: A merged or closed pull request
- **WHEN** the user presses Enter on a link of a merged or closed pull request
- **THEN** it opens, but is not kept

### Requirement: Links: The palette adds a pull request by link through the field

The palette's "Add Pull Request by Link…" MUST show the view with the focus in the field and its text selected, so a
paste replaces a filter.

#### Scenario: Add Pull Request by Link from the palette
- **GIVEN** the field holds a filter
- **WHEN** the user runs "Add Pull Request by Link…" from the palette
- **THEN** the view shows with the focus in the field and its text selected, so a paste replaces the filter
  [@test] tests/e2e/reviews/reviews.spec.ts (`should add a pull request by its link from the field the palette focuses, keep it, and remove it from its menu`)

### Requirement: Links: The links are Rust's, kept by GitHub repository

The links MUST be Rust's: `linkedPullRequests` of `repository-state.json`, by GitHub repository (`owner/name`, so every
worktree of it has them), the newest first, at most 50, written only through `SettingsService`, read by no window.

#### Scenario: The links of a repository
- **WHEN** the user adds links
- **THEN** they are kept in `linkedPullRequests` of `repository-state.json` by GitHub repository (`owner/name`, so
  every worktree of it has them), the newest first, at most 50
- **AND** only `SettingsService` writes them, and no window reads them
  [@test] crates/app-settings/src/linked_pull_requests_test.rs (`keeps_the_links_of_each_repository_newest_first_in_the_repository_state`, `a_repository_keeps_the_newest_links`)

### Requirement: Links: A link GitHub answers merged, closed or missing goes

A link GitHub answers merged, closed or missing MUST go.

#### Scenario: A link no longer open
- **WHEN** GitHub answers a link merged, closed or missing
- **THEN** the link goes
  [@test] crates/domain-policy/src/reviews_test.rs (`a_merged_closed_or_missing_link_goes`)

### Requirement: Links: Remove from Reviews removes a link at once

Remove from Reviews (`unlink_pull_request`) MUST remove a link, from the section at once, and read Reviews again, which
nothing waits for.

#### Scenario: Remove from Reviews
- **WHEN** the user chooses Remove from Reviews in the menu of a link
- **THEN** `unlink_pull_request` removes it from the section at once
- **AND** Reviews is read again, which nothing waits for
  [@test] crates/repository-model/src/review_request_facts_test.rs (`a_link_joins_and_leaves_the_links_of_reviews_before_the_listing`)
  [@test] crates/test-server/src/review_requests_test.rs (`a_link_joins_reviews_from_the_fake_github`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should add a pull request by its link from the field the palette focuses, keep it, and remove it from its menu`)

### Requirement: Links: A link another list has shows once, in the first that has it, and stays a link

A link that another list also has MUST show once, as every pull request does: in Waiting for You when Your Turn's rows
have it (a request by name, a returned own pull request), else in Added by Link, which comes before the sections that
also list it and which leave it out. Either way it MUST stay a link: a list that has it never takes it from the links.

#### Scenario: A link that a later section lists
- **GIVEN** a link that Requested from Your Teams also lists
- **WHEN** the view derives the sections
- **THEN** the link shows in Added by Link
- **AND** Requested from Your Teams leaves it out, its count GitHub's less it
  [@test] crates/domain-policy/src/reviews_test.rs (`each_pull_request_shows_once_in_the_first_section_after_your_turn_the_empty_ones_left_out`)

#### Scenario: A link that Your Turn's rows have
- **GIVEN** a link that is a request by name of the user
- **WHEN** the view derives the sections
- **THEN** the link shows in Waiting for You, and Added by Link leaves it out
- **AND** it stays among the links of the repository

### Requirement: Search: The field stands in the header row

The field "Filter, or paste a link or #number" (`SearchField`, its accessible name the same) MUST stand in the header
row while GitHub can be asked or the view has rows.

#### Scenario: The field of the view
- **WHEN** GitHub can be asked or the view has rows
- **THEN** the field "Filter, or paste a link or #number" (`SearchField`, its accessible name the same) stands in the
  header row
  [@test] tests/e2e/reviews/reviews.spec.ts (`should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches`)

### Requirement: Search: The keys of the field

`/` MUST focus the field from anywhere in the window and select its text, only while the view shows (a hidden view's
keys are off). X and Escape MUST clear it and its refusal, Escape in the empty field MUST move the focus to the heading,
↓ to the first row.

#### Scenario: Slash focuses the field
- **GIVEN** the view shows
- **WHEN** the user presses `/` anywhere in the window
- **THEN** the field takes the focus and its text is selected
  [@test] tests/e2e/reviews/reviews.spec.ts (`should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches`)

#### Scenario: Slash while the view is hidden
- **GIVEN** the view is hidden
- **WHEN** the user presses `/`
- **THEN** nothing of the view happens: a hidden view's keys are off

#### Scenario: X and Escape clear the field
- **WHEN** the user presses X or Escape in a field with text
- **THEN** the field and its refusal are cleared
  [@test] tests/e2e/reviews/reviews.spec.ts (`should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches`)

#### Scenario: Escape in the empty field and the down arrow
- **WHEN** the user presses Escape in the empty field, or ↓
- **THEN** Escape moves the focus to the heading, ↓ to the first row
  [@test] tests/e2e/reviews/reviews.spec.ts (`should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches`)

### Requirement: Search: The text of the field is the window's, by repository

The text of the field MUST be the window's, by repository: a switch of the views keeps it, another repository starts
without one.

#### Scenario: A switch of the views and another repository
- **WHEN** the user switches the views and back, or selects another repository
- **THEN** a switch of the views keeps the text
- **AND** another repository starts without one

### Requirement: Search: A row matches by every word of what it shows

A row MUST match when every word (split at white space) is, ignoring case, in what it shows or its branches: the
title, `#123`, the author, the meta of how the user takes part, the head and the base branch, its labels; never in its
URL or its description, whose words every row has. A pasted link of a row of the view MUST show that row.

#### Scenario: The words of a row
- **WHEN** the user types words
- **THEN** a row matches when every word (split at white space) is, ignoring case, in its title, `#123`, its author,
  the meta of how the user takes part (the teams asked, the reviewers, "you approved", "requested by dave", "changes
  requested by alice"), its head or its base branch, or its labels on GitHub
- **AND** never in its URL or its description, whose words every row has
  [@test] src/presenters/reviews.test.ts (`matches a row by every word of what it shows and its branches, ignoring case, never by its URL`, `matches a request by name by who asked, and a returned pull request by who handed it back and its branch`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches`)

#### Scenario: A pasted link of a row
- **WHEN** the user pastes a link of a row of the view
- **THEN** that row shows
  [@test] src/presenters/reviews.test.ts (`finds the shown row a number or a pasted link names, never another number that starts the same`)

### Requirement: Search: Filtered, a section shows its matches

Filtered, a section MUST show its matches, its count "1 of 63", and one without a match MUST be left out, Waiting for
You too; Waiting for You MUST show every match, no "Show N More".

#### Scenario: The sections while the field filters
- **WHEN** the field filters
- **THEN** a section shows its matches and its count "1 of 63"
- **AND** a section without a match is left out, Waiting for You too
- **AND** Waiting for You shows every match, no "Show N More"
  [@test] tests/e2e/reviews/reviews.spec.ts (`should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches`)

### Requirement: Search: A collapsed section is collapsed again once the filter is cleared

A section the user collapsed MUST be collapsed again once the filter is cleared; while it filters, a section with a
match MUST show open.

#### Scenario: A collapsed section through a filter
- **GIVEN** a section the user collapsed
- **WHEN** the field filters and the section has a match, then the filter is cleared
- **THEN** it shows open while it filters, and is collapsed again once the filter is cleared
  [@test] tests/e2e/reviews/reviews.spec.ts (`should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches`)

### Requirement: Search: Enter on a number or a link opens or adds its pull request

On Enter, a number (`#123`, `123`) or a link of a row of the view MUST open it, as Enter on the row does, and of another
pull request MUST add it by its link ([Links](#links)); `#20` is #20, never #201.

#### Scenario: A number or a link of a row of the view
- **WHEN** the user presses Enter on a number (`#123`, `123`) or a link of a row of the view
- **THEN** the row opens, as Enter on the row does
- **AND** `#20` is #20, never #201
  [@test] src/presenters/reviews.test.ts (`names a pull request by its number or a link, and words by nothing`, `finds the shown row a number or a pasted link names, never another number that starts the same`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches`)

#### Scenario: A number of another pull request
- **WHEN** the user presses Enter on a number or a link of another pull request
- **THEN** it is added by its link ([Links](#links))
  [@test] tests/e2e/reviews/reviews.spec.ts (`should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches`)

### Requirement: Search: Enter on words opens their one match

On Enter, words MUST open their one match, or move the focus to the first of their matches; words that match nothing
MUST do nothing.

#### Scenario: Words with one match, several or none
- **WHEN** the user presses Enter on words
- **THEN** their one match opens, or the focus moves to the first of their matches
- **AND** words that match nothing do nothing
  [@test] src/presenters/reviews.test.ts (`names a pull request by its number or a link, and words by nothing`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches`)

### Requirement: Search: A section GitHub returned a part of searches GitHub with the words

The filter MUST see what GitHub returned: a section GitHub returned a part of MUST say "First 50 of 63 filtered" with
"Search on GitHub", its search with the typed words (none for a number or a link, which names one pull request).

#### Scenario: A filtered section that GitHub returned a part of
- **WHEN** the field filters a section that GitHub returned a part of
- **THEN** the section says "First 50 of 63 filtered" with "Search on GitHub"
- **AND** "Search on GitHub" is its search with the typed words, none for a number or a link, which names one pull
  request
  [@test] src/presenters/reviews.test.ts (`searches GitHub with the typed words where it returned only a part, and says what found nothing`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should search GitHub with the words typed where it returned only a part of a section`)

### Requirement: Search: A filter that leaves the view empty says so

A filter that leaves the view empty MUST say `No pull requests match “migrate”` with Clear Filter, Add Pull Request
(Enter) when the text names a pull request, and for words Search on GitHub (`involves:@me` with them) when GitHub
returned a part of a list.

#### Scenario: Nothing matches
- **WHEN** a filter leaves the view empty
- **THEN** the view says `No pull requests match “migrate”` with Clear Filter
- **AND** with Add Pull Request (Enter) when the text names a pull request
- **AND** for words, with Search on GitHub (`involves:@me` with them) when GitHub returned a part of a list
  [@test] src/presenters/reviews.test.ts (`searches GitHub with the typed words where it returned only a part, and says what found nothing`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches`)

### Requirement: States: A state of GitHub is the state of the view

A state of GitHub MUST be the state of the view, never a toast: not on GitHub, no token or a rejected one (with Sign
In), no access, SSO, a rate limit; nothing at all: "No open pull requests involve you in acme/monorepo".

#### Scenario: Off GitHub
- **WHEN** the pull requests of the repository are not on GitHub
- **THEN** the view says so in its state, never a toast
  [@test] tests/e2e/reviews/reviews.spec.ts (`should say off GitHub why there are no reviews`)

#### Scenario: No token, no access, SSO or a rate limit
- **WHEN** there is no token or a rejected one, no access, SSO or a rate limit
- **THEN** the view says it in its state, never a toast, with Sign In for no token or a rejected one

#### Scenario: Nothing at all
- **WHEN** no open pull request involves the user
- **THEN** the view says "No open pull requests involve you in acme/monorepo"

### Requirement: States: The view is busy while the first answer of the rest is Loading

While the first answer of the rest is `Loading`, the view MUST be busy, no skeleton.

#### Scenario: The first answer
- **WHEN** the first answer of the rest is `Loading`
- **THEN** the view is busy, with no skeleton

### Requirement: States: The menu of the view refreshes and opens GitHub

"⋯" of the view MUST hold Refresh (with when GitHub was asked) and Open on GitHub (`involves:@me`).

#### Scenario: The menu of the view
- **WHEN** the user opens "⋯" of the view
- **THEN** it holds Refresh, with when GitHub was asked, and Open on GitHub (`involves:@me`)

### Requirement: States: The menu of Your Turn shows all reviews

"⋯" of Your Turn MUST have Show All Reviews (⌘2).

#### Scenario: Show All Reviews from Your Turn
- **WHEN** the user chooses Show All Reviews (⌘2) in "⋯" of Your Turn
- **THEN** the Reviews view shows
  [@test] tests/e2e/reviews/reviews.spec.ts (`should show all reviews from the menu of Your Turn`)

## User Experience

- The header row of the view: the heading "Reviews" (the focus target of a switch), the GitHub repository muted, the
  field (the search icon, `/` at its end while empty, up to 24rem), its help (what each section holds and in which
  order, what the marks say, what the filter sees), then "⋯" at the end; a refusal of a link is a line under the row,
  which keeps its height. The sections are `CollapsibleCard` islands, open, their counts in the header; the rows are
  the 36 px rows of Your Turn.

## Testing / Local Run

- `pnpm e2e tests/e2e/reviews/reviews.spec.ts`; the fixture `setReviewRequests` takes `others` (the searches),
  `answered` and `linkable` (the pull requests a link can name, `closed` for a merged one).

## References

- [Review Requests](../review-requests/spec.md), [Main Views](../main-views/spec.md), [Command Palette](../command-palette/spec.md),
  [Rust as the Domain Authority](../../../../docs/design/rust-domain-authority.md).

## ADDED Requirements

### Requirement: Search: Words search the closed pull requests on GitHub

While the view shows and its field holds words (not a number or a link) of three characters or more, at most eight,
the session MUST search GitHub for the closed pull requests the user reviewed or opened whose titles have every word
(`closed_search_words`): `is:closed in:title` with `reviewed-by:@me -author:@me` and with `author:@me`, each word
quoted, the first 20 of each. It MUST search once the words stood 400 ms, one search at a time, an answer kept two
minutes; off GitHub, under a rate limit and offline the words fail without asking.

#### Scenario: Words that stood
- **GIVEN** the view shows
- **WHEN** the words of the field stood 400 ms
- **THEN** the session searches the closed pull requests by them, once, one search at a time
- **AND** a number, a link, fewer than three characters and a hidden view search nothing
  [@test] crates/domain-policy/src/reviews_test.rs (`the_closed_pull_requests_are_searched_by_three_characters_or_more_never_by_a_number_or_a_link`)
  [@test] crates/remote-facts/src/scheduler_test.rs (`the_closed_pull_requests_are_searched_by_the_words_once_they_stood_one_at_a_time_and_kept_two_minutes`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should search the closed pull requests by the words typed and open one read only, never by a number or too few characters`)

#### Scenario: The searches of the request
- **WHEN** the session searches the closed pull requests by words
- **THEN** it asks the two searches, `in:title`, each word quoted, the first 20 of each
- **AND** a token without `read:org` is asked again without the teams
  [@test] crates/remote-facts/src/reviews_test.rs (`searches_the_closed_pull_requests_by_the_words_of_their_titles`)
  [@test] crates/remote-facts/src/reviews_test.rs (`a_search_of_the_closed_pull_requests_without_read_org_asks_again_without_the_teams`)

#### Scenario: A rate limit
- **WHEN** GitHub answers a search with its rate limit
- **THEN** the reads of GitHub but the refresh button wait, and the next words fail without asking it
  [@test] crates/remote-facts/src/scheduler_test.rs (`a_rate_limit_of_a_search_of_the_closed_pull_requests_holds_back_github_and_fails_the_next_words_at_once`)

### Requirement: Search: The closed pull requests show last

What a search found for the words of the field MUST show last, "Closed and Merged", while the field holds them: one list,
the newest closed first, none an earlier section shows, GitHub's count, "First 20 of 57 · Search on GitHub" when GitHub
found more. Each row MUST be marked merged or closed (Patronus' `merged` label is merged) and say when. While the
session searches the view MUST be busy; a search that failed or found nothing shows no section.

#### Scenario: What a search found
- **WHEN** a search of the words of the field found closed pull requests
- **THEN** "Closed and Merged" shows them last, the newest closed first, none that an earlier section shows
- **AND** each row is marked merged or closed and says when
  [@test] crates/domain-policy/src/reviews_test.rs (`the_closed_pull_requests_show_the_newest_closed_first_without_those_a_section_shows`)
  [@test] crates/repository-model/src/review_view_test.rs (`the_closed_pull_requests_of_the_words_show_read_only_until_no_window_filters_by_them`)
  [@test] src/presenters/reviews.test.ts (`marks a merged or closed one by how it ended, with when, and the window says it`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should search the closed pull requests by the words typed and open one read only, never by a number or too few characters`)

#### Scenario: The field cleared
- **WHEN** the field no longer holds the words
- **THEN** the section goes
  [@test] crates/repository-model/src/review_view_test.rs (`the_closed_pull_requests_of_the_words_show_read_only_until_no_window_filters_by_them`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should search the closed pull requests by the words typed and open one read only, never by a number or too few characters`)

### Requirement: Search: Enter opens a closed match only once no open one matches

On Enter, words with no open match MUST open their one closed match, or move the focus to the first of them; while the
session searches, Enter on words without an open match MUST do nothing.

#### Scenario: Words with one closed match only
- **WHEN** the user presses Enter on words that match no open pull request and one closed one
- **THEN** the closed one opens in the review window
  [@test] tests/e2e/reviews/reviews.spec.ts (`should search the closed pull requests by the words typed and open one read only, never by a number or too few characters`)

### Requirement: The actions of a pull request: A merged or closed pull request is read only

Of a merged or closed pull request Approve, Request Changes and the Safe Push MUST be disabled ("Pull request #401 is
merged"), Review and Open Pull Request stay; its readiness and what its approval counts for MUST NOT be asked. Its
review window MUST say "This pull request is merged" (or closed) and show its changes from where it forked, the parent
of its first commit: the base of a merged one has them.

#### Scenario: The actions of a merged or closed one
- **WHEN** Rust decides the actions of a merged or closed pull request
- **THEN** Approve, Request Changes and the Safe Push are disabled for it, Review and Open Pull Request stay
- **AND** nothing asks Patronus of it, and its approval counts for nothing
  [@test] crates/domain-policy/src/actions_test.rs (`a_merged_or_closed_pull_request_is_read_only_its_decisions_and_its_safe_push_disabled_for_it`)
  [@test] crates/repository-model/src/review_view_test.rs (`the_closed_pull_requests_of_the_words_show_read_only_until_no_window_filters_by_them`)
  [@test] crates/repository-model/src/review_view_test.rs (`a_merged_link_is_remembered_for_its_review_window_and_joins_no_links`)

#### Scenario: Its review window
- **WHEN** the review window shows a merged pull request
- **THEN** it says so, Approve is disabled with the reason, and its key approves nothing
- **AND** its changes show from where it forked
  [@test] crates/remote-facts/src/reviews_test.rs (`searches_the_closed_pull_requests_by_the_words_of_their_titles`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should search the closed pull requests by the words typed and open one read only, never by a number or too few characters`)

## MODIFIED Requirements

### Requirement: Links: A link of another repository or a missing number is refused

A link of another repository and a number the repository lacks MUST be refused under the header (`invalid_arguments`,
`no_pull_request`), no toast, the focus kept in the field, read-only while Rust reads it; a merged or closed one MUST
open read only but is not kept: the session remembers it, so its review window fetches it.

#### Scenario: Another repository, or a number the repository lacks
- **WHEN** the user presses Enter on a link of another repository, or on a number the repository lacks
- **THEN** it is refused under the header (`invalid_arguments`, `no_pull_request`), no toast
- **AND** the focus stays in the field, read-only while Rust reads it
  [@test] crates/remote-facts/src/reviews_test.rs (`a_link_names_a_pull_request_of_the_repository`)
  [@test] crates/test-server/src/review_requests_test.rs (`a_link_joins_reviews_from_the_fake_github`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should add a pull request by its link from the field the palette focuses, keep it, and remove it from its menu`)

#### Scenario: A merged or closed pull request
- **WHEN** the user presses Enter on a link of a merged or closed pull request
- **THEN** it opens read only, but is not kept
- **AND** its review window fetches it
  [@test] crates/remote-facts/src/reviews_test.rs (`reads_a_merged_link_with_its_state`)
  [@test] crates/repository-model/src/review_view_test.rs (`a_merged_link_is_remembered_for_its_review_window_and_joins_no_links`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should open a merged pull request added by its link read only, keeping no link of it`)

### Requirement: Search: A filter that leaves the view empty says so

A filter that leaves the view empty MUST say `No pull requests match “migrate”` with Clear Filter, Add Pull Request
(Enter) when the text names a pull request, and for words Search on GitHub (`is:pr involves:@me` with them, open or
closed); for words the session searches the closed pull requests by, only once that search answered.

#### Scenario: Nothing matches
- **WHEN** a filter leaves the view empty
- **THEN** the view says `No pull requests match “migrate”` with Clear Filter
- **AND** with Add Pull Request (Enter) when the text names a pull request
- **AND** for words, with Search on GitHub (`is:pr involves:@me` with them), once the search of the closed ones answered
  [@test] src/presenters/reviews.test.ts (`searches GitHub with the typed words where it returned only a part, and says what found nothing`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches`)
  [@test] tests/e2e/reviews/reviews.spec.ts (`should search the closed pull requests by the words typed and open one read only, never by a number or too few characters`)

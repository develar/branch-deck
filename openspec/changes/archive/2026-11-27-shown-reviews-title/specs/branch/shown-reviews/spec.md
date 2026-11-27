## ADDED Requirements

### Requirement: Title: A shown review is titled `[post-merge review]` once, and the page shows it without

Every opening of a shown review MUST title it `[post-merge review] <the title of the branch>`, the prefix once, so that
a reviewer tells it in any list of GitHub. One opened before MUST get the prefix with the next record of its body, in
the same write. The page MUST show the title without the prefix, beside its badge.

#### Scenario: A new review
- **WHEN** auth, whose title is "Add the token", is marked for a review after landing
- **THEN** the draft opens titled "[post-merge review] Add the token", and its open writes no title
  [@test] crates/domain-policy/src/shown_test.rs (`the_title_of_a_shown_review_starts_with_its_prefix_once_and_the_page_shows_it_without`)
  [@test] crates/operations/src/shown_test.rs (`a_mark_opens_its_draft_and_its_open_says_where_the_branch_was_reviewed_before`)
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should mark auth for a review after landing from its menu, say it in its row and its Safe Push, and open the review with its reviewers once that Safe Push merged`)

#### Scenario: A review opened before the prefix
- **WHEN** the open review #12 of auth, titled "Add the token", is opened, extended by a fix, or asked of one more reviewer
- **THEN** the record of its body writes the title "[post-merge review] Add the token" in the same write, once; an open of
  a draft with the prefix records nothing for it
  [@test] crates/operations/src/shown_test.rs (`an_open_of_a_review_without_the_prefix_writes_it_once_and_a_prefixed_one_writes_no_title`, `the_extend_of_a_shown_review_against_origin_and_github`)
  [@test] crates/remote-facts/src/pull_requests_test.rs (`the_closed_shown_reviews_are_a_search_of_their_own_with_what_the_remote_has_of_their_branches`)
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should ask more reviewers with Request Review… of an open review after landing, and close it unapproved with Close Review… after its confirmation`)

#### Scenario: The page
- **WHEN** a review after landing titled "[post-merge review] IJPL-7 Sign in" is listed, the author's or one the user is
  asked to review
- **THEN** its row says "IJPL-7 Sign in" beside the badge, the issue key muted; a pull request into master keeps its
  title whole
  [@test] crates/remote-facts/src/review_requests_test.rs (`a_review_after_landing_is_read_without_the_prefix_of_its_title`)
  [@test] crates/remote-facts/src/pull_requests_test.rs (`the_shown_reviews_are_the_heads_of_the_review_namespace_with_the_reviewers_their_body_records`)
  [@test] tests/e2e/pull-requests/shown-reviews.spec.ts (`should tell a reviewer a review after landing by its badge, offer no Safe Push of it, and take the approval`, `should return the review to the author on changes requested, wait while a fix lands, then put the fix onto its head and ask again`)

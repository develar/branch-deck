---
name: Reviews
description: Requirements for the Reviews view of the main window: every open pull request of the selected repository the user takes part in on GitHub, beyond the requests by name of Your Turn (the requests of the user's teams, the pull requests the user reviewed, the user's own), each once, and a field that adds a pull request by its link when its author forgot to ask the user; read in the request of the review requests only while the view shows, its sections and the actions of each pull request decided in Rust.
style: plain-1
targets:
  - ../../crates/domain-types/src/review_request.rs
  - ../../crates/domain-types/src/session.rs
  - ../../crates/domain-types/src/actions.rs
  - ../../crates/domain-policy/src/reviews.rs
  - ../../crates/domain-policy/src/actions.rs
  - ../../crates/domain-policy/src/refresh.rs
  - ../../crates/remote-facts/src/reviews.rs
  - ../../crates/remote-facts/src/review_requests.rs
  - ../../crates/remote-facts/src/scheduler_review_requests.rs
  - ../../crates/remote-facts/src/service_sources.rs
  - ../../crates/repository-model/src/review_request_facts.rs
  - ../../crates/repository-model/src/review_view.rs
  - ../../crates/repository-model/src/runtime.rs
  - ../../crates/app-settings/src/linked_pull_requests.rs
  - ../../crates/operations/src/review.rs
  - ../../crates/github/src/fake_repository.rs
  - ../../crates/test-server/src/review_fixtures.rs
  - ../../crates/test-server/src/session_routes.rs
  - ../../src-tauri/src/commands/review_requests.rs
  - ../../src-tauri/src/commands/session.rs
  - ../../src/components/reviews/ReviewsView.tsx
  - ../../src/components/repository/ReviewRequestRow.tsx
  - ../../src/components/repository/YourTurnCard.tsx
  - ../../src/hooks/useReturnedToYou.ts
  - ../../src/presenters/reviews.ts
  - ../../src/presenters/actions.ts
  - ../../src/stores/reviews.ts
  - ../../src/model/demand.ts
  - ../../src/model/patch.ts
  - ../../src/lib/review-request-actions.ts
  - ../../src/lib/pull-request-review.ts
  - ../../src/lib/pull-request-review-window.ts
  - ../../src/components/review/ReviewHeader.tsx
  - ../../src/components/review/PullRequestReview.tsx
  - ../../src/pages/PullRequestReviewPage.tsx
  - ../../src/lib/command-palette.ts
---

# Reviews

Status: Implemented
Date: 2026-10-27

## Summary

Your Turn ([Review Requests](review-requests.spec.md)) tells what waits for the user by name. The rest of what the
user takes part in on GitHub was told only by github.com/pulls: the requests of the user's teams (CODEOWNERS), the
pull requests the user reviewed, the user's own pull requests and their reviewers. And sometimes the author forgets to
ask the user: the user has only a link. Reviews is the second view of the main window (Branches · Reviews · Landing,
[Main Views](main-views.spec.md)): every open pull request of the selected repository the user takes part in, Your
Turn's rows first, then the sections, each pull request once, and a field that adds any pull request of the repository
by its link and opens it in the review window. Your Turn stays above the branches as it is.

## Goals

- See in one place what the user takes part in on GitHub, beyond what waits for the user by name.
- Review a pull request nobody asked the user for, from its link, in the app.
- Ask GitHub for it only while the user looks.

## Non-goals

- Other repositories than the selected one, and pull requests the user only commented on or is mentioned in.
- News, toasts, OS notifications and the Dock badge of the sections: they stay Your Turn's.
- Knowing which requested team the user is in: Reviews names the teams GitHub requests.
- Following a link across the worktrees of other GitHub repositories: a link names a pull request of this one.
- A `bd` command for Reviews yet: the rule is pure and `bd` can call it.

## Requirements

### Reading

- While a window shows the Reviews view (`Demand::reviews`, held while it shows, focused or not), every list of the
  review requests of the session reads the rest of Reviews in the same GraphQL request: the searches `team`
  (`review-requested:@me`), `reviewed` (`reviewed-by:@me -author:@me`), `authored` (`author:@me`), each open and not
  archived, drafts only of the user's own, 50 each, and the pull requests the user added by link by their numbers
  (`pullRequest(number:)` aliases). A token without `read:org` is asked again once without the team slugs, as the
  pull requests are. While no window shows it, the request is the one of Your Turn, and nothing more is read.
  [@test] ../../crates/remote-facts/src/reviews_test.rs (`reads_the_rest_of_reviews_in_the_request_of_the_review_requests`, `without_the_reviews_view_the_request_is_the_one_of_your_turn`, `a_token_without_read_org_is_asked_again_without_the_teams`)
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`)
- The view that shows reads the rest at once when it was never read or is older than two minutes (at least a minute
  after it was asked); off GitHub and under a rate limit not; a list in flight that doesn't read it asks once more
  after it. While it shows, the poll, a focus, a sync, Refresh and a review read it with the requests.
  [@test] ../../crates/domain-policy/src/refresh_test.rs (`the_reviews_view_lists_the_rest_of_reviews_when_it_is_older_than_two_minutes_or_never_read`)
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`the_reviews_view_reads_the_rest_of_reviews_with_the_review_requests_while_it_shows`)
- The rest is a part of the review requests fact of the session: `Unknown` until a window shows the view, `Loading`
  until a list brings it, then as the list stands (`Stale` with the reason through a failure, gone with no token or a
  rejected one); a list without it keeps it. The requests by name the user answered with a comment come with each
  list (Your Turn holds them back).
  [@test] ../../crates/repository-model/src/review_request_facts_test.rs (`the_rest_of_reviews_stands_by_its_listing_and_stays_through_one_without_it`)
  [@test] ../../crates/remote-facts/src/review_requests_test.rs (`a_request_the_user_answered_with_a_comment_is_not_their_turn`)

### Sections

- The view derives the sections (`domain_policy::reviews::sections`, `RepositoryView.reviews`) in their order: Added
  by Link (the newest link first), Requested from Your Teams (waiting the longest first, since the pull request was
  opened), Reviewed by You (those with commits since the user's review first, then the latest update; a request the
  user answered with a comment among them, "you replied"), Your Pull Requests (the latest update first). Your Turn's
  rows (the requests by name and the returned own pull requests) lead it as "Waiting for You". A pull request shows
  once, in the first section that has it; an empty section is left out; a section's count is GitHub's less the pull
  requests an earlier section has.
  [@test] ../../crates/domain-policy/src/reviews_test.rs (`each_pull_request_shows_once_in_the_first_section_after_your_turn_the_empty_ones_left_out`, `reviewed_by_the_user_shows_new_commits_first_and_the_requests_answered_with_a_comment`, `the_own_pull_requests_name_their_branch_and_leave_the_returned_ones_to_your_turn`)
  [@test] ../../crates/repository-model/src/review_view_test.rs (`the_view_has_the_sections_of_reviews_and_the_actions_of_each_pull_request_to_review`)
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`)
- A row is the row of a request ([Review Requests](review-requests.spec.md#card)) with the meta and the age of how the
  user takes part: "#201 · carol · requested from ide-platform" "waiting 2 days"; "you approved 11111111" "new
  commits since your review"; "#305 · carol · not reviewed" "updated 3 hours ago"; of an own pull request its reviewers
  ("approved by alice · waiting for bob"). The ages are muted: only Your Turn's say "your turn".
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`)
- A click and Enter review the pull request in the review window, D its changes, ⌘-click and O open it on GitHub,
  ↑/↓ move between the rows of a section; the menu is the actions of the pull request. An own pull request of a branch
  of the table shows the branch row on a click and Enter, as Your Turn's returned rows do.
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should review a pull request of a team in the window of Reviews and approve it there`, `should show the branch of an own pull request of the table`)
- With more than GitHub returns, a section says "50 of 63" with "Open All on GitHub", its search.

### The actions of a pull request

- Rust decides the actions of every pull request to review, Your Turn's and Reviews' and those a review window shows
  (`domain_policy::actions::review_actions`, `RepositoryView.actions` of `pull:<number>`): Review, Approve (done once
  the user approved its head: the review the app submitted, else GitHub's latest review of the user), Approve and Safe
  Push where Patronus merges (disabled while the target is not known, for a baseline that is no branch of origin, and
  for another base than the target), Request Changes, Open Pull Request, Show Branch of an own one of the table,
  Remove from Reviews of a link. GitHub takes no review of the author: the decisions of an own pull request are
  disabled with "Your own pull request". The target is what the sync read of the baseline (`TableHeader.targetBranch`),
  no git of its own. The page words them (`src/presenters/actions.ts`) and lays its window's own state over them (the
  commit it shows, a write that runs).
  [@test] ../../crates/domain-policy/src/actions_test.rs (`a_pull_request_to_review_offers_the_review_the_decisions_and_its_safe_push_into_the_target`, `an_approval_of_the_head_is_done_and_the_safe_push_waits_for_the_target_patronus_pushes_into`)
  [@test] ../../crates/repository-model/src/review_request_facts_test.rs (`a_submitted_review_and_a_shown_pull_request_outlive_the_listing`)
  [@test] ../../src/lib/review-request-actions.test.ts (`offers Approve and Safe Push where Rust offers it, disabled with its reason, the Safe Push alone once approved`, `disables the writes of the review until the commit the user saw is there, an approval of the same commit, and an own pull request`, `offers Show Branch of an own pull request and Remove from Reviews of a link where Rust offers them`)
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`)
- The review window opened from Reviews walks its rows ("Reviews › 2 of 4"), says how the user takes part ("ide-platform
  asked for a review of carol's pull request", "Added by link: …") and, of a pull request the user reviewed, offers
  "Since Your Review". A write is the `SubmitReview` of any pull request.
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should review a pull request of a team in the window of Reviews and approve it there`)
  [@test] ../../src/lib/pull-request-review.test.ts (`keeps its review for a plain open of the request it shows, and starts over for another request or an intent`)

### Links

- The field "Add pull request by link or #number" takes a URL of a pull request (`/files` and an anchor after it too),
  `owner/name#123`, `#123` or `123`; Enter asks Rust (`link_pull_request`), which reads it from GitHub, keeps an open
  one and reads Reviews again; the review window opens on it. A link of another repository and a number the repository
  lacks are refused under the field (`invalid_arguments`, `no_pull_request`), no toast; a merged or closed one opens but
  is not kept. The palette's "Add Pull Request by Link…" shows the view with the focus in the field.
  [@test] ../../crates/remote-facts/src/reviews_test.rs (`a_link_names_a_pull_request_of_the_repository`, `reads_a_link_the_user_adds`)
  [@test] ../../crates/test-server/src/review_requests_test.rs (`a_link_joins_reviews_from_the_fake_github`)
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should add a pull request by its link from the field the palette focuses, keep it, and remove it from its menu`)
- The links are Rust's: `linkedPullRequests` of `repository-state.json`, by GitHub repository (`owner/name`, so every
  worktree of it has them), the newest first, at most 50, written only through `SettingsService`, read by no window. A
  link GitHub answers merged, closed or missing goes; Remove from Reviews (`unlink_pull_request`) removes one; a link
  another section lists shows there and stays a link.
  [@test] ../../crates/app-settings/src/linked_pull_requests_test.rs (`keeps_the_links_of_each_repository_newest_first_in_the_repository_state`, `a_repository_keeps_the_newest_links`)
  [@test] ../../crates/domain-policy/src/reviews_test.rs (`a_merged_closed_or_missing_link_goes`)
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should add a pull request by its link from the field the palette focuses, keep it, and remove it from its menu`)

### States

- A state of GitHub is the state of the view, never a toast: not on GitHub, no token or a rejected one (with Sign In),
  no access, SSO, a rate limit; nothing at all: "No open pull requests involve you in acme/monorepo". While the first
  answer of the rest is `Loading`, the view is busy, no skeleton.
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should say off GitHub why there are no reviews`)
- "⋯" of the view: Refresh (with when GitHub was asked) and Open on GitHub (`involves:@me`). "⋯" of Your Turn has
  Show All Reviews (⌘2).
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should show all reviews from the menu of Your Turn`)

## User Experience

- The header row of the view: the heading "Reviews" (the focus target of a switch), the GitHub repository muted, the
  field, then "⋯" at the end. The sections are `CollapsibleCard` islands, open, their counts in the header; the rows
  are the 36 px rows of Your Turn.

## Testing / Local Run

- `pnpm e2e tests/e2e/reviews/reviews.spec.ts`; the fixture `setReviewRequests` takes `others` (the searches),
  `answered` and `linkable` (the pull requests a link can name, `closed` for a merged one).

## References

- [Review Requests](review-requests.spec.md), [Main Views](main-views.spec.md), [Command Palette](command-palette.spec.md),
  [Rust as the Domain Authority](../../docs/design/rust-domain-authority.md).

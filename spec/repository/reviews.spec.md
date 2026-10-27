---
name: Reviews
description: Requirements for the Reviews view of the main window: every open pull request of the selected repository the user takes part in on GitHub, beyond the requests by name of Your Turn (the requests of the user's teams, the pull requests the user reviewed, the user's own), each once, and one field that filters every section by the words typed and adds a pull request by its link when its author forgot to ask the user; read in the request of the review requests only while the view shows, its sections and the actions of each pull request decided in Rust.
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
  - ../../src/components/reviews/FilteredOnGitHub.tsx
  - ../../src/components/reviews/ReviewEntryRow.tsx
  - ../../crates/domain-policy/src/landing_node.rs
  - ../../src/components/shared/SearchField.tsx
  - ../../src/hooks/useSlashFocus.ts
  - ../../src/lib/text-search.ts
  - ../../src/lib/review-requests.ts
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
  opened), Reviewed by You (the user's moves first, then those with commits since the user's review, then the latest
  update; a request the user answered with a comment among them, "you replied"), Your Pull Requests (the user's moves
  first, then the latest update). Your Turn's
  rows (the requests by name and the returned own pull requests) lead it as "Waiting for You". A pull request shows
  once, in the first section that has it; an empty section is left out; a section's count is GitHub's less the pull
  requests an earlier section has; while the field filters, the count of the matches of it ([Search](#search)).
  [@test] ../../crates/domain-policy/src/reviews_test.rs (`each_pull_request_shows_once_in_the_first_section_after_your_turn_the_empty_ones_left_out`, `reviewed_by_the_user_shows_new_commits_first_and_the_requests_answered_with_a_comment`, `the_own_pull_requests_name_their_branch_and_leave_the_returned_ones_to_your_turn`)
  [@test] ../../crates/repository-model/src/review_view_test.rs (`the_view_has_the_sections_of_reviews_and_the_actions_of_each_pull_request_to_review`)
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`)
- Each section names the pull requests whose move is the user's (`ReviewSection.moves`, `domain_policy::reviews::is_users_move`):
  a link the user hasn't reviewed, or not since its head moved; a pull request the user reviewed whose head moved
  since, unless GitHub still counts the user's approval (`reviewDecision` approved); an own pull request whose branch
  of the table is the user's move (its node `ready`, `yourStep` or `needsFix`, `landing_node::is_users_move`), or,
  of no branch, with changes requested. A team's request is nobody's move in particular. A section shows its moves and
  three more rows, then "Show N More" (Show Less), as Your Turn shows its rows (`pickVisibleEntries`); filtered, every
  match.
  [@test] ../../crates/domain-policy/src/reviews_test.rs (`the_users_moves_are_a_link_to_review_a_review_its_commits_outdated_and_an_own_pull_request_to_land_or_fix`)
  [@test] ../../crates/domain-policy/src/landing_node_test.rs (`a_filled_node_is_the_users_move`)
  [@test] ../../src/presenters/reviews.test.ts (`shows the user's moves and three more before Show N More`)
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should mark the user's moves alone, keep the other rows behind Show N More, and give the title the room before the meta`)
- A row is the row of a request ([Review Requests](review-requests.spec.md#card)) by how the user takes part, so that
  only the user's moves stand out:
  - its mark: the filled `primary` dot of the user's move ("Your move: new commits since your review"); else the
    user's review muted, `CheckIcon` approved (of an own pull request, GitHub's approval), `MessageSquareXIcon` changes
    requested, `MessageSquareIcon` commented or replied; nothing for a team request; of an own pull request of a
    branch of the table the node of the branch, its hover the branch row's;
  - its meta in short, what the mark doesn't say: "#201 · carol · requested from ide-platform", "#202 · carol",
    "#202 · carol · you replied"; of an own pull request where it stands ("changes requested by alice", "waiting for
    bob", "review required"), of one of a branch of the table the first line of its node ("#1239 · Lands through
    #1239");
  - the size ("2 files · +12 −3") only of a move; of the rest it is in the tooltip of the meta, with what the meta
    leaves out ("you approved 11111111", "approved by alice · waiting for bob");
  - its age: "waiting 2 days", "new commits since your review", "updated 3 hours ago", muted, `text-foreground` of a
    move.
  [@test] ../../src/presenters/reviews.test.ts (`says in its meta only what its mark doesn't, the rest in the hint`, `marks the user's move alone, and the user's review muted`)
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should show every pull request the user takes part in once, by section, and read them only while the view shows`, `should show the node of the branch of an own pull request of the table, where the branch stands, and the branch on a click`)
- A click and Enter review the pull request in the review window, D its changes, ⌘-click and O open it on GitHub,
  ↑/↓ move between the rows of a section; the menu is the actions of the pull request. An own pull request of a branch
  of the table shows the branch row on a click and Enter, as Your Turn's returned rows do.
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should review a pull request of a team in the window of Reviews and approve it there`, `should show the node of the branch of an own pull request of the table, where the branch stands, and the branch on a click`)
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

- The field of the view ([Search](#search)) takes a link too: a URL of a pull request (`/files` and an anchor after it
  too), `owner/name#123`, `#123` or `123`. Enter on one that no row of the view is asks Rust (`link_pull_request`),
  which reads it from GitHub, keeps an open one and reads Reviews again; the field empties and the review window opens
  on it. A link of another repository and a number the repository lacks are refused under the header
  (`invalid_arguments`, `no_pull_request`), no toast, the focus kept in the field, read-only while Rust reads it; a
  merged or closed one opens but is not kept. The palette's "Add Pull Request by Link…" shows the view with the focus in
  the field and its text selected, so a paste replaces a filter.
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

### Search

- The field "Filter, or paste a link or #number" (`SearchField`, its accessible name the same) stands in the header
  row while GitHub can be asked or the view has rows; `/` focuses it from anywhere in the window and selects its text
  (only while the view shows: a hidden view's keys are off), X and Escape clear it and its refusal, Escape in the empty
  field moves the focus to the heading, ↓ to the first row. Its text is the window's, by repository: a switch of the
  views keeps it, another repository starts without one.
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches`)
- A row matches when every word (split at white space) is, ignoring case, in what it shows or its branches: the title,
  `#123`, the author, the meta of how the user takes part (the teams asked, the reviewers, "you approved", "requested
  by dave", "changes requested by alice"), the head and the base branch; never in its URL or its description, whose
  words every row has. A pasted link of a row of the view shows that row. Filtered, a section shows its matches, its
  count "1 of 63", and one without a match is left out, Waiting for You too; Waiting for You shows every match, no
  "Show N More". A section the user collapsed is collapsed again once the filter is cleared; while it filters, a
  section with a match shows open.
  [@test] ../../src/presenters/reviews.test.ts (`matches a row by every word of what it shows and its branches, ignoring case, never by its URL`, `matches a request by name by who asked, and a returned pull request by who handed it back and its branch`)
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches`)
- Enter: a number (`#123`, `123`) or a link of a row of the view opens it, as Enter on the row does, and of another
  pull request adds it by its link ([Links](#links)); `#20` is #20, never #201. Words open their one match, or move the
  focus to the first of their matches; words that match nothing do nothing.
  [@test] ../../src/presenters/reviews.test.ts (`names a pull request by its number or a link, and words by nothing`, `finds the shown row a number or a pasted link names, never another number that starts the same`)
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches`)
- The filter sees what GitHub returned: a section GitHub returned a part of says "First 50 of 63 filtered" with
  "Search on GitHub", its search with the typed words (none for a number or a link, which names one pull request). A
  filter that leaves the view empty says `No pull requests match “migrate”` with Clear Filter, Add Pull Request (Enter)
  when the text names a pull request, and for words Search on GitHub (`involves:@me` with them) when GitHub returned a
  part of a list.
  [@test] ../../src/presenters/reviews.test.ts (`searches GitHub with the typed words where it returned only a part, and says what found nothing`)
  [@test] ../../tests/e2e/reviews/reviews.spec.ts (`should search GitHub with the words typed where it returned only a part of a section`, `should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches`)

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
  field (the search icon, `/` at its end while empty, up to 24rem), its help (what each section holds and in which
  order, what the marks say, what the filter sees), then "⋯" at the end; a refusal of a link is a line under the row,
  which keeps its height. The sections are `CollapsibleCard` islands, open, their counts in the header; the rows are
  the 36 px rows of Your Turn.

## Testing / Local Run

- `pnpm e2e tests/e2e/reviews/reviews.spec.ts`; the fixture `setReviewRequests` takes `others` (the searches),
  `answered` and `linkable` (the pull requests a link can name, `closed` for a merged one).

## References

- [Review Requests](review-requests.spec.md), [Main Views](main-views.spec.md), [Command Palette](command-palette.spec.md),
  [Rust as the Domain Authority](../../docs/design/rust-domain-authority.md).

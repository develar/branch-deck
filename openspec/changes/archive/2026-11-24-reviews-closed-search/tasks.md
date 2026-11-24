# Tasks

- [x] `ReviewRequest.state` and `closed_at`, the base of a closed one where it forked (`request_of`), `FRAME_SCHEMA` 22
- [x] `Demand::reviews_searches`, `closed_search_words`, `closed_results`, `CLOSED_REVIEWS_SEARCH_DELAY`, `CLOSED_REVIEWS_KEPT`
- [x] `search_closed_reviews` and the `closed_reviews` read of the fact sources, its schedule in `scheduler_review_requests.rs`
- [x] `ReviewsView::closed_searches`, the closed entries among the pulls to review, `ActionBlocker::PullRequestClosed`
- [x] A merged or closed link remembered (`ReviewRequestsFact::linked`)
- [x] The page: `searchClosedReviews`, "Closed and Merged", the marks and the age, the window's note, Enter, the empty filter
- [x] The fixture `closed` of the E2E server and its log `closedSearches`

# Tasks

- [x] The search of the closed shown reviews in the listing, once an hour (`remote_facts::pull_requests`, `domain_policy::refresh::closed_shown_due`, the scheduler), and the fake GitHub of it
- [x] The rule of a review closed elsewhere (`domain_policy::shown::closed_elsewhere`, `closed_remains`) and its news, told once by the session
- [x] The delete at the tips of its pull request (`sync_core::review_head::delete_review_at`), `Intent::DeleteClosedShownReview` and its plan, the page's handler of the news
- [x] `brd close-review` of a review closed on GitHub; an open of a review without its base pushes it again

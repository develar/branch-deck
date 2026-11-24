# Tasks

- [x] The rule of a shown review (`domain_policy::shown`): its state by the reviewers it asked, whom it asks, its news
- [x] The start with `--request-review` (`domain_policy::land`, `operations::land_execute`: `PushReview`, `CreateShownReview`)
- [x] The branches of the review (`sync_core::review_head`) and their exclusion from the inputs of a sync
- [x] The listing of the shown reviews (`remote_facts::pull_requests`, the `shown` search)
- [x] The open and the close (`operations::shown`), the news of the session, the page's handler (`src/lib/shown-news.ts`)
- [x] The CodeOwners client (`crates/codeowners/`) and the reviewers of `brd` (`show_reviewers`)
- [x] `brd safe-push --request-review`, `brd request-review --after-landing`, `brd close-review`
- [x] The reviewer: `ActionBlocker::Shown`, `ApprovalLanding::Shown`; the row: `RowFact::Shown`; Your Turn: Returned to you

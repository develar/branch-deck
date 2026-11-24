# Tasks

- [x] The fixes a review has not taken (`domain_policy::shown::pending_fixes`), the state while a fix lands (`state_now`), the news of a fix that landed and of a draft landed by content (`shown_news`), the record of the fixes in the body
- [x] The extend of the head (`sync_core::review_head::extend_review`): replay onto the head the remote has, signing, a leased push, a fetch of a head the repository lacks
- [x] The plan and the run of the extend (`operations::shown::extend_plan`), the open with added reviewers and by content, the record and the comment of GitHub (`remote_facts::pull_request_writes`)
- [x] The session: the states settled with the robots, the news of a fix, a failed run told again (`Sessions::retell_shown`)
- [x] `brd request-review` of a branch with a shown review, `brd robot` `extends`, the page's handler of the news, the words of the row
- [x] The process budgets of the start, the open, the extend and the close at the level of `brd`; the CodeOwners lookup of the real shape

# Tasks

- [x] `git_ops::fix_replay`: a fix replayed commit by commit where it conflicts, the commit that conflicts taking its
  files from the commit that landed it, the temporary refs deleted in one transaction
- [x] `sync_core::review_head::extend_review` on it, with a fetch of the target when a fix doesn't apply
- [x] The comments of `operations::shown` per fix, the output of `brd request-review`
- [x] The tests of the review head, the process budget, the run against origin and GitHub, the E2E of both comments
- [x] The spec, Git Operations ("Review head"), Landing ("Show"), the help of `brd request-review` and Agent CLI

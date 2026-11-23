# Tasks

- [x] `land`: a check under the lock whose moves meet an index lock of another process waits for it outside the lock
      and checks under the lock again (`PickStep::WaitForIndex`)
- [x] Test: a pick whose target moved while it waited for the lock meets an index lock under it; another writer takes
      the repository lock while the pick waits for git's, and the pick lands
- [x] The scenario of the requirement, "Pick" of docs/design/git-operations.md

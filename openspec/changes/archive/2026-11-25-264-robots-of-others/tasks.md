# Tasks

- [x] `PatronusClient::list_robots` by `RobotListFilter` (`owner=me`, `sourceBranch`, everyone) with `me`; the owner's name
- [x] `PatronusService::robots` by `RobotsAsk`: the lists of a full and of a short incremental window, kept per list
- [x] `PatronusRobot::started_by`, `HoldView::started_by`, `SpanView::started_by`; a poll keeps it
- [x] `BranchTable::robot_sources` and `robot_sources_of`; `TableCommitted` carries them to the scheduler
- [x] The check of a push, `FactNeed::Robots(RobotsOf)`, `brd robot`, `brd status` and its kept sources
- [x] Reconcile and comment starts take only the user's robots
- [x] The words in Rust and on the page; the tests, the E2E test, the docs

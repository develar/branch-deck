# Tasks

- [x] `RunDir::forget`, `cut_log` and `Runs::remove_forgotten_past` (`KEPT_LOG`, `KEPT_FORGOTTEN`), with unit tests
- [x] `Request::Forget` keeps the run, `Request::Fetch` of a forgotten run refuses, the hourly cleanup bounds them
- [x] `RunInfo::ended_ms` and `RunInfo::forgotten`, optional within the schema
- [x] `host_client::phases::run_line` for `brd-host runs`, with its unit test; the process tests of the agent and of
  brd-host follow, fetch and forget a forgotten run

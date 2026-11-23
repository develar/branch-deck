# Tasks

- [x] `crates/backend-host/src/page.rs`: the pages of the app over the in-process connection, with its tests
  (`page_test.rs`)
- [x] The Tauri shell answers every command through the pages (`src-tauri/src/api.rs`), the host built before the first
  window, the life of the windows as `pageLoaded` and `windowClosed`
- [x] The page mock's routes through one host per test repository (`crates/test-server/src/invoke.rs`,
  `TestRepository::host`)
- [x] `claim_news` and `NewsClaim`; the OS notification of a news claims it first (`src/lib/os-notification.ts`)
- [x] A panicked command answers `-32603 panic`; a join of a replaced reservation leaves the session
- [x] The docs: `docs/design/backend-protocol.md` ("The pages of the app"), the code map, the e2e design

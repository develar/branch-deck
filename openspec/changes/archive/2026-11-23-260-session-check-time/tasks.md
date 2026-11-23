# Tasks

- [x] The session keeps the time of the check of a focus that found nothing changed in its view, and a run that ends
  takes it back (`TableState::inputs_checked`, `RepositoryView::inputs_checked_at`, `TableChanges::inputs_checked_at`)
- [x] The host names the clock of the time (`SessionHost::unix_now`): the system's, the page's in the E2E server
- [x] The page and the IDE mirror read it from the view; `recordSync` no longer marks the check
- [x] Tests: every window gets the check and its end (`session_test.rs`), the page shows the view's (`store.test.ts`)

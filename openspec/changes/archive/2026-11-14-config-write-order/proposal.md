# Proposal

## Why

The review of 6987f5d1 found two holes of the writes of Settings. The close of the window waited for a write that ran
before it sent the edit queued behind it, and a write the backend never answered kept that edit unsent; a save that
failed or got no answer at the close was told in the toast of a window that was gone. And the git config writes of
the app were ordered only by a mutex per file held across the git process, `set_repository_config` itself unordered.

## What Changes

- Each git config file has its one writer (`app_api::app::config_writer`): a task per file (the common git directory
  of the repository, or the global config) that runs the writes of Settings and of the companions one after the other;
  the mutex of `run_config_write` is gone, and a `config.lock` of another process fails after about a second with an
  error that says so.
- Each write of a page of Settings names its page, an id drawn at its load, and its number (`order`,
  `nextWriteOrder`, one count per page) in `set_repository_config` and `set_setting`; the writer applies it only when
  newer than the last write of its key of that page (`sync_utils::page_writes`), else answers null, which the page
  takes without an error. The page names itself, so a window loaded or opened again counts anew without a false
  superseded, whatever its host knew of the load.
- The close of Settings sends the edit of a field at once beside the write that runs (`FieldWrites` `now`; the settings
  store merges the queued changes with the unanswered patch), and the main window tells a save that failed or did not
  answer within 3 s, by the title of the write and with Open Settings on the field (`settings_not_saved`).
- Capability `repository/settings`: the close requirement, one writer per config file, the order of the writes.

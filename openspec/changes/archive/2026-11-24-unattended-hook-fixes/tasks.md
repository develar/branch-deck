# Tasks

- [x] `replace_if_unchanged` of `sync_utils::atomic_file`, the edit of the settings file reads and merges again when it changed
- [x] The settings file written where its symlink points
- [x] The hook of the exec form with `onFailure: continue`; the shell form read as `stale`
- [x] `set_unattended` saves before it answers; the app runs it off the async thread
- [x] `brd install` installs the rest past a settings file that cannot hold the hook (`hook_error`), the hook state `invalid` and its words

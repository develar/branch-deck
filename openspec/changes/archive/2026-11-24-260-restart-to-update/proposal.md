# Proposal

## Why

A stale backend with a window open (the app updated in place while it runs) stays when a bridge of the new build asks
it, and the protocol says it offers Restart to Update; it only logged that. And the restart of `restart_app` ran on
the main thread, where tauri restarts with the arguments of the start, not with those of the windows as they are
(`AppLife::restart`).

## What Changes

- The rule `app_api::app::life::BuildReplaced`: the hook of the stale backend tells the main window of the app
  `build-replaced`, once per process; the page words it as one event toast, "A newer Branch Deck is installed", whose
  Restart to Update runs `restart_app`.
- `RestartHost::restart` answers once the restart is under way; the app starts it off the main thread
  (`AppLife::restart`), so every restart takes the arguments of its windows.
- `POST /repositories/{id}/stale-backend` of the test server: a stale backend of the repository with a window open
  refuses a bridge of a new build.

Capabilities: `repository/backend-protocol`.

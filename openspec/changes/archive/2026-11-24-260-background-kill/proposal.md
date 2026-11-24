# Proposal

## Why

On macOS, Background Task Management quits an app that runs with no window about 30 s after it went windowless when
Allow in the Background is off for it, an IDE connected or not. The bridge then lost its backend, ended with 1, the IDE
started it again, and it started the app again: a loop that the user saw only as a dead Branch Deck in the IDE.

## What Changes

- The app records an end it did not ask for while it had no window (`backend-killed.json` in its data directory), from
  the exit of AppKit's `terminate:` and from a SIGTERM, which now quits it as Quit does.
- A bridge of the same build does not start the app for a day after such a kill: it serves its IDE itself and still
  joins an app that runs.
- The first bridge that serves for a kill tells the IDE once, with Open Login Items (the tool `open_login_items`).

Capabilities: `repository/ide-embedding`.

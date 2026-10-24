# Checking the Real App (macOS, Peekaboo)

**Status:** Active  
**Updated:** 2026-10-24  
**Tags:** [macos, release, screenshots, timing]

## Summary

What a release build shows on screen (a window that opens late, a frame of "Loading…") is checked in the real app,
not only in E2E: `pnpm install-app` (5–7 min, quits and replaces /Applications/BranchDeck.app), then drive it with
the `peekaboo` CLI. An agent never does that: it checks a copy of its own
([An agent's check copy](#an-agents-check-copy)).
- **Through Peekaboo.app**: its Bridge holds Screen Recording, Accessibility and Event Synthesizing. Keep Peekaboo.app
  open, never pass `--no-remote`, never delete or replace the CLI (other agents call it), kill nothing. Peekaboo may
  pick its on-demand daemon (`daemon-*.sock`), which has no permissions ("Operation hotkey is not allowed"): pass
  `--bridge-socket "$HOME/Library/Application Support/Peekaboo/bridge.sock"` on every call; if the socket is missing,
  ask the user to open Peekaboo.app. From an agent shell `screencapture` and `osascript` keystrokes are refused.
- **Windows**: `peekaboo window list --pid <pid> --json` gives the window ids (a webview window has several rows);
  `peekaboo window focus --window-id <id>`, a screenshot with `peekaboo see --window-id <id> --no-elements --path x.png`.
- **Input**: `peekaboo move --at x,y --global --foreground` (a move needs `--foreground` as consent), `peekaboo click
  --at x,y --global --foreground`, `peekaboo press d --foreground`.
- **Frames of an open**: start `peekaboo capture live --mode area --region x,y,w,h --duration 2500 --active-fps 15
  --threshold 0 --path <dir>` in the background, then press; `<dir>/metadata.json` lists the frames with their time
  and change, so a blank or "Loading" frame before the content shows.
- **Timings**: every sub-window open logs its phases in `~/Library/Logs/org.develar.branch-deck/BranchDeck.log`
  ("commit-review open 258 ms: loaded 91, files 187, diffs 187, painted 235"), the backend the source of
  each patch (memory, disk, git) at debug level. The main window logs its start from its build ("main open 812 ms:
  loaded 240, shell 300"): the time of its first rows, not of its show. The backend logs what showed it ("the page
  settled its content, showing the window", or "the page settled no content within its budget, showing the window" 1 s
  after the page drew its shell; a warning when a page loaded and drew no shell, or never loaded) and how long the
  webview took to present the page before the window showed (`presented_ms`), so the frames of a start begin with the
  rows or with the title row and the loading table, never with a bare canvas. The log stamps whole seconds; the milliseconds of a start are in the unified log of WebKit:
  `/usr/bin/log show --start '<local time>' --end '<local time>' --predicate 'process == "branch-deck"' --style
  compact` (zsh has a `log` builtin; the unified log is in local time, the app's log in UTC) has `isViewVisible(): …
  window visible 1` at the show, `didCommitLoadForFrame` at the load of the page, a `WebURLSchemeTaskProxy::startLoading`
  per command the page invokes, and a burst of `runJavaScriptInFrameInScriptWorld` per batch of events the backend
  sends.

## An agent's check copy

The user works in /Applications/BranchDeck.app, and the `bd` of other agents (`~/.local/bin/bd`) points into it: an
agent never quits, replaces or relaunches it, never runs `pnpm install-app` or `bd install`. It builds a copy with an
identifier of its own and runs it from its lease on a scratch repository.
- **Build**: `pnpm tauri build --bundles app --no-sign --config
  '{"identifier":"org.develar.branch-deck.check","productName":"BranchDeckCheck","build":{"features":[]}}'` in the
  lease (about 7 min under load) gives `target/release/bundle/macos/BranchDeckCheck.app`; copy it out of `target/`
  (`notes.local/after/`), the next build overwrites it. The identifier gives the copy its own app data
  (`~/Library/Application Support/org.develar.branch-deck.check/`: the settings, the repository state, the window
  state), its own log (`~/Library/Logs/org.develar.branch-deck.check/BranchDeckCheck.log`, rotated at 10 MB: delete
  it before a run) and its own keychain items (the service of a secret is the name of the app data directory,
  `secret_store.rs`); no features means no updater.
- **Data**: before each launch write `settings.json` with one recent project, a scratch repository in
  `notes.local/` (a few commits with `(feature-N)` prefixes and a branch prefix, never one of the user's
  repositories), `repository-state.json` with a `lastSyncTime` an hour ago (a repository synced before syncs when the
  window opens it, as on the user's start) and `.window-state.json` with a small window in a corner (in physical
  pixels: on a 2× display `"width":1920,"height":1200,"x":1536,"y":960` is 960×600 points at the bottom right).
- **Run**: `open -n <copy>` (`--env NAME=value` for a switch of the check), its pid by the path of its executable
  (`pgrep -f "^<copy>/Contents/MacOS/branch-deck"`), then `kill` that pid, checked by its path, never `pkill -f`. A
  window that shows takes the focus: keep the launches few. Settings opens through the copy's menu without a key:
  `peekaboo menu click --pid <pid> --path "BranchDeckCheck > Settings…" --bridge-socket …`.
- **A window show**: the app log says what showed the window and `presented_ms`; the unified log of the process
  (`/usr/bin/log show --start … --predicate "processID == <pid>" --style compact`) has `window visible 1` when the
  window is ordered in. A temporary probe in the page (never committed) gives the milliseconds of the page: a
  `MutationObserver` for the shell and the rows, and a `ResizeObserver` observed then, whose first callback comes in
  the next rendering update, which paints and commits the layers. The first frame of the window carries the content
  only when the order in comes after that update: before the fix the window was ordered in 6–8 ms before it and
  showed the frame before (the shell) for 25–80 ms; with the page busy for 0.6 s after it said its content settled,
  for 0.67 s.
- **Frames**: Peekaboo captures 2–7 fps on a busy Mac, too few for a stale frame of 20–80 ms; make it long in the
  probe (a page busy for 0.6 s right after its ready) and compare the frames with and without the change. Give no
  `--capture-engine`: an explicit engine captures locally, without the Bridge's permission ("Screen Recording
  permission is required"). The region behind the window shows the user's screen: delete the frames after the check.
- **Linux and Windows**: the probe build in the VMs of [Platform Checks](platform-checks.md), with a probed `dist/`
  and an identifier of its own where the build reads `TAURI_CONFIG`. Linux: under `GDK_BACKEND=x11` the X window of
  the app can be captured from its creation at 60 fps (`xwininfo -root -tree` for its id, then `gst-launch-1.0
  ximagesrc xid=<id> use-damage=false ! … ! pngenc ! multifilesink`); the root of a rootless Xwayland is black, and
  a transparent pixel of the window is black too. Windows: a PowerShell loop of `CopyFromScreen` into bitmaps kept in
  memory, saved after the capture (30–60 fps), in physical pixels, like the window state, whose window must fit the
  work area or the plugin centres it; the console of a debug build opens beside the window. A debug build there takes
  about 6 GB without debug info (`CARGO_PROFILE_DEV_DEBUG=0`); with it the disk of the VM ran full. Delete the target
  and the data of the copy after the check.

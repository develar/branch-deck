# Checking the Real App (macOS, Peekaboo)

**Status:** Active  
**Updated:** 2026-10-29  
**Tags:** [macos, linux, release, screenshots, timing]

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
  and an identifier of its own where the build reads `TAURI_CONFIG`. Linux: [Linux in the VM](#linux-in-the-vm); the
  root of a rootless Xwayland is black, and `ximagesrc xid=<id>` gives a transparent pixel of the window as black too.
  Windows: a PowerShell loop of `CopyFromScreen` into bitmaps kept in memory, saved after the capture (30–60 fps), in
  physical pixels, like the window state, whose window must fit the work area or the plugin centres it; the console of
  a debug build opens beside the window. A debug build there takes about 6 GB without debug info
  (`CARGO_PROFILE_DEV_DEBUG=0`); with it the disk of the VM ran full. Delete the target and the data of the copy after
  the check.

## Linux in the VM

The first frames of the check copy on GNOME, in the VM "Ubuntu 26.04 ARM64" of [Platform Checks](platform-checks.md):
what the window draws and what the screen shows, cold and warm, under Wayland (GNOME's default) and X11.
- **Build** in the Docker image `bd-linux-build` with the `dist/` of `pnpm build`, an identifier of its own and a target
  directory of its own: `TAURI_CONFIG='{"identifier":"org.develar.branch-deck.check","productName":"BranchDeckCheck"}'`
  and `CARGO_TARGET_DIR=/target/<name>`, then `cargo build --release -p branch-deck --features tauri/custom-protocol
  --config profile.release.lto=false --config profile.release.codegen-units=16` (about a minute with the registry
  volume, after the `rustup toolchain install` of a fresh container). No features: no updater.
- **Data**: a scratch repository in the VM (a bare `origin` with the first commit, then commits with `(feature-N)`
  prefixes and one `(feature-1/feature-3)`, `git config branchdeck.branchPrefix <prefix>` and `branchdeck.stacks
  true`), and before each launch, as `parallels`: `~/.local/share/<identifier>/settings.json`
  (`{"recentProjects":[{"path":"<repository>"}]}`), `repository-state.json` beside it
  (`{"repositoryState":{"<repository>":{"lastSyncTime":<ms>}}}`, milliseconds as `Date.now()`: seconds read as 1970,
  "Last synced 57 years ago") and `~/.config/<identifier>/.window-state.json` (`{"main":{"width":1000,"height":700,
  "x":110,"y":70,"prev_x":110,"prev_y":70,"maximized":false,"visible":true,"decorated":false,"fullscreen":false}}`).
  The user `parallels` has no git identity: the unit gets `GIT_CONFIG_GLOBAL=<a file with user.name and user.email>`,
  `~/.gitconfig` stays as it is, else the page shows its identity alert. GNOME maximizes a window that fills most of
  the work area as it maps: 1000×700 on the 1144×826 screen opens at 1077px wide under Wayland, a width the header
  overflowed at before 2026-10-29.
- **Run** a transient unit of your own from a script in the Mac home (`/media/psf/Home/…`): `systemd-run
  --unit=<unit> --uid=parallels -p WorkingDirectory=<a writable directory>` with `HOME`,
  `XDG_RUNTIME_DIR=/run/user/1000`, `DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus`, `XDG_DATA_DIRS`,
  `XDG_CURRENT_DESKTOP=ubuntu:GNOME`, and `WAYLAND_DISPLAY=wayland-0` or `GDK_BACKEND=x11` with `DISPLAY=:0` and
  `XAUTHORITY` (`/run/user/1000/.mutter-Xwaylandauth.*`). `prlctl exec` fails now and then with "PrlJob_GetRetCode:
  Invalid argument" whether the script ran or not: make the scripts idempotent and run them again. Stop the unit
  before the next launch and at the end: the unattended upgrade that runs when the VM resumes restarts the running
  units (needrestart), so a unit left behind comes back with its old binary, and its window, maximized and focused,
  covers the first frames of the next check (`systemctl list-units 'bd-*' 'lff-*'` lists them).
- **Logs**: `journalctl -u <unit> -o short-precise --since @<epoch of the launch>` stamps the app's lines in
  microseconds (its log file in whole seconds): the page's `main open … ms: loaded …, shell …`, the backend's
  `the page settled its content, showing the window` (`presented_ms=0`: WebKitGTK tells no presentation), the sync.
- **The window's own frames** (X11): GTK creates the X window at the show, 50–120 ms after that log line, so poll
  `xwininfo -root -tree` for `"BranchDeck"` every few ms and read the window from the first poll on. A Python `gi` loop
  of `Gdk.pixbuf_get_from_window(GdkX11.X11Window.foreign_new_for_display(display, xid), 0, 0, width, height)` at
  60 fps keeps the alpha, keeps only the frames that changed with the time each came, and writes them after the
  capture. Run it under `nice -n 19`: at full priority it took one of the two CPUs of the VM and WebKit painted about
  0.5 s after the window appeared instead of 107–275 ms. The first read may be opaque black (the X window before GTK
  drew it, which mutter never shows); then the window is transparent with GTK's shadow; then the page.
- **The screen** (what GNOME shows, Wayland and X11 alike): mutter's ScreenCast over D-Bus asks no consent:
  `CreateSession({})` of `org.gnome.Mutter.ScreenCast`, `RecordMonitor("", {"cursor-mode": <uint32 0>})` and `Start()`
  of the session, then `PipeWireStreamAdded(node)` of the stream; `pipewiresrc path=<node> do-timestamp=true !
  videoconvert ! jpegenc ! multifilesink post-messages=true` writes a frame per damage (30–60 fps while a window
  opens), and the running time of each `GstMultiFileSink` message on the system clock gives the wall time of its
  frame. GNOME shows its recording indicator meanwhile. Under X11 the screen shows GNOME animating the transparent
  window in, a see-through outline, until WebKit's first paint; under Wayland the first frame of the window has the
  page.
- **A warm start** in a scratch repository is synced before its page loaded, so its first frame shows the synced rows
  already. To see the rows of the frame first, slow the sync's commit list: a `git` first in the `PATH` of the unit
  (the app finds git by `PATH`) sleeps 2 s for `log --reverse --no-merges` while a marker file exists, then runs
  `/usr/bin/git`; a commit added between the launches shows the catch-up (a count of commits that changes).
- **Clean up**: stop the unit, delete the scratch directory, the data of the identifier (`~/.local/share/<identifier>`,
  `~/.config/<identifier>`), the frames and the target directory in the volume (`/target/<name>`).

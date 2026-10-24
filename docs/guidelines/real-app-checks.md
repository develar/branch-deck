# Checking the Real App (macOS, Peekaboo)

**Status:** Active  
**Updated:** 2026-10-24  
**Tags:** [macos, release, screenshots, timing]

## Summary

What a release build shows on screen (a window that opens late, a frame of "Loading…") is checked in the real app,
not only in E2E: `pnpm install-app` (5–7 min, quits and replaces /Applications/BranchDeck.app), then drive it with
the `peekaboo` CLI.
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
  loaded 240"): it shows on its first rows, so the frames of a start begin with the rows (or with the title row and
  the loading table when the first sync took longer than 1 s after the page loaded), never with a bare canvas.

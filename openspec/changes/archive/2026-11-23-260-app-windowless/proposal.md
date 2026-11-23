# Proposal

## Why

The desktop app becomes the one backend of a user (#260): the bridge of an IDE (`branch-deck ide`) starts it when it
does not run and talks to it over a local socket. So the app must start without a window, and it must not end while
an IDE is connected to it, as it did with its last window and with Quit of the app menu (`terminate:` of AppKit).

## What Changes

- A start with `--background` builds no window and, on macOS, shows no Dock icon (`ActivationPolicy::Accessory`).
- The last window that closes, and our own Quit of the app menu (⌘Q, in place of the predefined one), close the
  windows and keep the process while a bridge is connected; with no window and no connection the process exits 30 s
  after the later of the moment it became windowless and the end of its last connection. Quit of the Dock still ends
  the process.
- Reopen of macOS and `x-branch-deck/showMain` (the hook `MainWindowHost::show_main` for the host of the socket) build
  or show the main window and put the app back in the Dock; a second start with `--background` shows nothing.
- A restart starts the new process with the windows as they are now: without `--background` when a window is open,
  with it when none is.
- Without a main window the app checks for an update itself, at the times of the page of the main window.
- The rule is pure (`app_api::app::life`), fed by the count of the bridge connections (`BridgeConnections`), which the
  host of the socket sets; until it lands the count is always 0.

Capabilities: `repository/ide-embedding`, `repository/settings`.

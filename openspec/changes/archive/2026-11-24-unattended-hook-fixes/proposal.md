# Proposal

## Why

A review of Unattended Mode found five faults of its hook and its switch:

- The install read `~/.claude/settings.json`, merged, wrote and renamed with no look at the file between: a write of
  Claude Code in that window was lost.
- The rename replaced a settings file that is a symlink (stow, chezmoi) by a regular file.
- The hook was a command line of the shell form, which Claude Code runs in PowerShell on Windows without Git Bash, where
  a quoted path does not parse; and there a hook that fails blocks the tool call, with the mode off too.
- The switch of the app did not save the setting before it answered: after Turn Off the hook kept denying for up to
  500 ms, or until a failed save was retried, while the toast said off.
- A settings file the hook cannot go into refused the whole `brd install`, the `brd` symlink and the skills included.

## What Changes

- The hook is a command hook of the exec form, `command` the path of brd and `args` `hook permission-request`, with
  `"onFailure": "continue"`: no shell on any platform, and a hook that fails leaves the request to the prompt. A hook of
  the shell form is read as Branch Deck's, `stale`, replaced by an install and removed by an uninstall.
- An edit of the settings file reads it again just before the rename (`replace_if_unchanged`), and reads and merges
  again when it changed, five times at most; a symlink is written where it points.
- `set_unattended` saves the setting before it answers, for the app and for `brd unattended`; a failed save is an error.
- `brd install` installs the rest when the settings file cannot hold the hook, and says why in `hook_error`; the hook
  state `invalid` tells it in `brd unattended`, in Settings → Tools and in the toast of a switch on, and the toast of
  the install of the app is a warning that says it.

Capabilities: `repository/unattended-mode`.

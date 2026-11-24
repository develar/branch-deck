# Proposal

## Why

The hook of Unattended Mode sat in `~/.claude/settings.json` from `brd install` on, so the settings of every user who
installs the Agent CLI were written, also of one who never turns the mode on. Claude Code reads a hook written into
the file while its sessions run (its file watcher, within seconds, for a session and for a subagent started after it),
so the hook can come with the mode and go with it.

## What Changes

- Turning the mode on (the app's switch, the palette, `brd unattended on`) first adds the hook, then switches the
  setting; a hook that cannot be written (a settings file that is no JSON object, no home directory, a failed write)
  keeps the mode off and answers `install_refused`, which the app tells by `notifyError`.
- Turning it off switches the setting first, then removes the hook, and a `PermissionRequest` and a `hooks` that
  empties, so the file is what it was; a removal that fails is the `warning` of the answer, off all the same: the hook
  still reads the mode.
- `brd install` writes no hook: while the mode is off it removes the hook an earlier install wrote, while it is on it
  writes it anew for this brd; `--uninstall` removes it.
- Gone with their reason: the hook states of Settings → Tools and `AgentCliStatus`, the offer of Install Agent CLI in
  the toast of a switch on; `brd unattended` keeps `hook` as `present` or `absent`.
- A crash or a quit while the mode is on leaves the hook and the mode on: the night goes on.

Capabilities: `repository/unattended-mode`.

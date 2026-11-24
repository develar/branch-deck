# Proposal

## Why

Agent teams left running overnight stop at the first permission prompt of the lead or of any of its background
subagents: the prompt waits for a human until morning. The user wants a mode of Branch Deck, which only they turn on,
in which every such prompt is denied with a message that tells the agent to go on, and the prompts behave as usual
while it is off.

## What Changes

- The setting `appSettings.unattended`, off by default, written through the writer of the settings.
- `brd hook permission-request`, the `PermissionRequest` hook of Claude Code: in the mode it denies and records the
  request in a journal beside the settings; otherwise it prints nothing.
- `brd unattended on|off|status`: the switch and the requests denied since the mode was last turned on.
- `brd install` adds the hook to `~/.claude/settings.json` and `--uninstall` removes it, keeping the rest of the file.

Capabilities: `repository/unattended-mode` (new).

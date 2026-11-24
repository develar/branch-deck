# Proposal

## Why

The rules of the edit of the settings of Claude Code were named after `brd install`, which no longer writes the hook:
the switch of the mode adds and removes it.

## What Changes

- The requirements of the edit of the settings file are the group "Hook file:", named by the edit, not by the install.
- A hook of another brd or of the shell form is replaced by the hook of this brd; no state of it is read any more.

Capabilities: `repository/unattended-mode`.

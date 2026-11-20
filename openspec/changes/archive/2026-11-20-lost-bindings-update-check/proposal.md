# Proposal

## Why

The update of robot 1137287182 (2026-11-20) applied a clean replay that did not compile: a local commit removed a
private property and an import with their last use, and the upstream used both again in other hunks
(`lost-bindings-update`, the resolve session of `brd update`, archived in the same commit).

## What Changes

- The check of an update that would apply a clean rebase reads the lost bindings of its replay; a finding is a conflict
  that moves nothing, `FirstConflict.lost_bindings`.
- The app's words of it: the tooltip and Activity, the dialog (what is lost and where, no "Rebase in a terminal"), the
  prompt for an agent (each name kept in a fixup commit). No Update Anyway.

Capabilities: `repository/update-project`.

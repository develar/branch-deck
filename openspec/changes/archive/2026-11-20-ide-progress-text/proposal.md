# Proposal

## Why

The tab of the main view read "Branches 37%": the progress of the Dock reached the IDE as a bare fraction, and a robot
without an estimate sent `fraction: null`, which the contract reads as the clear, so such a robot showed nothing.

## What Changes

- In the IDE the progress names what runs before its percent: Safe Push or Dry Run of the robot the Dock follows,
  Update or Fetch of a running Update Project.
- A progress without an estimate is `setProgress {view: "main", text}` without `fraction`, a progress without a
  percent (idea-1 IJAI-6352). The Dock of the desktop app stays as it is.

Capabilities: `repository/ide-embedding`.

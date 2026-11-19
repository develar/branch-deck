# Proposal

## Why

The Studio has steady light traffic all day, the narrow bt runs of many sessions: an exclusive run that waits for the
runs in their command to end would often reach the 2 h of its wait, as new runs keep starting.

## What Changes

- While an exclusive run is at the head of the queue and no heavy run holds, it drains the host (`drain.json`, since
  when): each new run that is not heavy waits for it before its lend, in the step `exclusive`, with a line that names the
  measurement and the most it waits; 15 min after the start of the drain the bar lifts and the new runs start again,
  while the exclusive run waits on.

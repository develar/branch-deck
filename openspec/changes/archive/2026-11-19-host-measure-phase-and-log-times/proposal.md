# Proposal

## Why

On 2026-11-18 the external SSD of the Studio hung for 13 to 40 minutes at a time. The runs whose slots were on it waited
in `checkout`, but the idea-1 runs whose Bazel output base was on it lost 6 to 38 minutes that no step booked: the measure
of the cores of the command looks at the Bazel servers of the slot before the command and after it, outside every phase.
The log of the agent had no times, so its lines could not be set against the system log of the stall.

## What Changes

- A new step `measure` of the host books the measure of the cores outside the command, between `command` and `stage`.
- Each line of the log of the agent and of `supervisor.log` starts with the local time of the host to the millisecond,
  in one write.

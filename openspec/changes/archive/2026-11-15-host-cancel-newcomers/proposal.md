# Proposal

## Why

A cancel sends SIGINT to the process group of the command and SIGKILL 120 s later. A shell (`sh -c 'trap … INT; a;
b'`) that gets the SIGINT between two commands runs the next one before its trap, and a process that it forks as the
signal comes resets the handlers it had of the shell before its `exec`: neither sees the SIGINT, the shell waits for it,
and the run ends only when that process ends by itself or at the SIGKILL. `sigterm_to_the_client_cancels_the_run_on_the_host`
showed it: 9 of 300 runs under load took 62 s, the `sleep 60` that the shell started as the cancel came.

## What Changes

- A second after the SIGINT of a cancel, each process of the command group that started as it came (a second before
  it or later), but the command itself, gets SIGINT once more; the line of the run names each.
- Capability `cli/host`.

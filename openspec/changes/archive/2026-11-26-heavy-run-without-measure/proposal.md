# Proposal

## Why

A heavy run whose command has no measure took the whole host: the first runs of any command, every command whose text
changes (the commits of a gate, an inline script), and a new class of `--measure-as`. It waited for every holder and a
quiet host, and an agent was told to name a class, which agents do not do by default. On the Studio over 2 days the
heavy runs waited 412 h against 76 h of work, 8.8 of 20 cores busy on average.

## What Changes

- A run without a measure of its command takes the measure of its program: its first word and subcommand, the command
  of a script of a shell past its setup (`host_link::heavy::program_key`); each measure goes under that key too
  (`host_cores::expected_of`). Walls stay of the command.
- A run of no measure of either takes the cores the host assumes of one: `branchdeck.hostUnmeasuredCores`, else half
  of its cores, as `expected` of its entry with `expectation: assumed`; a holder of them holds the most it used of the
  last 5 min when that is more. The holder of the build before keeps the whole host.
- The lines of `brd-host runs` and of a wait say which; the history names the need `assumed`, which stats does not
  count for the whole host. `pnpm perf:host-admission` replays the history of a host under these rules.

## Decisions

- **It reverses "a default measure was not taken"**: the holder of assumed cores now holds what it shows within a
  sample, and the program gives most such runs a measure first. A replay of 66 h of the Studio (`pnpm
  perf:host-admission`, its model of the turn before within 15 % of the real wait): 328 h with the turn before, 291 h
  with the program alone, 263 h with both, at the same stretch.
- **No key without the commits of a run**: it gave 5 % of the runs without a measure one, the program 51 %.
- **No overcommit**: the history shows runs side by side slowing up to 3 times beyond the cores they share.

Capabilities: `cli/host`.

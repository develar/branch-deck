# Proposal

## Why

On 2026-11-19 the Studio ran at a load of 61 to 84 on 20 cores, `top` at 0 % idle, while the heavy turn counted 8.9
cores held and a fast lane that held "the whole host" timed out its 20 test shards. The measures were far too low: a
cold `cargo build -p git-ops` used 221 s of CPU in 108 s (2.0 cores) and measured 0.8. A sample saw only the processes
alive at it, so each rustc between two samples was lost, and `RUSAGE_CHILDREN` of the supervisor lost the build too:
`zsh -c` execs its last command, and macOS drops the reaped children of a process at its exec. The actions of a Bazel
server, in groups of their own, counted not at all. And the admission counted only what the runs noted.

## What Changes

- A sample counts every descendant of the command group and of the Bazel servers of the slot, each with the CPU of its
  reaped children, with a credit for a process that a sample saw alive and its reaper counts again.
- `serve` samples the busy cores of the whole host every 2 s into `host.json` of the heavy turn; an admission counts the
  busy cores outside the holders, and a run that takes the whole host waits for a quiet host.

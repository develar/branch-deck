# Proposal

## Why

On 2026-11-18 gets of the idea-1 pool took 25 to 62 minutes, and nothing said which step took the time: the clone, the
checkout, the wait for the clones lock, or the trim and shrink a get runs below the minimum free space. The steps went
to stderr without a time, and the elapsed times of the clone were only in the debug log (`-v`), which no agent passes.

## What Changes

- Each step a get prints on stderr names the time since the get began: `brd lease get: [3 min 12 s] checking out …`.
- The answer of a get lists its steps with when each started (`steps`, `at_ms`), so a transcript keeps where the time
  went.

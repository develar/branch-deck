## 1. The class and the place

- [x] 1.1 `host_cores`: the walls of the passed runs (`walls.json`, `passed_wall`); `record` takes whether the command
  passed, and `measure_end` of the supervisor passes it
- [x] 1.2 `host_turn::short_wall`, `host_turn::place`, the marks of `passes.json`; `enqueue` puts a short run in its
  place, counts the passes, carries them over a supersede and takes the earliest join of the runs it passed

## 2. The lines

- [x] 2.1 `turn::joined_lines`: the runs that a short run went ahead of
- [x] 2.2 `turn::live` sends `marks`, `turn::lines_at` marks a short run and the passes of a run

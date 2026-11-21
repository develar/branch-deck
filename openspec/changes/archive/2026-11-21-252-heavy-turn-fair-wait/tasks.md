## 1. The admission

- [x] 1.1 `LOAD_BOUND` and `TurnParams::load_bound`: a head of the queue that waited 15 min takes a free turn whatever
  the load outside the heavy runs; an exclusive run waits for its quiet host

## 2. The measures

- [x] 2.1 `host_link::heavy::measure_key` with the directories of `RunSpec::fetch`, in `turn::take` and `turn::record`
- [x] 2.2 a run that waits reads the measures of its command again at each look

## 3. The lock of the table

- [x] 3.1 `turn::take` retries a look whose lock another process held for 2 s, with its line at once and once a minute
- [x] 3.2 `host_turn::look`: a run behind the head of the queue reads the table without the lock

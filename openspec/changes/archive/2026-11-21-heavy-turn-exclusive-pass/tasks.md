## 1. The admission

- [x] 1.1 `host_turn::passes`: the first run behind an exclusive head whose drain ended starts past it while the head
  cannot start and `admits` admits that run; `look` takes the lock for that run
- [x] 1.2 `TurnParams::drain_bar`, `HeadDrain`, `TurnAnswer::Passed`, `TurnEntry::passed`

## 2. The lines

- [x] 2.1 `host_runs::turn_line`: the exclusive run ahead, the drain of the head, the holder that passed
- [x] 2.2 `turn::took_line`, `turn::lines_at` and `head_drain` of `turn::live`

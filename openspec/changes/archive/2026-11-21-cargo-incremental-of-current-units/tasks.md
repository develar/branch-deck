## 1. The incremental dirs of current units

- [x] 1.1 `Unit::current` from `split_groups`; only a current unit keeps the dir it wrote (`split_units`)
- [x] 1.2 The newest incremental dir of each crate stays, whatever `keep` says
- [x] 1.3 Tests: `an_incremental_dir_stays_while_the_current_unit_that_wrote_it_stays`, `the_cargo_rule_keeps_the_newest_and_the_young_units`

# An incremental dir of the cargo rule stays while the unit that wrote its session stays

## Why

A review of the cargo rule found that it keeps the 8 newest dirs of `incremental/` per crate, but the 8 newest compile
units per package and target. The library, its check, its test harness and its clippy check of one crate share one
group of 8 dirs. Every build script of the workspace shares the crate `build_script_build`. So the rule often deleted
the session of a kept unit, and the next build of that unit compiled every part of the crate again.

## What Changes

- An incremental dir also stays while a kept unit of its crate wrote its newest session: the lock of that session is
  between `invoked.timestamp` of the unit and the newest file of its fingerprint dir.
- The group of the crate stays as before, so the rule deletes no dir that it kept before.
- The guards of the object pass stay as they are.
- Capability `cli/lease`.

# A lease names the build config that its home branch changed since it forked

## Why

A long session works in its lease for hours without a pick. When its home branch changes the build config in that
time, the builds of the lease still use the old config. On 2026-11-15 master changed `.cargo/config.toml` to stop the
growth of `*.rcgu.o` objects, and leases 1 to 5 commits behind master kept the old config until their next pick.
`brd workspace` in a lease named no such change, and the skill did not tell the holder when to run it there.

## What Changes

- In a lease with a home branch, `home.build_config_changed` of `brd workspace` lists the build config files that the
  branch changed since the fork point of the lease (the merge base of its HEAD and the branch), and that the lease has
  in another form. The field is omitted when there is none, and when the read fails.
- The build config is every `.cargo/config.toml`, `.cargo/config`, `rust-toolchain.toml` and `rust-toolchain`, a
  `Cargo.toml` whose top-level `profile` table changed, and the `[host]` table of the root `.branchdeck.toml`.
- A file whose blob, or table, in the lease HEAD is that of the home tip is not named. A table entry counts only when
  the home branch changed the table since the fork point.
- The read is one `diff-tree --merge-base` beside the `status` of the shared checkout, and one `cat-file` only when
  the diff names a build config file (`git_ops::paths::changed_paths_since_fork`, `lease_pool::build_config`).
- The skill tells the holder to run `brd workspace` in the lease when the session resumes and before a full build or
  test run, and to pick before the build when the field names files.
- Capability `cli/workspace`.

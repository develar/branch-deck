## ADDED Requirements

### Requirement: Disk: Each thread of a walk reads every directory into one buffer of its own

On macOS each thread of a walk MUST read every directory into one buffer of its own for `getattrlistbulk` (128 KiB).
The thread MUST make this buffer when it takes its first directory, not once per directory.

#### Scenario: A walk of many directories
- **WHEN** a walk reads 201 directories
- **THEN** it allocates at most one read buffer for each of its 8 threads
- **AND** a walk of one directory allocates at most one read buffer
  [@test] crates/fs-walk/tests/walk_buffer.rs (`a_walk_allocates_one_read_buffer_per_thread_not_per_directory`)

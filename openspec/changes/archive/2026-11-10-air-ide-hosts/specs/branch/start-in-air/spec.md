## MODIFIED Requirements

### Requirement: The IDE: Each IDE with Air is found by its instance file

Each IDE process with Air writes one instance file, `<pid>.json`, once at its start, in
`~/Library/Application Support/JetBrains/air/launch` (macOS), `%APPDATA%\JetBrains\air\launch` (Windows),
`$XDG_DATA_HOME/JetBrains/air/launch` or `~/.local/share/JetBrains/air/launch` (Linux). A file whose process does not
run, a file that does not parse and a file of another `schemaVersion` MUST be skipped. A directory that does not exist
MUST read as no instance, not as an error.

#### Scenario: The files of running processes are read, the others skipped
- **GIVEN** the directory of instance files of the platform
- **WHEN** Branch Deck reads it
- **THEN** it reads the file of each running IDE process: its `pid`, `product`, `build` and `socket`
- **AND** it skips a file whose process does not run, a file that does not parse and a file of another `schemaVersion`
  [@test] crates/air-launch/src/discovery_test.rs (`reads_the_files_of_running_processes_and_skips_dead_garbled_and_later_ones`, `find_instance_checks_the_processes_of_the_files`)

#### Scenario: No directory of instance files
- **WHEN** Branch Deck reads a directory of instance files that does not exist
- **THEN** it reads no instance, and no error
  [@test] crates/air-launch/src/discovery_test.rs (`no_directory_is_no_instance`)

### Requirement: The IDE: The IDE of the repository has its directory or a parent open

The IDE of the repository MUST be the one whose `hosts` holds the directory of the repository or one of its parents,
compared by path components; the deepest host first, then the newest `focusedAt` (the last focus of a window of that
IDE, Unix milliseconds; an IDE that answers none is older than any), then the newest instance file.

#### Scenario: A parent is open and the deepest project wins
- **WHEN** IDEs host the directory of the repository or one of its parents, compared by path components
- **THEN** the IDE of the deepest host is the IDE of the repository
  [@test] crates/air-launch/src/discovery_test.rs (`the_ide_of_a_directory_has_it_or_a_parent_open_and_the_deepest_project_wins`)

#### Scenario: Of two IDEs with the same project the newest file wins
- **WHEN** two IDEs host the same directory and neither answers a `focusedAt`
- **THEN** the IDE of the newest instance file is picked
  [@test] crates/air-launch/src/discovery_test.rs (`of_two_ides_with_the_same_project_the_newest_file_wins`)

#### Scenario: Of two IDEs with the same project the last focused wins
- **WHEN** two IDEs host the same directory
- **THEN** the one with the newest `focusedAt` is picked
- **AND** an IDE that answers no `focusedAt` is older than any
  [@test] crates/air-launch/src/discovery_test.rs (`of_two_ides_with_the_same_project_the_last_focused_wins_and_one_without_a_focus_is_older_than_any`)

#### Scenario: The deepest project wins over the last focus
- **WHEN** one IDE hosts a deeper directory and another answers the newer `focusedAt`
- **THEN** the IDE of the deeper host is picked
  [@test] crates/air-launch/src/discovery_test.rs (`the_deepest_project_wins_over_the_last_focus`)

## ADDED Requirements

### Requirement: The IDE: Each running IDE names its hosts

Branch Deck MUST ask each running IDE over its socket for its hosts (`{"v":1,"method":"hosts"}`, the answer
`{"status":"ok","hosts":[…],"focusedAt":…}`), all IDEs at once, each within 2 s (`HOSTS_TIMEOUT`). The IDE asks no
consent for it. An IDE that does not answer in time, fails or refuses MUST be skipped with a debug line.

#### Scenario: The hosts request and its answer
- **WHEN** the client asks an IDE for its hosts
- **THEN** the request has no fields but `v` and `method`
- **AND** the answer names the absolute launch directories and the last focus, or a refusal
  [@test] crates/air-launch/src/client_test.rs (`the_hosts_request_has_no_fields_and_reads_the_hosts_the_last_focus_and_a_refusal`)

#### Scenario: An IDE that refuses, fails or does not answer in time
- **WHEN** an IDE refuses `hosts`, has a socket nobody listens on, or does not answer in time
- **THEN** Branch Deck skips it and takes the hosts of the others
  [@test] crates/air-launch/src/discovery_test.rs (`each_ide_names_its_hosts_and_one_that_refuses_fails_or_does_not_answer_in_time_is_skipped`)

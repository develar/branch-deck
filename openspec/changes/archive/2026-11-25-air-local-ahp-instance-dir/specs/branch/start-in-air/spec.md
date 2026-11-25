## MODIFIED Requirements

### Requirement: The IDE: Each IDE with Air is found by its instance file

Each IDE process with Air that admits local apps writes one instance file, `<pid>.json`, in
`<PathManager.getCommonDataPath()>/air/ahp` of the IDE, and Branch Deck MUST read that directory. A file whose process
does not run, a file that does not parse and a file of another `schema` than 2 MUST be skipped. A directory that does
not exist MUST read as no instance, not as an error.

#### Scenario: The files of running processes are read, the others skipped
- **GIVEN** the directory of instance files of the platform
- **WHEN** Branch Deck reads it
- **THEN** it reads the file of each running IDE process: its `pid`, `product`, `build` and `socket`
- **AND** it skips a file whose process does not run, a file that does not parse and a file of another `schema`
  [@test] crates/air-launch/src/discovery_test.rs (`reads_the_files_of_running_processes_and_skips_dead_garbled_and_later_ones`, `find_instance_checks_the_processes_of_the_files`)

#### Scenario: No directory of instance files
- **WHEN** Branch Deck reads a directory of instance files that does not exist
- **THEN** it reads no instance, and no error
  [@test] crates/air-launch/src/discovery_test.rs (`no_directory_is_no_instance`)

#### Scenario: The directory of the instance files of each platform
- **WHEN** Branch Deck finds the directory of the instance files on macOS, Windows and Linux
- **THEN** it is `~/Library/Application Support/JetBrains/air/ahp` on macOS
- **AND** `%APPDATA%\JetBrains\air\ahp` on Windows, else `~\AppData\Roaming\JetBrains\air\ahp`
- **AND** `$XDG_DATA_HOME/JetBrains/air/ahp` on Linux, else `~/.local/share/JetBrains/air/ahp`
- **AND** an empty variable is unset, as the IDE reads it, and so is a relative `XDG_DATA_HOME`
  [@test] crates/air-launch/src/discovery_test.rs (`the_directory_of_the_instance_files_is_the_common_data_directory_of_the_ide_on_each_platform`)

## MODIFIED Requirements

### Requirement: run: stdout holds only the stdout of the command

The stdout of `brd-host run` MUST hold only the bytes of the stdout of the command. A line MUST come when it ends,
and a line longer than 64 KiB in parts of at most 64 KiB. The stderr of the command and every line of brd-host MUST go
to the stderr.

#### Scenario: The streams of a run
- **WHEN** a run prints
- **THEN** stdout holds only the stdout of the command, each line when it ends
- **AND** the stderr of the command and every line of brd-host go to the stderr
  [@test] crates/bd-host/tests/cli.rs (`stdout_holds_only_the_command_and_the_lines_of_brd_host_go_to_stderr`)

### Requirement: follow, cancel, fetch, forget, runs: follow shows a run from its first line

`brd-host follow <alias> <run>` MUST follow the output of a run from its first line, with the bytes of each stream as
`run` writes them, and exit with the exit code of the command at its end. Ctrl+C MUST leave the follow, and the run
goes on.

#### Scenario: A follow of a run
- **WHEN** `brd-host follow <alias> <run>` runs
- **THEN** it shows the output of the run from its first line, with the bytes of each stream as `run` writes them
- **AND** it exits with the exit code of the command at the end of the run
  [@test] crates/bd-host/tests/cli.rs (`follow_of_an_ended_run_gives_the_bytes_of_its_stdout`)
  [@test] crates/host-agent/tests/serve/follow.rs (`a_follow_of_an_ended_run_gives_the_bytes_of_each_stream`)
  [@test] crates/host-client/src/run_test.rs (`the_output_sink_gets_the_bytes_of_each_stream`)

#### Scenario: Ctrl+C during a follow
- **WHEN** the user presses Ctrl+C during `brd-host follow`
- **THEN** the follow leaves, and the run goes on on the host

## ADDED Requirements

### Requirement: run: The output of a run keeps every byte of the command

The stdout of the command MUST reach the stdout of `brd-host run` byte for byte. The stderr MUST hold every byte of the
stderr of the command in its order. The only byte that brd-host adds to it is a line end before a line of its own. A
cut MUST add no line end, and each line end, carriage return and byte that is not UTF-8 MUST stay. A last stdout line
without a line end MUST stay without one.

#### Scenario: A line of 200 KiB without a line end
- **WHEN** the command prints one stdout line of 200 KiB without a line end
- **THEN** stdout gets its bytes, with no line end between them or after them
  [@test] crates/bd-host/tests/cli.rs (`a_line_of_200_kib_without_a_line_end_reaches_stdout_byte_for_byte`)
  [@test] crates/host-agent/src/supervise_test.rs (`a_long_line_comes_in_partial_pieces`)

#### Scenario: A character across the bound of an event
- **WHEN** a character of 4 bytes starts 1, 2 or 3 bytes before the bound of an event, 64 KiB
- **THEN** stdout and stderr get its bytes unchanged
  [@test] crates/bd-host/tests/cli.rs (`a_character_across_the_bound_of_an_event_reaches_stdout_and_stderr_unchanged`)
  [@test] crates/host-agent/src/supervise_test.rs (`a_character_across_the_bound_goes_whole_into_the_next_piece`)
  [@test] crates/host-agent/src/run_test.rs (`a_cut_keeps_each_character_whole`)

#### Scenario: Short lines and a long line
- **WHEN** the command prints short lines, an empty line, a line of CR LF, a line of 150 KiB, bytes that are not UTF-8
  and a last line without a line end
- **THEN** stdout gets every byte and every line end, and its last line stays without one
- **AND** the stderr gets every byte of the command in its order, and a line end before the line of the phases
  [@test] crates/bd-host/tests/cli.rs (`short_lines_and_a_long_line_keep_every_line_end`)
  [@test] crates/host-agent/src/supervise_test.rs (`a_line_comes_without_its_line_end_and_a_last_line_without_one_comes_partial`)

### Requirement: run: A line of brd-host starts a line of its own

A line of brd-host on the stderr MUST start a line of its own. When the stderr of the command stopped inside a line,
brd-host MUST end that line first. The log of the agent MUST do the same before a line of the agent and before a piece
of another pipe in the same stream.

#### Scenario: A last line of the command without a line end
- **WHEN** the stderr of the command ends without a line end
- **THEN** a line end comes before the line of the phases
  [@test] crates/bd-host/tests/cli.rs (`short_lines_and_a_long_line_keep_every_line_end`)

#### Scenario: Two sources in one stream of the log
- **WHEN** a line of the agent, or a piece of another pipe, comes while a pipe has an open line in a stream of the log
- **THEN** the log ends that line first, and a pipe goes on with its own open line without a line end
  [@test] crates/host-agent/src/run_test.rs (`a_long_piece_becomes_several_events_and_another_source_ends_an_open_line`)

#### Scenario: A client without an output sink
- **WHEN** a client has no output sink, as in a test
- **THEN** the stdout collects as bytes, and `say` gets each stderr line, a line without its end before the next line
  of the client
  [@test] crates/host-client/src/run_test.rs (`without_a_sink_the_stdout_collects_and_say_gets_each_stderr_line`)

### Requirement: The link: A piece of output carries its bytes and its line end

`Reply::Output` and an event of `events.log` MUST carry one piece of one stream: a line without its line end, or a part
of a line. A part is at most 64 KiB, cut between two characters. `partial` MUST tell that no line end comes after the
piece. `base64` MUST hold the bytes of a piece that is not UTF-8, beside its lossy `text`.

#### Scenario: The JSON of a piece
- **WHEN** the agent sends a piece
- **THEN** it is `{"reply": "output", "stream", "text", "partial", "base64", "end"}`, with `partial` only when it is
  true and `base64` only for a piece that is not UTF-8
  [@test] crates/host-link/src/proto_test.rs (`a_piece_keeps_its_bytes_and_an_older_end_reads_it`, `a_request_and_a_reply_have_a_tagged_json_shape`)
  [@test] crates/host-agent/tests/serve/follow.rs (`a_follow_of_an_ended_run_gives_the_bytes_of_each_stream`)

#### Scenario: A follow from an offset inside a line
- **WHEN** a follow goes on from the offset after a partial piece, as after a lost connection
- **THEN** the next piece goes on with that line, and the client adds no line end between them
  [@test] crates/host-agent/tests/serve/follow.rs (`a_follow_of_an_ended_run_gives_the_bytes_of_each_stream`)
  [@test] crates/host-client/src/run_test.rs (`the_output_sink_gets_the_bytes_of_each_stream`)

#### Scenario: base64 that does not decode
- **WHEN** a piece has `base64` that does not decode
- **THEN** it is an error of the protocol, and the run refuses with `link`
  [@test] crates/host-link/src/proto_test.rs (`a_piece_keeps_its_bytes_and_an_older_end_reads_it`)
  [@test] crates/host-client/src/run_test.rs (`a_piece_whose_base64_does_not_decode_refuses_with_link`)

### Requirement: The link: A field that an older end ignores stays within the schema

A new field that an older end ignores MUST stay within schema 1, so `brd-host upgrade` takes the next binary over the
link. An older client MUST read a piece as a line: it ignores `partial` and `base64`. An older agent sends neither, so
each of its pieces MUST be a whole line. Only a client and an agent of this build or later keep every byte.

#### Scenario: An older client and an agent of this build
- **WHEN** a client of an older build reads a piece of an agent of this build
- **THEN** it reads `text`, ignores `partial` and `base64`, and writes a line end after each piece as before
  [@test] crates/host-link/src/proto_test.rs (`a_piece_keeps_its_bytes_and_an_older_end_reads_it`)

#### Scenario: A client of this build and an older agent
- **WHEN** a client of this build reads a piece of an agent of an older build, or `serve` of this build reads an event
  of an older supervisor
- **THEN** the piece is a whole line, and the hello of another build names `brd-host upgrade <alias>`
  [@test] crates/host-link/src/proto_test.rs (`a_piece_keeps_its_bytes_and_an_older_end_reads_it`)
  [@test] crates/host-agent/src/run_test.rs (`the_log_gives_complete_events_with_their_end_offsets`)
  [@test] crates/host-client/src/host_test.rs (`another_schema_refuses_and_another_build_runs_with_a_hint`)

# Tasks

- [x] 1.1 `host_link::proto::Piece` with `partial` and `base64`, in `Reply::Output` and in the events of `events.log`.
- [x] 1.2 The reader of a pipe of the agent keeps every byte and cuts a long line between two characters; the event log
  ends the open line of another source before a line of its own.
- [x] 1.3 The client writes the bytes of each piece to its output sink; the CLI writes them to stdout and to the stderr,
  and a line of brd-host starts a line of its own.
- [x] 1.4 The tests of the line reader, the event log, the protocol, the follow of the client and the agent, three
  process tests of `brd-host run` and one of `brd-host follow`; the spec, the design doc, the guide and the skill.

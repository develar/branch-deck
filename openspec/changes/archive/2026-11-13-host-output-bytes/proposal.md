# Proposal

## Why

`brd-host run` added a line end inside each stdout line longer than 64 KiB. The agent cut such a line into events of
at most 64 KiB, and each event was a line: the client wrote a line end after each one. The cut also decoded each piece
alone as lossy UTF-8, so a character across the cut became U+FFFD. The reader dropped the carriage return of a CR LF,
and a last line without a line end got one. bt of idea-1 reads one JSON object from the stdout of
`brd-host run studio -- bt --json`: a JSON line of 96292 bytes got a line end at byte 65536, and the JSON did not parse.

## What Changes

- An event of the log of the agent and `Reply::Output` carry a piece of one stream (`host_link::proto::Piece`): a line
  without its line end, or a part of a line, with `partial` when no line end comes after it, and the bytes of a piece
  that is not UTF-8 in `base64`. The cut of a long line falls between two characters, and the reader of a pipe keeps
  every byte.
- The client writes the bytes of each piece, and its line end only when the piece has one. The stdout of
  `brd-host run` and of `brd-host follow` is the stdout of the command, byte for byte. The stderr holds every byte of
  the stderr of the command in its order. The only byte that brd-host adds to it is a line end before a line of its
  own.
- `partial` and `base64` are fields that an older end ignores, so the schema stays 1 and `brd-host upgrade` takes the
  next binary as before. An older client writes a line end after each piece, as it did. An older agent sends neither
  field, so each of its pieces is a whole line. Only a client and an agent of this build or later keep every byte.
- Capability: `cli/host`.
